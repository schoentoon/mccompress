use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use flate2;
use std::io;

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    UnsupportedCompressionFormat {
        /// Compression type byte from the format.
        compression_type: u8,
    },
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::Io(err)
    }
}

/// A region file
///
/// These normally have a .mca extension on disk.  They contain up to 1024 chunks, each containing
/// a 32-by-32 column of blocks.
#[allow(dead_code)]
pub struct RegionFile<T> {
    /// Offsets (in bytes, from the beginning of the file) of each chunk.
    /// An offset of zero means the chunk does not exist
    offsets: Vec<u32>,

    /// Timestamps, indexed by chunk.  If the chunk doesn't exist, the value will be zero
    timestamps: Vec<u32>,

    /// Size of each chunk, in number of 4096-byte sectors
    chunk_size: Vec<u8>,

    cursor: Box<T>,
}

impl<R> RegionFile<R>
where
    R: io::Read + io::Seek + io::Write,
{
    /// Parses a region file
    pub fn new(mut r: R) -> Result<RegionFile<R>, Error> {
        let mut offsets = Vec::with_capacity(1024);
        let mut timestamps = Vec::with_capacity(1024);
        let mut chunk_size = Vec::with_capacity(1024);

        for _ in 0..1024 {
            let v = r.read_u32::<BigEndian>()?;

            // upper 3 bytes are an offset
            let offset = v >> 8;
            let sector_count = (v & 0xff) as u8;

            offsets.push(offset * 4096);
            chunk_size.push(sector_count);
        }

        for _ in 0..1024 {
            let ts = r.read_u32::<BigEndian>()?;
            timestamps.push(ts);
        }

        Ok(RegionFile {
            offsets: offsets,
            timestamps: timestamps,
            chunk_size: chunk_size,
            cursor: Box::new(r),
        })
    }

    /// Returns a unix timestamp of when a given chunk was last modified.  If the chunk does not
    /// exist in this Region, return `None`.
    ///
    /// # Panics
    ///
    /// x and z must be between 0 and 31 (inclusive).  If not, panics.
    pub fn get_chunk_timestamp(&self, x: u8, z: u8) -> Option<u32> {
        assert!(x < 32);
        assert!(z < 32);
        let idx = x as usize % 32 + (z as usize % 32) * 32;
        if idx < self.timestamps.len() {
            Some(self.timestamps[idx])
        } else {
            None
        }
    }

    /// Returns the byte-offset for a given chunk (as measured from the start of the file).
    ///
    /// # Panics
    ///
    /// x and z must be between 0 and 31 (inclusive).  If not, panics.
    fn get_chunk_offset(&self, x: u8, z: u8) -> u32 {
        assert!(x < 32);
        assert!(z < 32);
        let idx = x as usize % 32 + (z as usize % 32) * 32;
        self.offsets[idx]
    }

    /// Returns the amount of chunks in the file are used for this particular ingame chunk
    ///
    /// # Panics
    ///
    /// x and z must be between 0 and 31 (inclusive).  If not, panics.
    fn get_chunk_size(&self, x: u8, z: u8) -> usize {
        assert!(x < 32);
        assert!(z < 32);
        let idx = x as usize % 32 + (z as usize % 32) * 32;
        self.chunk_size[idx] as usize * 4096
    }

    /// Does the given chunk exist in the Region
    ///
    /// # Panics
    ///
    /// x and z must be between 0 and 31 (inclusive).  If not, panics.
    pub fn chunk_exists(&self, x: u8, z: u8) -> bool {
        assert!(x < 32);
        assert!(z < 32);
        let idx = x as usize % 32 + (z as usize % 32) * 32;
        self.offsets.get(idx).map_or(false, |v| *v > 0)
    }

    /// Figures out how many 'junk' bytes there are present for a specific chunk
    ///
    /// # Panics
    ///
    /// x and z must be between 0 and 31 (inclusive).  If not, panics.
    pub fn junk_bytes(&mut self, x: u8, z: u8) -> Result<usize, Error> {
        let offset = self.get_chunk_offset(x, z);
        let chunk_size = self.get_chunk_size(x, z);

        self.cursor.seek(io::SeekFrom::Start(offset as u64))?;
        let total_len = self.cursor.read_u32::<BigEndian>()? as usize;
        let _ = self.cursor.read_u8()?; // this is the compression type but this is not relevant for us here

        let data = {
            // we subtract 5 here as the first 5 bytes are used for the length of the actual data
            // and the compression mode
            let mut v: Vec<u8> = Vec::with_capacity(chunk_size - 5);
            v.resize(chunk_size - 5, 0);
            self.cursor.read_exact(&mut v)?;
            v
        };

        for &n in &data[total_len..] {
            if n != 0u8 {
                return Ok(chunk_size - total_len);
            }
        }

        Ok(0)
    }

    fn recompress_chunk(
        &mut self,
        x: u8,
        z: u8,
        level: flate2::Compression,
    ) -> Result<(usize, usize), Error> {
        let offset = self.get_chunk_offset(x, z);
        let chunk_size = self.get_chunk_size(x, z);

        self.cursor.seek(io::SeekFrom::Start(offset as u64))?;
        let total_len = self.cursor.read_u32::<BigEndian>()? as usize;
        let compression_type = self.cursor.read_u8()?;

        assert!(chunk_size > total_len);

        if compression_type != 2 {
            return Err(Error::UnsupportedCompressionFormat { compression_type });
        }

        let compressed_data = {
            let mut v: Vec<u8> = Vec::with_capacity(total_len - 1);
            v.resize(total_len - 1, 0);
            self.cursor.read_exact(&mut v)?;
            v
        };

        // we decode the original stream and re compress it with the specified compression level
        let mut decoder = flate2::read::ZlibDecoder::new(io::Cursor::new(compressed_data));
        let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), level);

        // we copy the entire decoder into the new encoder
        io::copy(&mut decoder, &mut encoder)?;

        let mut compressed = encoder.finish()?;
        let new_len = compressed.len() + 1;

        // make sure the new length actually fits within the chunk size
        assert!(chunk_size - 5 > new_len);

        // pad the rest with zeros again
        compressed.resize(chunk_size - 5, 0);

        // as our data is prepared by now we're moving back to the start of this chunk
        self.cursor.seek(io::SeekFrom::Start(offset as u64))?;

        // then we right away write the new length and write the compression type (which will be the same)
        self.cursor.write_u32::<BigEndian>(new_len as u32)?;
        self.cursor.write_u8(compression_type)?;

        // and afterwards we're writing the newly compressed data
        self.cursor.write(&compressed)?;

        // we should be at the end of a file chunk now
        debug_assert_eq!(
            self.cursor.seek(io::SeekFrom::Current(0)).unwrap() % 4096,
            0
        );

        Ok((total_len, new_len))
    }

    pub fn recompress_region(
        &mut self,
        level: flate2::Compression,
    ) -> Result<(usize, usize), Error> {
        let mut out: (usize, usize) = (0, 0);
        for x in 0..32 {
            for z in 0..32 {
                if self.chunk_exists(x, z) {
                    let res = self.recompress_chunk(x, z, level)?;
                    out.0 += res.0;
                    out.1 += res.1;
                }
            }
        }
        Ok(out)
    }

    fn clean_chunk(&mut self, x: u8, z: u8) -> Result<usize, Error> {
        let offset = self.get_chunk_offset(x, z);
        let chunk_size = self.get_chunk_size(x, z);

        self.cursor.seek(io::SeekFrom::Start(offset as u64))?;
        let total_len = self.cursor.read_u32::<BigEndian>()? as usize;

        assert!(chunk_size > total_len);

        let size = chunk_size - total_len - 4 as usize;

        self.cursor.seek(io::SeekFrom::Current(total_len as i64))?;

        let zero = {
            let mut v: Vec<u8> = Vec::with_capacity(size);
            v.resize(size, 0);
            v
        };

        self.cursor.write(&zero)?;

        // we should be at the end of a file chunk now
        debug_assert_eq!(
            self.cursor.seek(io::SeekFrom::Current(0)).unwrap() % 4096,
            0
        );

        Ok(size)
    }

    pub fn clean_junk(&mut self) -> Result<usize, Error> {
        let mut out: usize = 0;
        for x in 0..32 {
            for z in 0..32 {
                if self.chunk_exists(x, z) {
                    let res = self.clean_chunk(x, z)?;
                    out += res;
                }
            }
        }
        Ok(out)
    }
}
/// Loads a chunk into a parsed NBT Tag structure.
///
/// # Panics
///
/// x and z must be between 0 and 31 (inclusive).  If not, panics.
/*pub fn load_chunk(&mut self, x: u8, z: u8) -> Result<nbt::Tag, nbt_error::Error> {
    let offset = self.get_chunk_offset(x, z); // might panic

    self.cursor.seek(SeekFrom::Start(offset as u64))?;
    let total_len = self.cursor.read_u32::<BigEndian>()? as usize;
    let compression_type = self.cursor.read_u8()?;

    if compression_type != 2 {
        return Err(nbt_error::Error::UnsupportedCompressionFormat{compression_type});
    }

    let compressed_data = {
        let mut v: Vec<u8> = Vec::with_capacity(total_len - 1);
        v.resize(total_len - 1, 0);
        self.cursor.read_exact(&mut v)?;
        v
    };

    let mut decoder = flate2::read::ZlibDecoder::new(Cursor::new(compressed_data));

    let (_, tag) = nbt::Tag::parse(&mut decoder).unwrap();
    Ok(tag)

}*/

#[test]
fn test_region() {
    use std::fs::File;

    let f = File::open("tests/data/r.0.0.mca").unwrap();
    let mut region = RegionFile::new(f).unwrap();

    let ts = region.get_chunk_timestamp(0, 0).unwrap();
    assert_eq!(ts, 1383443712);

    let ts = region.get_chunk_timestamp(13, 23).unwrap();
    assert_eq!(ts, 0);

    let ts = region.get_chunk_timestamp(14, 10).unwrap();
    assert_eq!(ts, 1383443713);

    assert!(region.chunk_exists(14, 10));
    assert!(!region.chunk_exists(15, 15));

    assert_eq!(region.get_chunk_offset(0, 0), 180224);

    assert_eq!(region.junk_bytes(14, 10).unwrap(), 0);
}

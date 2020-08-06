use std::io::{Read, Seek, SeekFrom};
use byteorder::{BigEndian, ReadBytesExt};
//use flate2;

/// A region file
///
/// These normally have a .mca extension on disk.  They contain up to 1024 chunks, each containing
/// a 32-by-32 column of blocks.
#[allow(dead_code)]
pub struct RegionFile<T>
{
    /// Offsets (in bytes, from the beginning of the file) of each chunk.
    /// An offset of zero means the chunk does not exist
    offsets: Vec<u32>,

    /// Timestamps, indexed by chunk.  If the chunk doesn't exist, the value will be zero
    timestamps: Vec<u32>,

    /// Size of each chunk, in number of 4096-byte sectors
    chunk_size: Vec<u8>,

    cursor: Box<T>,
}



impl<R> RegionFile<R> where R: Read + Seek
{
    /// Parses a region file
    pub fn new(mut r: R) -> Result<RegionFile<R>, Box<dyn std::error::Error>> {
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
    pub fn junk_bytes(&mut self, x: u8, z: u8) -> Result<u32, Box<dyn std::error::Error>> {
        let offset = self.get_chunk_offset(x, z);

        self.cursor.seek(SeekFrom::Start(offset as u64))?;
        let total_len = self.cursor.read_u32::<BigEndian>()? as usize;
        let _ = self.cursor.read_u8()?; // this is the compression type but this is not relevant for us here

        let data = {
            let mut size = 4096 - 5; // the 5 is the metadata (size and compression type)
            while total_len > size {
                size += 4096;
            }
            let mut v: Vec<u8> = Vec::with_capacity(size);
            v.resize(size, 0);
            self.cursor.read_exact(&mut v)?;
            v
        };

        let mut junk = 0 as u32;
        for &n in &data[total_len..] {
            if n != 0u8 {
                junk += 1;
            }
        }

        Ok(junk)
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
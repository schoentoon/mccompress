use std::env;
use std::fs::File;

mod region;

fn main() {
    let args: Vec<String> = env::args().collect();

    let filename = &args[1];

    let f = File::open(filename).unwrap();
    let mut region = region::RegionFile::new(f).unwrap();

    let mut total_junk = 0 as u32;
    for x in 0..32 {
        for z in 0..32 {
            if region.chunk_exists(x, z) {
                let junk = region.junk_bytes(x, z).unwrap();
                total_junk += junk;
            }
        }
    }
    println!("{}", total_junk)
}

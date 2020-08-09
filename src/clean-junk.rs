use std::env;
use std::fs::OpenOptions;

mod region;

fn main() {
    let args: Vec<String> = env::args().collect();

    let filename = &args[1];
    println!("Reading {}", filename);

    let f = OpenOptions::new().write(true).read(true).open(filename).unwrap();
    let mut region = region::RegionFile::new(f).unwrap();

    let res = region.clean_junk().unwrap();
    
    println!("Cleaned up a total of {} bytes", res);
}

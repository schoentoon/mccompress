extern crate clap;
extern crate walkdir;

use clap::Clap;
use flate2::Compression;
use std::fs::OpenOptions;
use std::path::PathBuf;
use threadpool::ThreadPool;
use walkdir::{DirEntry, WalkDir};

mod region;

#[derive(Clap)]
struct Opts {
    #[clap(subcommand)]
    subcmd: SubCommand,
}

#[derive(Clap)]
enum SubCommand {
    Cleanup(CleanupOpts),
    Recompress(RecompressOpts),
}

#[derive(Clap)]
struct CleanupOpts {
    // the files/folders that should be processed
    #[clap(required = true)]
    input: Vec<PathBuf>,

    // the amount of jobs are allowed to run at the same time
    #[clap(short, long, default_value = "16")]
    jobs: usize,
}

#[derive(Clap)]
struct RecompressOpts {
    // the level of compression that should be used to recompress, 1 being the fastest, 9 being the best
    #[clap(short, long, default_value = "5")]
    level: u32,

    // the files/folders that should be processed
    #[clap(required = true)]
    input: Vec<PathBuf>,

    // the amount of jobs are allowed to run at the same time
    #[clap(short, long, default_value = "16")]
    jobs: usize,
}

fn is_mca(entry: &DirEntry) -> bool {
    let file_type = entry.file_type();
    entry
        .file_name()
        .to_str()
        .map(|s| file_type.is_dir() || (file_type.is_file() && s.ends_with(".mca")))
        .unwrap_or(false)
}

fn cleanup_handle(subopts: &CleanupOpts) {
    let pool = ThreadPool::new(subopts.jobs);

    let cleanup = |file: &DirEntry| {
        let res = || -> Result<usize, region::Error> {
            let f = OpenOptions::new()
                .write(true)
                .read(true)
                .open(file.path())?;
            let mut region = region::RegionFile::new(f)?;

            region.clean_junk()
        };

        match res() {
            Ok(_res) => {
                println!("Proccessed {}", file.path().display());
            }
            Err(error) => {
                println!(
                    "Error while processing {}: {:?}",
                    file.path().display(),
                    error
                );
            }
        };
    };

    for dir in &subopts.input {
        WalkDir::new(dir)
            .into_iter()
            .filter_entry(|e| is_mca(e))
            .filter_map(|v| v.ok())
            .for_each(|x| {
                let metadata = x.metadata().unwrap();
                if metadata.is_file() && metadata.len() > 0 {
                    pool.execute(move || cleanup(&x));
                }
            });
    }

    pool.join();
}

fn recompress_handle(subopts: &RecompressOpts) {
    let pool = ThreadPool::new(subopts.jobs);

    for dir in &subopts.input {
        WalkDir::new(dir)
            .into_iter()
            .filter_entry(|e| is_mca(e))
            .filter_map(|v| v.ok())
            .for_each(|file| {
                let metadata = file.metadata().unwrap();
                if metadata.is_file() && metadata.len() > 0 {
                    let level = subopts.level;
                    pool.execute(move || {
                        let res = || -> Result<usize, region::Error> {
                            let f = OpenOptions::new()
                                .write(true)
                                .read(true)
                                .open(file.path())?;
                            let mut region = region::RegionFile::new(f)?;

                            let res = region.recompress_region(Compression::new(level));

                            match res {
                                Ok(r) => Ok(r.1),
                                Err(error) => Err(error),
                            }
                        };

                        match res() {
                            Ok(_res) => {
                                println!("Processed {}", file.path().display());
                            }
                            Err(error) => {
                                println!(
                                    "Error while processing {}: {:?}",
                                    file.path().display(),
                                    error
                                );
                            }
                        };
                    });
                }
            });
    }

    pool.join();
}

fn main() {
    let opts: Opts = Opts::parse();

    match opts.subcmd {
        SubCommand::Cleanup(subopts) => {
            cleanup_handle(&subopts);
        }
        SubCommand::Recompress(subopts) => {
            recompress_handle(&subopts);
        }
    }
}

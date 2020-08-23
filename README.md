mccompress
==========

Unlike the name might suggest, this won't make your minecraft worlds smaller.
It is however likely to make compressed backups of your minecraft worlds smaller.

It does this by simply cleaning up unused bytes left over in the mca files, which usually ends up in making the files better compressable.
Below are some publicly available minecraft maps processed by this tool and recompress in the same format afterwards.
As those results show, results may vary greatly

| Map | Before | After | Difference
| - | - | - | - |
| [HermitCraft Season 6](http://download.hermitcraft.com/hermitcraft6.zip) | 2,068,732,229B | 1,700,648,978B | 17.8% |
| [HermitCraft Season 5](http://download.hermitcraft.com/hermitcraft5.zip) | 3,080,706,585B | 2,830,558,543B | 8.1% |
| [HermitCraft Season 4](http://download.hermitcraft.com/hermitcraft4_full.zip) | 2,788,339,294B | 2,602,330,397B | 6.6% |

To build this tool you'll need to have [rust](https://www.rust-lang.org/) and cargo installed.
After cloning this repository, simply run `cargo build --release`. The binary will be available at `./target/release/mccompress`

This tool has 2 modes of operations. It has the cleanup method, which only zeros out the unused bytes and is usually very fast.
And it has a recompress method as well, where it'll not only zero out the unused bytes, but it will also recompress the chunks
allowing you to compress it with a higher compression level of gzip. This is rarely worth it however.
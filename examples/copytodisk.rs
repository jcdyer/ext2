extern crate ext2;

use std::fs::File;
use std::io::{Read, Write};
use ext2::{Disk, Ext2};

fn main() {
    let ext2 = File::open("basic.ext2").and_then(Ext2::new).unwrap();
    let mut pattern = ext2.open("/sub/pattern/test_pattern.txt").unwrap();
    let mut buf = Vec::new();
    pattern.read_to_end(&mut buf).expect("ERRER");
    let mut outfile = File::create("/tmp/ext2-test-pattern.txt").unwrap();
    outfile.write_all(&buf).unwrap();
}

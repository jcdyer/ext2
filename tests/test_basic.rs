#![cfg(test)]

extern crate ext2;

use std::fs::File;
use std::io::{self, Read, Seek};

use ext2::Ext2;

#[test]
fn file_open() {
    let fs = File::open("basic.ext2").and_then(Ext2::new).unwrap();
    assert!(fs.open("/goodbye.txt").is_ok());
    assert!(fs.open("/sub/michelle.jpg").is_ok());
    assert!(fs.open("/goodbye.doc").is_err());
}

#[test]
fn file_seek() {
    let fs = File::open("basic.ext2").and_then(Ext2::new).unwrap();
    let mut file = fs.open("/sub/michelle.jpg").unwrap();
    assert_eq!(file.seek(io::SeekFrom::Current(0)).unwrap(), 0);
    assert!(file.seek(io::SeekFrom::Current(-1)).is_err());
    assert_eq!(file.seek(io::SeekFrom::End(0)).unwrap(), 75557);
    assert_eq!(file.seek(io::SeekFrom::End(-256)).unwrap(), 75301);
}

#[test]
fn file_read_full_block() {
    let fs = File::open("basic.ext2").and_then(Ext2::new).unwrap();
    let mut f = fs.open("/hello.txt").unwrap();
    let mut buf = [255; 24];
    f.read(&mut buf[..]).unwrap();
    assert_eq!(
        &buf[..],
        b"Hello world!\n\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff"
    );
}

#[test]
fn file_read_more_than_a_block() {
    // Current implementation only reads one block at a time.
    let fs = File::open("basic.ext2").and_then(Ext2::new).unwrap();
    let mut f = fs.open("/sub/michelle.jpg").unwrap();
    let mut buf = [255; 4099];
    f.read(&mut buf[..]).unwrap();
    assert_eq!(&buf[4094..], b"\x66\x47\xff\xff\xff");
}

#[test]
fn file_read_past_end_of_file() {
    // Reading truncates at the end of the file
    let fs = File::open("basic.ext2").and_then(Ext2::new).unwrap();
    let mut f = fs.open("/sub/michelle.jpg").unwrap();
    f.seek(io::SeekFrom::End(-256)).unwrap();
    let mut buf = [255; 4096];
    assert_eq!(f.read(&mut buf[..]).unwrap(), 256);
    assert_eq!(
        &buf[250..262],
        b"\xd4\x67\x2f\x4f\xff\xd9\xff\xff\xff\xff\xff\xff"
    );
}

#[test]
fn file_read_no_more_than_requested() {
    let fs = File::open("basic.ext2").and_then(Ext2::new).unwrap();
    let mut f = fs.open("/goodbye.txt").unwrap();
    f.seek(io::SeekFrom::Start(6)).unwrap();
    let mut buf = [255; 5];
    assert_eq!(f.read(&mut buf[..]).unwrap(), 5);
    assert_eq!(&buf[..], b"Pkunk");
}

#[test]
fn read_large_file() {
    let fs = File::open("basic.ext2").and_then(Ext2::new).unwrap();
    let bs = fs.block_size().unwrap() as usize;
    let mut f = fs.open("/sub/pattern/test_pattern.txt").unwrap();
    let mut buf = String::new();
    f.read_to_string(&mut buf).unwrap();
    for i in 1..10 {
        assert_eq!(
            &buf[bs * i - 1..bs * i + 15],
            &format!("\n{} .............", i)
        );
    }
    for i in 10..16 {
        // One fewer dot for two digit numbers
        assert_eq!(
            &buf[bs * i - 1..bs * i + 15],
            &format!("\n{} ............", i)
        );
    }
}

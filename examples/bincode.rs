//! Try using the bincode crate to deserialize the superblock.

extern crate bincode;
extern crate ext2;

use std::error::Error;
use std::fs::File;
use std::io::prelude::*;
use std::io::SeekFrom;
use std::mem;
use std::path::Path;
use std::slice;

use ext2::Superblock;

/// Load the first superblock from the ext2 image given by path.
fn load_superblock<P: AsRef<Path>>(path: P) -> Result<[u8; 1024], Box<Error>> {
    let mut buf = [0; 1024];
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(1024))?;
    file.read_exact(&mut buf)?;
    Ok(buf)
}

/// Construct a Superblock instance by copying the raw data.SeekFrom
///
/// This essentially performs a memcpy().  It only works correctly on little endian systems, since
/// it the superblock itself is stored in little endian on disk.
#[cfg(target_endian = "little")]
fn new_superblock_copy(raw_block: &[u8]) -> Superblock {
    let length = mem::size_of::<Superblock>();
    let mut superblock;
    let superblock_buf;
    unsafe {
        superblock = mem::uninitialized();
        superblock_buf = slice::from_raw_parts_mut(&mut superblock as *mut _ as *mut u8, length);
    }
    superblock_buf.copy_from_slice(&raw_block[..length]);
    superblock
}

fn main() -> Result<(), Box<Error>> {
    let raw_block = load_superblock("basic.ext2")?;
    let superblock_ext2 = Superblock::new(&raw_block)?;

    // Parse the superblock using bincode and print whether the result matches.
    let superblock_bincode = bincode::deserialize(&raw_block)?;
    println!("{}", superblock_ext2 == superblock_bincode);

    // On little endian machines, memcpy the superblock and print whether the results match.
    #[cfg(target_endian = "little")]
    {
        let superblock_copy = new_superblock_copy(&raw_block);
        println!("{}", superblock_ext2 == superblock_copy);
    }

    Ok(())
}

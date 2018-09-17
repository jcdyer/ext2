#![cfg(test)]

extern crate ext2;

use std::fs::File;

use ext2::{Ext2, Superblock};

#[test]
fn basic_superblock() {
    let f = File::open("./basic.ext2").unwrap();
    let mut fs = Ext2::open(f).unwrap();
    let superblock = fs.superblock().unwrap();
    assert_eq!(superblock, Superblock::default())
}

#[test]
fn basic_descriptor() {
    let f = File::open("./basic.ext2").unwrap();
    let mut fs = Ext2::open(f).unwrap();
    let superblock = fs.superblock().unwrap();
    let table = fs.block_group_descriptor_table(&superblock).unwrap();
    let mut expected = BlockGroupDescriptor::default();
    expected.bg_block_bitmap = 2;
    expected.bg_inode_bitmap = 3; expected.bg_inode_table = 4; expected.bg_free_blocks_count = 30; expected.bg_free_inodes_count = 17; expected.bg_used_dirs_count = 3; expected.bg_pad = 4; expected.bg_reserved = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    assert_eq!(table, vec![expected])
}





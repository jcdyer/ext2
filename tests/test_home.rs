#![cfg(test)]

extern crate ext2;
extern crate uuid;

use std::fs::File;

use ext2::{BlockGroupDescriptor, Ext2, FileType, FsPath, Inode, Superblock};
use uuid::Uuid;

#[test]
fn basic_superblock() {
    let mut fs = File::open("./basic.ext2").and_then(Ext2::open).unwrap();
    let superblock = fs.superblock().unwrap();
    let expected = Superblock {
        s_inodes_count: 32,
        s_blocks_count: 64,
        s_r_blocks_count: 3,
        s_free_blocks_count: 30,
        s_free_inodes_count: 17,
        s_first_data_block: 0,
        s_log_block_size: 2,
        s_log_frag_size: 2,
        s_blocks_per_group: 32768,
        s_frags_per_group: 32768,
        s_inodes_per_group: 32,
        s_mtime: 1537149494,
        s_wtime: 1537149919,
        s_mnt_count: 1,
        s_max_mnt_count: 65535,
        s_magic: 61267,
        s_state: 1,
        s_errors: 1,
        s_minor_rev_level: 0,
        s_lastcheck: 1537147869,
        s_checkinterval: 0,
        s_creator_os: 0,
        s_rev_level: 1,
        s_def_resuid: 0,
        s_def_resgid: 0,
        s_first_ino: 11,
        s_inode_size: 128,
        s_block_group_nr: 0,
        s_feature_compat: 56,
        s_feature_incompat: 2,
        s_feature_ro_compat: 3,
        s_uuid: Uuid::from_bytes([
            175, 254, 89, 103, 185, 28, 68, 194, 156, 174, 245, 82, 44, 170, 139, 58
        ]),
        s_volume_name: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        s_last_mounted: FsPath::new([
            47, 104, 111, 109, 101, 47, 99, 108, 105, 102, 102, 47, 115, 114, 99, 47, 101, 120,
            116, 50, 47, 109, 110, 116, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]),
        s_algo_bitmap: 0,
        s_prealloc_blocks: 0,
        s_prealloc_dir_blocks: 0,
        _align: (0, 0),
        s_journal_uuid: Uuid::from_bytes([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
        s_journal_inum: 0,
        s_journal_dev: 0,
        s_last_orphan: 0,
        s_hash_seed: [3806470851, 3057855919, 2015335302, 3627203126],
        s_def_hash_version: 1,
        _hash_version_align: (0, 0, 0),
        s_default_mount_options: 12,
        s_first_meta_bg: 0,
    };
    assert_eq!(superblock, expected)
}

#[test]
fn basic_descriptor() {
    let mut fs = File::open("./basic.ext2").and_then(Ext2::open).unwrap();
    let superblock = fs.superblock().unwrap();
    let descriptor = fs.get_block_group_descriptor(0, &superblock).unwrap();
    let expected = BlockGroupDescriptor {
        bg_block_bitmap: 2,
        bg_inode_bitmap: 3,
        bg_inode_table: 4,
        bg_free_blocks_count: 30,
        bg_free_inodes_count: 17,
        bg_used_dirs_count: 3,
        bg_pad: 4,
        bg_reserved: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    };
    assert_eq!(descriptor, Some(expected));
    assert!(
        fs.get_block_group_descriptor(9999, &superblock)
            .unwrap()
            .is_none()
    );
}

#[test]
fn basic_inode() {
    let mut fs = File::open("./basic.ext2").and_then(Ext2::open).unwrap();
    let superblock = fs.superblock().unwrap();
    let inode = fs.get_inode(2, &superblock).unwrap().unwrap();
    let expected = Inode {
        i_mode: 16877,
        i_uid: 0,
        i_size: 4096,
        i_atime: 1537149907,
        i_ctime: 1537149905,
        i_mtime: 1537149905,
        i_dtime: 0,
        i_gid: 0,
        i_links_count: 4,
        i_blocks: 8,
        i_flags: 0,
        i_osd1: 3,
        i_block: ([5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], 0, 0, 0),
        i_generation: 0,
        i_file_acl: 0,
        i_dir_acl: 0,
        i_faddr: 0,
        i_osd2: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    };
    assert_eq!(inode, expected);
    assert_eq!(inode.file_type(), FileType::Directory);
}

#[test]
fn basic_directory() {}

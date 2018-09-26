#![cfg(test)]

extern crate ext2;

use std::fs::File;
use std::ffi::OsStr;
use std::io::{self, Read, Seek};
use std::os::unix::ffi::OsStrExt;

use ext2::{BlockGroupDescriptor, DirEntry, Ext2, FileType, FsPath, Inode, Superblock};

#[test]
fn basic_superblock() {
    let fs = File::open("./basic.ext2").and_then(Ext2::new).unwrap();
    let superblock = fs.superblock().unwrap();
    let expected = Superblock {
        s_inodes_count: 32,
        s_blocks_count: 64,
        s_r_blocks_count: 3,
        s_free_blocks_count: 12,
        s_free_inodes_count: 15,
        s_first_data_block: 0,
        s_log_block_size: 2,
        s_log_frag_size: 2,
        s_blocks_per_group: 32768,
        s_frags_per_group: 32768,
        s_inodes_per_group: 32,
        s_mtime: 1537710967,
        s_wtime: 1537711046,
        s_mnt_count: 2,
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
        s_feature_compat: 0x38,
        s_feature_incompat: 0x2,
        s_feature_ro_compat: 0x3,
        s_uuid: [
            175, 254, 89, 103, 185, 28, 68, 194, 156, 174, 245, 82, 44, 170, 139, 58
        ],
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
        s_journal_uuid: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
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
    let fs = File::open("./basic.ext2").and_then(Ext2::new).unwrap();
    let superblock = fs.superblock().unwrap();
    let descriptor = fs.get_block_group_descriptor(0, &superblock).unwrap();
    let expected = BlockGroupDescriptor {
        bg_block_bitmap: 2,
        bg_inode_bitmap: 3,
        bg_inode_table: 4,
        bg_free_blocks_count: 12,
        bg_free_inodes_count: 15,
        bg_used_dirs_count: 4,
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
    let fs = File::open("./basic.ext2").and_then(Ext2::new).unwrap();
    let superblock = fs.superblock().unwrap();
    let inode = fs.get_root_directory(&superblock).unwrap();
    let expected = Inode {
        i_mode: 16877,
        i_uid: 0,
        i_size: 4096,
        i_atime: 1537710973,
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
fn basic_directory_entry() {
    let fs = File::open("./basic.ext2").and_then(Ext2::new).unwrap();
    let superblock = fs.superblock().unwrap();
    let inode = fs.get_root_directory(&superblock).unwrap();
    let entries = fs.read_dir(&inode, &superblock).unwrap().unwrap();
    let expected = DirEntry {
        inode: 2,
        rec_len: 12,
        name_len: 1,
        file_type: 2,
        name: OsStr::from_bytes(b".").to_os_string(),
    };
    assert_eq!(entries.len(), 6);
    assert_eq!(entries[0], expected);
    let filenames: Vec<_> = entries.into_iter().map(|entry| entry.name).collect();
    assert_eq!(
        filenames,
        vec![".", "..", "lost+found", "hello.txt", "sub", "goodbye.txt"],
    );
}

#[test]
fn basic_file_entry() {
    let fs = File::open("./basic.ext2").and_then(Ext2::new).unwrap();
    let superblock = fs.superblock().unwrap();
    let inode = fs.get_root_directory(&superblock).unwrap();
    let entries = fs.read_dir(&inode, &superblock).unwrap().unwrap();
    let file_entry = entries
        .into_iter()
        .find(|entry| entry.file_type == FileType::File as u8)
        .unwrap();
    let expected_entry = DirEntry {
        inode: 12,
        rec_len: 20,
        name_len: 9,
        file_type: 1,
        name: OsStr::from_bytes(b"hello.txt").to_os_string(),
    };
    assert_eq!(file_entry, expected_entry);
    let expected_inode = Inode {
        i_mode: 33188,
        i_uid: 0,
        i_size: 13,
        i_atime: 1537149548,
        i_ctime: 1537149548,
        i_mtime: 1537149548,
        i_dtime: 0,
        i_gid: 0,
        i_links_count: 1,
        i_blocks: 8,
        i_flags: 0,
        i_osd1: 1,
        i_block: ([11, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], 0, 0, 0),
        i_generation: 270238708,
        i_file_acl: 0,
        i_dir_acl: 0,
        i_faddr: 0,
        i_osd2: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    };
    let file_inode = fs.get_inode(file_entry.inode, &superblock)
        .unwrap()
        .unwrap();
    assert_eq!(file_inode, expected_inode);
    assert_eq!(superblock.block_size(), 4096);
    let mut data = vec![0; 4096];
    let read = fs.read_inode_data_block(&file_inode, &mut data, 0, &superblock)
        .unwrap();
    assert_eq!(read, 4096);
    assert_eq!(&String::from_utf8(data).unwrap()[..13], "Hello world!\n");
}

#[test]
fn get_inode_from_directory() {
    let fs = File::open("./basic.ext2").and_then(Ext2::new).unwrap();
    let superblock = fs.superblock().unwrap();
    assert_eq!(
        fs.get_inode_from_abspath("/", &superblock)
            .unwrap()
            .unwrap(),
        fs.get_root_directory(&superblock).unwrap(),
    );
    let inode = fs.get_inode_from_abspath("/sub/michelle.jpg", &superblock)
        .unwrap()
        .unwrap();
    let obama_portrait = Inode {
        i_mode: 33188,
        i_uid: 0,
        i_size: 75557,
        i_atime: 1537149748,
        i_ctime: 1537149748,
        i_mtime: 1537149748,
        i_dtime: 0,
        i_gid: 0,
        i_links_count: 1,
        i_blocks: 160,
        i_flags: 0,
        i_osd1: 1,
        i_block: ([13, 14, 16, 15, 17, 18, 19, 20, 21, 22, 23, 24], 25, 0, 0),
        i_generation: 1337774247,
        i_file_acl: 0,
        i_dir_acl: 0,
        i_faddr: 0,
        i_osd2: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    };
    assert_eq!(inode, obama_portrait);
}

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
    assert_eq!(&buf[254..258], b"\x18\x62\xff\xff");
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

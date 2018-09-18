extern crate byteorder;
extern crate uuid;

use std::cmp::PartialEq;
use std::fmt;
use std::io;
use byteorder::{ByteOrder, LE};

mod disk;

fn array64(input: &[u8]) -> [u8; 64] {
    let mut a = [0; 64];
    copy_array(input, &mut a[..]);
    a
}

fn array16(input: &[u8]) -> [u8; 16] {
    let mut a = [0; 16];
    copy_array(input, &mut a[..]);
    a
}

fn array12(input: &[u8]) -> [u8; 12] {
    let mut a = [0; 12];
    copy_array(input, &mut a[..]);
    a
}

fn copy_array(input: &[u8], output: &mut [u8]) {
    if input.len() != output.len() {
        panic!("Requires an input length of {}", output.len());
    }
    for i in 0..input.len() {
        output[i] = input[i];
    }
}

#[derive(Clone)]
pub struct FsPath([u8; 64]);

impl FsPath {
    pub fn new(val: [u8; 64]) -> FsPath {
        FsPath(val)
    }
}

impl fmt::Debug for FsPath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, r#"FsPath::new({:?})"#, &self.0[..])
    }
}

impl Default for FsPath {
    fn default() -> FsPath {
        FsPath::new([0; 64])
    }
}

impl PartialEq for FsPath {
    /// FsTitle compares equal if the elements match through the first null byte
    fn eq(&self, other: &FsPath) -> bool {
        for i in 0..64 {
            if self.0[i] != other.0[i] {
                return false;
            } else if self.0[i] == 0 {
                return true;
            }
        }
        true
    }
}

impl Eq for FsPath {}

pub struct Ext2<T: disk::Disk>(T);

/// Ext2 Filesystem
impl<T: disk::Disk> Ext2<T> {
    pub fn open(disk: T) -> io::Result<Ext2<T>> {
        Ok(Ext2(disk))
    }

    fn read_block(&mut self, blocknum: u32, buf: &mut [u8], sb: &Superblock) -> io::Result<()> {
        let block_size = sb.block_size();
        if buf.len() < block_size as usize {
            panic!("Must provide a buffer of size {}", block_size);
        }
        let sectors_per_block = block_size / 512;
        let start_sector = sectors_per_block * blocknum;
        for i in 0..sectors_per_block {
            let start = (i * 512) as usize;
            let end = start + 512;
            self.0
                .read_sector((start_sector + i) as u64, &mut buf[start..end])?;
        }
        Ok(())
    }

    pub fn superblock(&mut self) -> io::Result<Superblock> {
        let mut block = [0; 1024];
        self.0.read_sector(2, &mut block[..512])?;
        self.0.read_sector(3, &mut block[512..])?;
        Superblock::new(&block[..])
    }

    pub fn first_descriptor_block(&mut self, sb: &Superblock) -> u32 {
        if sb.block_size() == 1024 {
            2
        } else {
            1
        }
    }

    pub fn get_block_group_descriptor(
        &mut self,
        groupnum: u32,
        sb: &Superblock,
    ) -> io::Result<Option<BlockGroupDescriptor>> {
        if groupnum > sb.block_group_count() {
            Ok(None)
        } else {
            let bs = sb.block_size();
            let descriptor_block = (groupnum * 32) / bs + self.first_descriptor_block(&sb);
            let offset = ((groupnum * 32) % bs) as usize;
            let mut buf = vec![0; bs as usize];
            self.read_block(descriptor_block, &mut buf, &sb)?;
            Ok(Some(BlockGroupDescriptor::new(&buf[offset..offset + 32])?))
        }
    }

    pub fn get_inode(&mut self, inode: u32, sb: &Superblock) -> io::Result<Option<Inode>> {
        let (igroup, ioffset) = sb.locate_inode(inode);
        let descriptor = self.get_block_group_descriptor(igroup, &sb)?.unwrap(); // Should check for valid Inode
        let iblock = descriptor.bg_inode_table + (ioffset * sb.inode_size()) / sb.block_size();
        let iblock_offset = ((ioffset * sb.inode_size()) % sb.block_size()) as usize;
        let mut buf = vec![0; sb.block_size() as usize];
        self.read_block(iblock, &mut buf[..], &sb)?;
        Ok(Some(Inode::new(
            &buf[iblock_offset..iblock_offset + sb.inode_size() as usize],
        )?))
    }

    pub fn get_root_directory(&mut self, sb: &Superblock) -> io::Result<Inode> {
        self.get_inode(2, &sb).map(|optinode| optinode.unwrap())
    }

    pub fn get_block_ptr(&mut self, inode: &Inode, ptr: u32, sb: &Superblock) -> io::Result<u32> {
        let blocksize = sb.block_size();
        let inodes_per_block = blocksize / sb.inode_size();
        let direct_limit = 12;
        let single_limit = direct_limit + inodes_per_block;
        let double_limit = single_limit + inodes_per_block * inodes_per_block;
        let triple_limit = double_limit + inodes_per_block * inodes_per_block * inodes_per_block;

        let node = if ptr < direct_limit {
             inode.i_block.0[ptr as usize]
        } else if ptr < single_limit {
            let mut buf = vec![0; sb.block_size() as usize];
            self.read_block(inode.i_block.1, &mut buf, sb)?;
            let ptr = (ptr - 12) as usize;
            LE::read_u32(&buf[ptr * 4..(ptr + 1) * 4])
        } else if ptr < double_limit {
            unimplemented!()
        } else if ptr < triple_limit {
            unimplemented!()
        } else {
            0
        };
        match node {
            0 => Err(io::Error::new(io::ErrorKind::Other, "Not found")),
            x => Ok(x),
        }
    }

    /// TODO: Handle multiple block directories.
    pub fn read_dir(
        &mut self,
        inode: &Inode,
        sb: &Superblock,
    ) -> Option<io::Result<Vec<DirEntry>>> {
        match inode.file_type() {
            FileType::Directory => {
                let mut buf = vec![0; sb.block_size() as usize];
                let mut vec = Vec::new();
                Some(self.read_block(inode.i_block.0[0], &mut buf, sb).map(|()| {
                    let mut start: usize = 0;
                    while start < sb.block_size() as usize {
                        let entry = DirEntry::new(&buf[start..sb.block_size() as usize]);
                        start += entry.rec_len as usize;
                        if entry.inode == 0 {
                            assert_eq!(start, sb.block_size() as usize)
                        }
                        vec.push(entry);
                    }
                    vec
                }))
            }
            _ => None,
        }
    }

    /// Todo: Fix calculation of blocks to be read.
    pub fn read_file_block(&mut self, inode: &Inode, buf: &mut [u8], idx: u32, sb: &Superblock) -> io::Result<usize> {
        match inode.file_type() {
            FileType::File => {
                let ptr = self.get_block_ptr(inode, idx, sb)?;
                self.read_block(ptr, buf, sb)
                    .map(|()| sb.block_size() as usize)
            }
            _ => Err(io::Error::new(io::ErrorKind::Other, "Not found")),
        }
    }
}

/// Ext2 superblock struct.
///
/// See documentation at http://www.nongnu.org/ext2-doc/ext2.html#SUPERBLOCK
#[repr(C)]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Superblock {
    pub s_inodes_count: u32,
    pub s_blocks_count: u32,
    pub s_r_blocks_count: u32,
    pub s_free_blocks_count: u32,
    pub s_free_inodes_count: u32,
    pub s_first_data_block: u32,
    pub s_log_block_size: u32,
    pub s_log_frag_size: u32,
    pub s_blocks_per_group: u32,
    pub s_frags_per_group: u32,
    pub s_inodes_per_group: u32,
    pub s_mtime: u32,
    pub s_wtime: u32,
    pub s_mnt_count: u16,
    pub s_max_mnt_count: u16,
    pub s_magic: u16,
    pub s_state: u16,
    pub s_errors: u16,
    pub s_minor_rev_level: u16,
    pub s_lastcheck: u32,
    pub s_checkinterval: u32,
    pub s_creator_os: u32,
    pub s_rev_level: u32,
    pub s_def_resuid: u16,
    pub s_def_resgid: u16,
    // EDX2_DYNAMIC_REV specific
    pub s_first_ino: u32,
    pub s_inode_size: u16,
    pub s_block_group_nr: u16,
    pub s_feature_compat: u32,
    pub s_feature_incompat: u32,
    pub s_feature_ro_compat: u32,
    pub s_uuid: uuid::Uuid,
    pub s_volume_name: [u8; 16],
    pub s_last_mounted: FsPath,
    pub s_algo_bitmap: u32,
    // Performance hints
    pub s_prealloc_blocks: u8,
    pub s_prealloc_dir_blocks: u8,
    pub _align: (u8, u8),
    // Journaling support
    pub s_journal_uuid: uuid::Uuid,
    pub s_journal_inum: u32,
    pub s_journal_dev: u32,
    pub s_last_orphan: u32,
    // Directory indexing support
    pub s_hash_seed: [u32; 4],
    pub s_def_hash_version: u8,
    pub _hash_version_align: (u8, u8, u8),
    // Other options
    pub s_default_mount_options: u32,
    pub s_first_meta_bg: u32,
}

impl Superblock {
    pub fn new(data: &[u8]) -> io::Result<Superblock> {
        Ok(Superblock {
            s_inodes_count: LE::read_u32(&data[0..4]),
            s_blocks_count: LE::read_u32(&data[4..8]), //LE::read_u32(&d[])?,
            s_r_blocks_count: LE::read_u32(&data[8..12]), // LE::read_u32(&d[])?,
            s_free_blocks_count: LE::read_u32(&data[12..16]),
            s_free_inodes_count: LE::read_u32(&data[16..20]),
            s_first_data_block: LE::read_u32(&data[20..24]),
            s_log_block_size: LE::read_u32(&data[24..28]),
            s_log_frag_size: LE::read_u32(&data[28..32]),
            s_blocks_per_group: LE::read_u32(&data[32..36]),
            s_frags_per_group: LE::read_u32(&data[36..40]),
            s_inodes_per_group: LE::read_u32(&data[40..44]),
            s_mtime: LE::read_u32(&data[44..48]),
            s_wtime: LE::read_u32(&data[48..52]),
            s_mnt_count: LE::read_u16(&data[52..54]),
            s_max_mnt_count: LE::read_u16(&data[54..56]),
            s_magic: LE::read_u16(&data[56..58]),
            s_state: LE::read_u16(&data[58..60]),
            s_errors: LE::read_u16(&data[60..62]),
            s_minor_rev_level: LE::read_u16(&data[62..64]),
            s_lastcheck: LE::read_u32(&data[64..68]),
            s_checkinterval: LE::read_u32(&data[68..72]),
            s_creator_os: LE::read_u32(&data[72..76]),
            s_rev_level: LE::read_u32(&data[76..80]),
            s_def_resuid: LE::read_u16(&data[80..82]),
            s_def_resgid: LE::read_u16(&data[82..84]),
            s_first_ino: LE::read_u32(&data[84..88]),
            s_inode_size: LE::read_u16(&data[88..90]),
            s_block_group_nr: LE::read_u16(&data[90..92]),
            s_feature_compat: LE::read_u32(&data[92..96]),
            s_feature_incompat: LE::read_u32(&data[96..100]),
            s_feature_ro_compat: LE::read_u32(&data[100..104]),
            s_uuid: uuid::Uuid::from_slice(&data[104..120]).unwrap(),
            s_volume_name: array16(&data[120..136]),
            s_last_mounted: FsPath::new(array64(&data[136..200])),
            s_algo_bitmap: LE::read_u32(&data[200..204]),
            // Performance hints
            s_prealloc_blocks: data[204],
            s_prealloc_dir_blocks: data[205],
            _align: (data[206], data[207]),
            // Journaling support
            s_journal_uuid: uuid::Uuid::from_slice(&data[208..224]).unwrap(),
            s_journal_inum: LE::read_u32(&data[224..228]),
            s_journal_dev: LE::read_u32(&data[228..232]),
            s_last_orphan: LE::read_u32(&data[232..236]),
            // Directory indexing support
            s_hash_seed: [
                LE::read_u32(&data[236..240]),
                LE::read_u32(&data[240..244]),
                LE::read_u32(&data[244..248]),
                LE::read_u32(&data[248..252]),
            ],
            s_def_hash_version: data[252],
            _hash_version_align: (data[253], data[254], data[255]),
            // Other options
            s_default_mount_options: LE::read_u32(&data[256..260]),
            s_first_meta_bg: LE::read_u32(&data[260..264]),
        })
    }

    pub fn block_group_count(&self) -> u32 {
        self.s_inodes_count / self.s_inodes_per_group
            + if self.s_inodes_count % self.s_inodes_per_group == 0 {
                0
            } else {
                1
            }
    }

    fn locate_inode(&self, inode: u32) -> (u32, u32) {
        let index = (inode - 1) / self.s_inodes_per_group;
        let offset = (inode - 1) % self.s_inodes_per_group;
        (index, offset)
    }

    pub fn block_size(&self) -> u32 {
        1024 << self.s_log_block_size
    }

    fn inode_size(&self) -> u32 {
        if self.s_rev_level > 0 {
            self.s_inode_size as u32
        } else {
            128
        }
    }
}

#[repr(C)]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BlockGroupDescriptor {
    pub bg_block_bitmap: u32,
    pub bg_inode_bitmap: u32,
    pub bg_inode_table: u32,
    pub bg_free_blocks_count: u16,
    pub bg_free_inodes_count: u16,
    pub bg_used_dirs_count: u16,
    pub bg_pad: u16,
    pub bg_reserved: [u8; 12],
}

impl BlockGroupDescriptor {
    pub fn new(data: &[u8]) -> io::Result<BlockGroupDescriptor> {
        if data.len() != 32 {
            panic!("BlockGroupDescriptors must be 32 bytes in length");
        }
        Ok(BlockGroupDescriptor {
            bg_block_bitmap: LE::read_u32(&data[0..4]),
            bg_inode_bitmap: LE::read_u32(&data[4..8]),
            bg_inode_table: LE::read_u32(&data[8..12]),
            bg_free_blocks_count: LE::read_u16(&data[12..14]),
            bg_free_inodes_count: LE::read_u16(&data[14..16]),
            bg_used_dirs_count: LE::read_u16(&data[16..18]),
            bg_pad: LE::read_u16(&data[18..20]),
            bg_reserved: array12(&data[20..32]),
        })
    }
}

#[repr(C)]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Inode {
    pub i_mode: u16,
    pub i_uid: u16,
    pub i_size: u32,
    pub i_atime: u32,
    pub i_ctime: u32,
    pub i_mtime: u32,
    pub i_dtime: u32,
    pub i_gid: u16,
    pub i_links_count: u16,
    pub i_blocks: u32,
    pub i_flags: u32,
    pub i_osd1: u32,
    pub i_block: ([u32; 12], u32, u32, u32),
    pub i_generation: u32,
    pub i_file_acl: u32,
    pub i_dir_acl: u32,
    pub i_faddr: u32,
    pub i_osd2: [u8; 12],
}

impl Inode {
    pub fn new(data: &[u8]) -> io::Result<Inode> {
        Ok(Inode {
            i_mode: LE::read_u16(&data[0..2]),
            i_uid: LE::read_u16(&data[2..4]),
            i_size: LE::read_u32(&data[4..8]),
            i_atime: LE::read_u32(&data[8..12]),
            i_ctime: LE::read_u32(&data[12..16]),
            i_mtime: LE::read_u32(&data[16..20]),
            i_dtime: LE::read_u32(&data[20..24]),
            i_gid: LE::read_u16(&data[24..26]),
            i_links_count: LE::read_u16(&data[26..28]),
            i_blocks: LE::read_u32(&data[28..32]),
            i_flags: LE::read_u32(&data[32..36]),
            i_osd1: LE::read_u32(&data[36..40]),
            i_block: (
                [
                    LE::read_u32(&data[40..44]),
                    LE::read_u32(&data[44..48]),
                    LE::read_u32(&data[52..56]),
                    LE::read_u32(&data[48..52]),
                    LE::read_u32(&data[56..60]),
                    LE::read_u32(&data[60..64]),
                    LE::read_u32(&data[64..68]),
                    LE::read_u32(&data[68..72]),
                    LE::read_u32(&data[72..76]),
                    LE::read_u32(&data[76..80]),
                    LE::read_u32(&data[80..84]),
                    LE::read_u32(&data[84..88]),
                ],
                LE::read_u32(&data[88..92]),
                LE::read_u32(&data[92..96]),
                LE::read_u32(&data[96..100]),
            ),
            i_generation: LE::read_u32(&data[100..104]),
            i_file_acl: LE::read_u32(&data[104..108]),
            i_dir_acl: LE::read_u32(&data[108..112]),
            i_faddr: LE::read_u32(&data[112..116]),
            i_osd2: array12(&data[116..128]),
        })
    }

    pub fn file_type(&self) -> FileType {
        use FileType::*;
        match self.i_mode & 0xf000 {
            0x1000 => FIFO,
            0x2000 => CharDev,
            0x4000 => Directory,
            0x6000 => BlockDev,
            0x8000 => File,
            0xa000 => SymLink,
            0xc000 => UnixSocket,
            x => panic!("Invalid file_type: 0x{:x}", x),
        }
    }

    pub fn size(&self) -> u64 {
        (self.i_dir_acl as u64) << 32 + self.i_size as u64
    }

    pub fn block_count(&self, sb: &Superblock) -> u32 {
        self.i_blocks / 2 << sb.s_log_block_size
    }
}

/// Can't make this repr(C) because the size would be variable
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DirEntry {
    pub inode: u32,
    pub rec_len: u16,
    pub name_len: u8,
    pub file_type: u8,
    pub name: Vec<u8>, // Should this be OsString?
}

impl DirEntry {
    pub fn new(data: &[u8]) -> DirEntry {
        DirEntry {
            inode: LE::read_u32(&data[0..4]),
            rec_len: LE::read_u16(&data[4..6]),
            name_len: data[6],
            file_type: data[7],
            name: data[8..8 + data[6] as usize].to_vec(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FileType {
    Unknown = 0,
    File = 1,
    Directory = 2,
    CharDev = 3,
    BlockDev = 4,
    FIFO = 5,
    UnixSocket = 6,
    SymLink = 7,
}

#[cfg(test)]
mod tests {

    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}

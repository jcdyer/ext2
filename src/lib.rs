//! TODO:
//!
//! * Change io::Result<Option<T>> to io::Result<T> using
//!   io::ErrorKind::NotFound in place of Ok(None)
//! * Implement write

extern crate byteorder;
#[macro_use]
extern crate serde_derive;

use std::cmp::PartialEq;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path};
use std::sync::Mutex;
use byteorder::{ByteOrder, LE};

mod disk;
mod array;
pub mod handle;

pub use disk::Disk;
pub struct Ext2<T: disk::Disk>(Mutex<T>);

/// Ext2 Filesystem
impl<T: disk::Disk> Ext2<T> {
    pub fn new(disk: T) -> io::Result<Ext2<T>> {
        Ok(Ext2(Mutex::new(disk)))
    }

    pub fn open<P: AsRef<Path>>(&self, path: P) -> io::Result<handle::Ext2Handle<'_, T>> {
        let superblock = self.superblock()?;
        if let Some(inode) = self.get_inode_from_abspath(&path, &superblock)? {
            Ok(handle::Ext2Handle::new(self, &path, superblock, inode))
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("{:?} not found", path.as_ref()),
            ))
        }
    }

    pub fn block_size(&self) -> io::Result<u32> {
        self.superblock().map(|sb| sb.block_size())
    }

    fn read_block(&self, blocknum: u32, buf: &mut [u8], sb: &Superblock) -> io::Result<()> {
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
                .lock()
                .expect("Got a poisoned mutex.  Cannot recover")
                .read_sector((start_sector + i) as u64, &mut buf[start..end])?;
        }
        Ok(())
    }

    fn superblock(&self) -> io::Result<Superblock> {
        let mut block = [0; 1024];
        {
            let mut disk = self.0
                .lock()
                .expect("Got a poisoned mutex.  Cannot recover");
            disk.read_sector(2, &mut block[..512])?;
            disk.read_sector(3, &mut block[512..])?;
        }
        Superblock::new(&block)
    }

    fn first_descriptor_block(&self, sb: &Superblock) -> u32 {
        if sb.block_size() == 1024 {
            2
        } else {
            1
        }
    }

    fn get_block_group_descriptor(
        &self,
        groupnum: u32,
        sb: &Superblock,
    ) -> io::Result<Option<BlockGroupDescriptor>> {
        if groupnum > sb.block_group_count() {
            Ok(None)
        } else {
            let bs = sb.block_size();
            let descriptor_block = (groupnum * 32) / bs + self.first_descriptor_block(sb);
            let offset = ((groupnum * 32) % bs) as usize;
            let mut buf = vec![0; bs as usize];
            self.read_block(descriptor_block, &mut buf, sb)?;
            Ok(Some(BlockGroupDescriptor::new(&buf[offset..offset + 32])?))
        }
    }

    fn get_inode(&self, iptr: u32, sb: &Superblock) -> io::Result<Option<Inode>> {
        let (igroup, ioffset) = sb.locate_inode(iptr);
        let descriptor = self.get_block_group_descriptor(igroup, sb)?.unwrap();
        let iblock = descriptor.bg_inode_table + (ioffset * sb.inode_size()) / sb.block_size();
        let iblock_offset = ((ioffset * sb.inode_size()) % sb.block_size()) as usize;
        let mut buf = vec![0; sb.block_size() as usize];
        self.read_block(iblock, &mut buf[..], sb)?;
        Ok(Some(Inode::new(
            &buf[iblock_offset..iblock_offset + sb.inode_size() as usize],
        )?))
    }

    fn get_root_directory(&self, sb: &Superblock) -> io::Result<Inode> {
        self.get_inode(2, sb).map(|optinode| optinode.unwrap())
    }

    fn get_inode_from_abspath<P: AsRef<Path>>(
        &self,
        path: P,
        sb: &Superblock,
    ) -> io::Result<Option<Inode>> {
        let path = path.as_ref();
        assert!(
            path.is_absolute(),
            "This library only supports absolute paths."
        );
        let mut inode = Inode::default();
        for component in path.components() {
            match component {
                Component::RootDir => inode = self.get_root_directory(sb)?,
                Component::Prefix(_) => {
                    panic!("Prefix found in path.  I don't speak Windows");
                }
                component => {
                    inode = match self.get_inode_in_dir(&inode, component.as_os_str(), sb)? {
                        Some(inode) => inode,
                        None => return Ok(None),
                    };
                }
            }
        }
        Ok(Some(inode))
    }

    fn get_inode_in_dir(
        &self,
        inode: &Inode,
        filename: &OsStr,
        sb: &Superblock,
    ) -> io::Result<Option<Inode>> {
        if let Some(entries) = self.read_dir(inode, sb)? {
            for entry in entries {
                if entry.name == filename {
                    return self.get_inode(entry.inode, sb);
                }
            }
        }
        Ok(None)
    }

    fn find_ptr(&self, nextptr: u32, offset: u32, level: u32, sb: &Superblock) -> io::Result<u32> {
        if level == 0 {
            Ok(nextptr)
        } else {
            let mut buf = vec![0; sb.block_size() as usize];
            if nextptr == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "unexpectedly missing block",
                ));
            }
            self.read_block(nextptr, &mut buf, sb)?;
            let ptrs_per_block = sb.block_size() / 4;
            let ptrs_per_bucket = ptrs_per_block.pow(level);
            let skipped_buckets = offset / ptrs_per_bucket;
            let next_offset = offset - ptrs_per_bucket * skipped_buckets;
            let nextptr =
                LE::read_u32(&buf[next_offset as usize * 4..(next_offset as usize + 1) * 4]);
            self.find_ptr(nextptr, next_offset, level - 1, sb)
        }
    }

    fn get_block_ptr(&self, inode: &Inode, idx: u32, sb: &Superblock) -> io::Result<u32> {
        let blocksize = sb.block_size();
        let inodes_per_block = blocksize / sb.inode_size();
        let direct_limit = 12;
        let single_limit = direct_limit + inodes_per_block;
        let double_limit = single_limit + inodes_per_block * inodes_per_block;
        let triple_limit = double_limit + inodes_per_block * inodes_per_block * inodes_per_block;

        let node = if idx < direct_limit {
            let level = 0;
            self.find_ptr(inode.i_block.0[idx as usize], 0, level, sb)?
        } else if idx < single_limit {
            let level = 1;
            self.find_ptr(inode.i_block.1, idx - direct_limit, level, sb)?
        } else if idx < double_limit {
            let level = 2;
            self.find_ptr(inode.i_block.2, idx - single_limit, level, sb)?
        } else if idx < triple_limit {
            let level = 3;
            self.find_ptr(inode.i_block.3, idx - double_limit, level, sb)?
        } else {
            0
        };
        match node {
            //0 => Err(io::Error::new(io::ErrorKind::NotFound, "End of the line")),
            x => Ok(x),
        }
    }

    /// TODO: Handle multiple block directories.
    fn read_dir(&self, inode: &Inode, sb: &Superblock) -> io::Result<Option<Vec<DirEntry>>> {
        match inode.file_type() {
            FileType::Directory => {
                let mut buf = vec![0; sb.block_size() as usize];
                let mut vec = Vec::new();
                self.read_block(inode.i_block.0[0], &mut buf, sb)?;
                let mut start: usize = 0;
                while start < sb.block_size() as usize {
                    let entry = DirEntry::new(&buf[start..sb.block_size() as usize]);
                    start += entry.rec_len as usize;
                    if entry.inode == 0 {
                        assert_eq!(start, sb.block_size() as usize)
                    }
                    vec.push(entry);
                }
                Ok(Some(vec))
            }
            _ => Ok(None),
        }
    }

    /// Todo: Fix calculation of blocks to be read.
    fn read_inode_data_block(
        &self,
        inode: &Inode,
        buf: &mut [u8],
        idx: u32,
        sb: &Superblock,
    ) -> io::Result<usize> {
        match inode.file_type() {
            FileType::File => {
                let ptr = self.get_block_ptr(inode, idx, sb)?;
                if ptr == 0 {
                    Ok(0)
                } else {
                    self.read_block(ptr, buf, sb)
                        .map(|()| sb.block_size() as usize)
                }
            }
            _ => Err(io::Error::new(io::ErrorKind::Other, "Not found")),
        }
    }
}

/// Ext2 superblock struct.
///
/// See documentation at http://www.nongnu.org/ext2-doc/ext2.html#SUPERBLOCK
#[repr(C)]
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
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
    pub s_uuid: [u8; 16],
    pub s_volume_name: [u8; 16],
    pub s_last_mounted: FsPath,
    pub s_algo_bitmap: u32,
    // Performance hints
    pub s_prealloc_blocks: u8,
    pub s_prealloc_dir_blocks: u8,
    pub _align: (u8, u8),
    // Journaling support
    pub s_journal_uuid: [u8; 16],
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
            s_uuid: array::array16(&data[104..120]),
            s_volume_name: array::array16(&data[120..136]),
            s_last_mounted: FsPath::new(array::array64(&data[136..200])),
            s_algo_bitmap: LE::read_u32(&data[200..204]),
            // Performance hints
            s_prealloc_blocks: data[204],
            s_prealloc_dir_blocks: data[205],
            _align: (data[206], data[207]),
            // Journaling support
            s_journal_uuid: array::array16(&data[208..224]),
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

    pub fn locate_inode(&self, inode: u32) -> (u32, u32) {
        let index = (inode - 1) / self.s_inodes_per_group;
        let offset = (inode - 1) % self.s_inodes_per_group;
        (index, offset)
    }

    pub fn block_size(&self) -> u32 {
        1024 << self.s_log_block_size
    }

    pub fn inode_size(&self) -> u32 {
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
            bg_reserved: array::array12(&data[20..32]),
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
                    LE::read_u32(&data[48..52]),
                    LE::read_u32(&data[52..56]),
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
            i_osd2: array::array12(&data[116..128]),
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
        ((self.i_dir_acl as u64) << 32) + self.i_size as u64
    }

    pub fn block_count(&self, sb: &Superblock) -> u32 {
        self.i_blocks / (2 << sb.s_log_block_size)
    }
}

/// Can't make this repr(C) because the size would be variable
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DirEntry {
    pub inode: u32,
    pub rec_len: u16,
    pub name_len: u8,
    pub file_type: u8,
    pub name: OsString, // Should this be OsString?
}

impl DirEntry {
    pub fn new(data: &[u8]) -> DirEntry {
        DirEntry {
            inode: LE::read_u32(&data[0..4]),
            rec_len: LE::read_u16(&data[4..6]),
            name_len: data[6],
            file_type: data[7],
            name: OsStr::from_bytes(&data[8..8 + data[6] as usize]).to_os_string(),
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

#[derive(Clone, Deserialize, Serialize)]
pub struct FsPath([[u8; 32]; 2]);

impl FsPath {
    pub fn new(val: [u8; 64]) -> FsPath {
        FsPath(unsafe { std::mem::transmute(val) })
    }

    /// Iterator over the bytes before the first null byte.
    pub fn bytes(&self) -> impl Iterator<Item = u8> {
        self.0
            .concat()
            .into_iter()
            .take_while(|&x| x != 0)
    }
}

impl fmt::Debug for FsPath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "FsPath::new({:?}{:?})", &self.0[0], &self.0[1])
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
        self.bytes().eq(other.bytes())
    }
}

impl Eq for FsPath {}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use super::*;

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
                116, 50, 47, 109, 110, 116, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
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
            i_block: ([13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24], 25, 0, 0),
            i_generation: 1337774247,
            i_file_acl: 0,
            i_dir_acl: 0,
            i_faddr: 0,
            i_osd2: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        };
        assert_eq!(inode, obama_portrait);
    }

    #[test]
    fn get_pattern_inode() {
        let fs = File::open("./basic.ext2").and_then(Ext2::new).unwrap();
        let superblock = fs.superblock().unwrap();
        let inode = fs.get_inode_from_abspath("/sub/pattern/test_pattern.txt", &superblock)
            .unwrap()
            .unwrap();
        let test_pattern = Inode {
            i_mode: 33188,
            i_uid: 0,
            i_size: 65536,
            i_atime: 1537711032,
            i_ctime: 1537711032,
            i_mtime: 1537711032,
            i_dtime: 0,
            i_gid: 0,
            i_links_count: 1,
            i_blocks: 136,
            i_flags: 0,
            i_osd1: 1,
            i_block: ([32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43], 51, 0, 0),
            i_generation: 2497518845,
            i_file_acl: 0,
            i_dir_acl: 0,
            i_faddr: 0,
            i_osd2: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        };
        assert_eq!(inode, test_pattern);
        let mut buf = vec![0xff; 4096];
        fs.read_block(32, &mut buf, &superblock).unwrap();
        assert_eq!(&mut buf[..8], b"0 ......");
        fs.read_block(33, &mut buf, &superblock).unwrap();
        assert_eq!(&mut buf[..8], b"1 ......");
        fs.read_block(34, &mut buf, &superblock).unwrap();
        assert_eq!(&mut buf[..8], b"2 ......");
        fs.read_block(35, &mut buf, &superblock).unwrap();
        assert_eq!(&mut buf[..8], b"3 ......");
    }
}

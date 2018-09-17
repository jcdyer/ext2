extern crate byteorder;
extern crate uuid;

use std::cmp::PartialEq;
use std::fmt;
use std::io;
use byteorder::{ByteOrder, LE};

mod disk;

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

    pub fn superblock(&mut self) -> io::Result<Superblock> {
        let mut block = [0; 4096];
        self.0.read_block(0, &mut block)?;
        Superblock::new(&block[1024..2048])
    }

    pub fn block_group_descriptor_table(
        &mut self,
        sb: &Superblock,
    ) -> io::Result<Vec<BlockGroupDescriptor>> {
        let ct = sb.block_group_count();
        let bs = sb.block_size();
        let mut block = vec![0; bs as usize];
        self.0.read_block(1, &mut block)?;

        if ct * 32 > bs {
            panic!("Handling multi-block not implemented");
        }
        let mut vec = Vec::with_capacity(ct as usize);
        let mut offset = 0;
        for _i in 0..ct {
            vec.push(BlockGroupDescriptor::new(&block[offset..offset + 32])?);
            offset += 32;
        }
        Ok(vec)
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
        panic!("Requires an input length of {}", input.len());
    }
    for i in 0..input.len() {
        output[i] = input[i];
    }
}

impl Superblock {
    pub fn new(d: &[u8]) -> io::Result<Superblock> {
        Ok(Superblock {
            s_inodes_count: LE::read_u32(&d[0..4]),
            s_blocks_count: LE::read_u32(&d[4..8]), //LE::read_u32(&d[])?,
            s_r_blocks_count: LE::read_u32(&d[8..12]), // LE::read_u32(&d[])?,
            s_free_blocks_count: LE::read_u32(&d[12..16]),
            s_free_inodes_count: LE::read_u32(&d[16..20]),
            s_first_data_block: LE::read_u32(&d[20..24]),
            s_log_block_size: LE::read_u32(&d[24..28]),
            s_log_frag_size: LE::read_u32(&d[28..32]),
            s_blocks_per_group: LE::read_u32(&d[32..36]),
            s_frags_per_group: LE::read_u32(&d[36..40]),
            s_inodes_per_group: LE::read_u32(&d[40..44]),
            s_mtime: LE::read_u32(&d[44..48]),
            s_wtime: LE::read_u32(&d[48..52]),
            s_mnt_count: LE::read_u16(&d[52..54]),
            s_max_mnt_count: LE::read_u16(&d[54..56]),
            s_magic: LE::read_u16(&d[56..58]),
            s_state: LE::read_u16(&d[58..60]),
            s_errors: LE::read_u16(&d[60..62]),
            s_minor_rev_level: LE::read_u16(&d[62..64]),
            s_lastcheck: LE::read_u32(&d[64..68]),
            s_checkinterval: LE::read_u32(&d[68..72]),
            s_creator_os: LE::read_u32(&d[72..76]),
            s_rev_level: LE::read_u32(&d[76..80]),
            s_def_resuid: LE::read_u16(&d[80..82]),
            s_def_resgid: LE::read_u16(&d[82..84]),
            s_first_ino: LE::read_u32(&d[84..88]),
            s_inode_size: LE::read_u16(&d[88..90]),
            s_block_group_nr: LE::read_u16(&d[90..92]),
            s_feature_compat: LE::read_u32(&d[92..96]),
            s_feature_incompat: LE::read_u32(&d[96..100]),
            s_feature_ro_compat: LE::read_u32(&d[100..104]),
            s_uuid: uuid::Uuid::from_slice(&d[104..120]).unwrap(),
            s_volume_name: array16(&d[120..136]),
            s_last_mounted: FsPath::new(array64(&d[136..200])),
            s_algo_bitmap: LE::read_u32(&d[200..204]),
            // Performance hints
            s_prealloc_blocks: d[204],
            s_prealloc_dir_blocks: d[205],
            _align: (d[206], d[207]),
            // Journaling support
            s_journal_uuid: uuid::Uuid::from_slice(&d[208..224]).unwrap(),
            s_journal_inum: LE::read_u32(&d[224..228]),
            s_journal_dev: LE::read_u32(&d[228..232]),
            s_last_orphan: LE::read_u32(&d[232..236]),
            // Directory indexing support
            s_hash_seed: [
                LE::read_u32(&d[236..240]),
                LE::read_u32(&d[240..244]),
                LE::read_u32(&d[244..248]),
                LE::read_u32(&d[248..252]),
            ],
            s_def_hash_version: d[252],
            _hash_version_align: (d[253], d[254], d[255]),
            // Other options
            s_default_mount_options: LE::read_u32(&d[256..260]),
            s_first_meta_bg: LE::read_u32(&d[260..264]),
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

    pub fn block_size(&self) -> u32 {
        1024 << self.s_log_block_size
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
    pub fn new(d: &[u8]) -> io::Result<BlockGroupDescriptor> {
        if d.len() != 32 {
            panic!("BlockGroupDescriptors must be 32 bytes in length");
        }
        Ok(BlockGroupDescriptor {
            bg_block_bitmap: LE::read_u32(&d[0..4]),
            bg_inode_bitmap: LE::read_u32(&d[4..8]),
            bg_inode_table: LE::read_u32(&d[8..12]),
            bg_free_blocks_count: LE::read_u16(&d[12..14]),
            bg_free_inodes_count: LE::read_u16(&d[14..16]),
            bg_used_dirs_count: LE::read_u16(&d[16..18]),
            bg_pad: LE::read_u16(&d[18..20]),
            bg_reserved: array12(&d[20..32]),
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}

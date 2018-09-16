extern crate uuid;

mod disk;

struct<T: Disk> Ext2(T);


/// Ext2 superblock struct.
///
/// See documentation at http://www.nongnu.org/ext2-doc/ext2.html#SUPERBLOCK
#[repr(C)]
struct Superblock {
    s_inodes_count: u32,
    s_blocks_count: u32,
    s_r_blocks_count: u32,
    s_free_blocks_count: u32,
    s_free_inodes_count: u32,
    s_first_data_block: u32,
    s_log_block_size: u32,
    s_blocks_per_group:u32,
    s_frags_per_group:  u32,
    s_inodes_per_group: u32,
    s_mtime: u32, s_wtime: u32,
    s_mnt_count: u16,
    s_max_mnt_count: u16,
    s_magic: u16,
    s_state: u16,
    s_errors: u16,
    s_minor_rev_level: u16,
    s_lastcheck: u32,
    s_checkinterval: u32,
    s_creator_os: u32
        , s_rev_level: u32,
    s_def_resuid: u16,
    s_def_resgid: u16

    // EDX2_DYNAMIC_REV specific
    s_first_ino: u32,
    s_inode_size: u16,
        s_block_group_nr: u16,
        s_feature_compat: u32,
        s_feature_incompat: u32,
        s_feature_ro_compat: u32,
        s_uuid: uuid::Uuid,
        s_volume_name: [u8; 16],
        s_last_mounted: [u8; 64],
        s_algo_bitmap: u32,
    // Performance hints
        s_prealloc_blocks: u8,
        s_prealloc_dir_blocks: u8,
        _align: u16,
    // Journaling support
    s_journal_uuid: uuid::Uuid,
        s_journal_inum: u32,
        s_journal_dev: u32,
        s_last_orphan: u32,
        // Directory indexing support
        s_hash_seed: [u32; 4],
        s_def_hash_version: u8,
        _hash_version_align: [u8; 3],
    // Other options
        s_default_mount_options: u32,
        s_first_meta_bg: u32,
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}

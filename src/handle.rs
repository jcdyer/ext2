use std::io;
use std::path::{Path, PathBuf};
use super::{Ext2, Inode, Superblock};
use super::disk;

pub struct Ext2Handle<'fs, T: disk::Disk + 'fs> {
    fs: &'fs Ext2<T>,
    superblock: Superblock,
    path: PathBuf,
    inode: Inode,
    pos: u64,
}

impl<'fs, T: disk::Disk + 'fs> Ext2Handle<'fs, T> {
    // HERE
    pub fn new<P: AsRef<Path>>(
        fs: &'fs Ext2<T>,
        path: P,
        superblock: Superblock,
        inode: Inode,
    ) -> Ext2Handle<'fs, T> {
        Ext2Handle {
            fs,
            superblock,
            path: path.as_ref().to_owned(),
            inode,
            pos: 0,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn size(&self) -> u64 {
        self.inode.size()
    }
}

impl<'fs, T: disk::Disk + 'fs> io::Seek for Ext2Handle<'fs, T> {
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        use io::SeekFrom::*;
        let (base, offset) = match pos {
            Start(n) => {
                self.pos = n;
                return Ok(n);
            }
            End(n) => (self.inode.size(), n),
            Current(n) => (self.pos, n),
        };
        let newpos = if offset >= 0 {
            base.checked_add(offset as u64)
        } else {
            base.checked_sub(offset.wrapping_neg() as u64)
        };
        match newpos {
            Some(n) => {
                self.pos = n;
                Ok(n)
            }
            None => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "attempted to seek to negative or overflowing position",
            )),
        }
    }
}

impl<'fs, T: disk::Disk + 'fs> io::Read for Ext2Handle<'fs, T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let bs = self.superblock.block_size() as u64;
        let blocknum = (self.pos / bs) as u32;
        let remaining = self.inode.size() - self.pos;
        let offset = (self.pos % bs) as usize;
        let read = if self.pos % bs == 0 && buf.len() >= bs as usize && remaining >= bs {
            // Read an entire block cleanly into the provided buffer.
            self.fs
                .read_inode_data_block(&self.inode, buf, blocknum, &self.superblock)?
        } else {
            // Read a partial block into the provided buffer, using an internal
            // buffer to read a whole block off the disk, with a length no
            // greater than the length of the provided buffer or the number of
            // bytes remaining in the file.
            let mut innerbuf = vec![0; bs as usize];
            let len = self.fs.read_inode_data_block(
                &self.inode,
                &mut innerbuf,
                blocknum,
                &self.superblock,
            )?;
            let len = len.min(buf.len());
            let len = len.min(remaining as usize);
            buf[..len].copy_from_slice(&innerbuf[offset..len + offset]);
            len
        };
        self.pos += read as u64;
        Ok(read)
    }
}

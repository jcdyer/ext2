use std::cmp;
use std::io::{self, prelude::*};

pub trait Disk {
    fn read_sector(&mut self, blocknum: u64, buf: &mut [u8]) -> io::Result<()>;
    fn write_sector(&mut self, blocknum: u64, buf: &[u8]) -> io::Result<()>;
    fn sync_disk(&mut self) -> io::Result<()>;
}

impl<T> Disk for T
where
    T: Read + Write + Seek,
{
    fn read_sector(&mut self, sector: u64, buf: &mut [u8]) -> io::Result<()> {
        let len = cmp::min(buf.len(), 512);
        self.seek(io::SeekFrom::Start(512 * sector))?;
        self.read_exact(&mut buf[..len])
    }

    fn write_sector(&mut self, sector: u64, buf: &[u8]) -> io::Result<()> {
        let len = cmp::min(buf.len(), 512);
        self.seek(io::SeekFrom::Start(512 * sector))?;
        self.write_all(&buf[..len])
    }

    fn sync_disk(&mut self) -> io::Result<()> {
        self.flush()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::io::Cursor;

    fn make_vec(len: usize) -> Vec<u8> {
        (0..len).enumerate().map(|(i, _)| (i / 64) as u8).collect()
    }

    #[test]
    fn can_read() {
        let v = make_vec(4096);
        let mut c = Cursor::new(v);
        let mut buf = [0; 512];
        c.read_sector(2, &mut buf).unwrap();
        assert_eq!(&buf[..4], &[16, 16, 16, 16]);
    }

    #[test]
    fn read_incomplete_sector() {
        let mut c = Cursor::new(make_vec(511));
        let mut buf = [0; 512];
        match c.read_sector(0, &mut buf) {
            Ok(()) => panic!("Unexpected success"),
            Err(err) => match err.kind() {
                io::ErrorKind::UnexpectedEof => {}
                kind => panic!("Got unexpected error: {:?}", kind),
            },
        };
    }

    #[test]
    fn i_like_big_bufs() {
        let mut c = Cursor::new(make_vec(1024));
        let mut buf = [0; 514];
        c.read_sector(0, &mut buf).unwrap();
        assert_eq!(&buf[510..], &[7, 7, 0, 0]);
    }

    #[test]
    fn read_nonexistent_block() {
        let mut c = Cursor::new(make_vec(512 * 4));
        let mut buf = [0; 512];
        match c.read_sector(5, &mut buf) {
            Ok(()) => panic!("Unexpected success"),
            Err(err) => match err.kind() {
                io::ErrorKind::UnexpectedEof => {}
                kind => panic!("Got unexpected error: {:?}", kind),
            },
        };
    }

    #[test]
    fn can_write() {
        let mut c = Cursor::new(make_vec(1024));
        let buf = [255; 512];
        c.write_sector(0, &buf).unwrap();
        c.sync_disk().unwrap();
        assert_eq!(&c.into_inner()[510..514], &[255, 255, 8, 8]);
    }
}

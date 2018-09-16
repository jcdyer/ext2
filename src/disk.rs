use std::cmp;
use std::io::{
    self,
    prelude::*
};

struct Block([u8]);


trait Disk {
    fn read_block(&mut self, blocknum: u64, buf: &mut [u8])-> io::Result<()>;
    fn write_block(&mut self, blocknum: u64, buf: &[u8]) -> io::Result<()>;
    fn sync_disk(&mut self) -> io::Result<()>;
}

impl <T> Disk for T
where T: Read + Write + Seek
{
    fn read_block(&mut self, blocknum: u64, buf: &mut [u8]) -> io::Result<()> {
        let len = cmp::min(buf.len(), 4096);
        self.seek(io::SeekFrom::Start(4096 * blocknum))?;
        self.read_exact(&mut buf[..len])
    }

    fn write_block(&mut self, blocknum: u64, buf: &[u8]) -> io::Result<()> {
        let len = cmp::min(buf.len(), 4096);
        self.seek(io::SeekFrom::Start(4096 * blocknum))?;
        self.write_all(&buf[..len])
    }

    fn sync_disk(&mut self) -> io::Result<()> {
        self.flush()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::io::{
        Cursor,
    };

    fn make_vec(len: usize) -> Vec<u8> {
        (0..len).enumerate().map(|(i, _)| (i / 128) as u8).collect()
    }

    #[test]
    fn can_read() {
        let v = make_vec(4096 * 4);
        let mut c = Cursor::new(v);
        let mut buf = [0; 4096];
        c.read_block(2, &mut buf).unwrap();
        assert_eq!(&buf[..4], &[64, 64, 64, 64]);
    }

    #[test]
    fn read_incomplete_sector() {
        let mut c = Cursor::new(make_vec(4095));
        let mut buf = [0; 4096];
        match c.read_block(0, &mut buf) {
            Ok(()) => panic!("Unexpected success"),
            Err(err) => match err.kind() {
                io::ErrorKind::UnexpectedEof => {},
                kind => panic!("Got unexpected error: {:?}", kind),
            },
        };
    }

    #[test]
    fn i_like_big_bufs() {
        let mut c = Cursor::new(make_vec(4096 * 2));
        let mut buf = [0; 4098];
        c.read_block(0, &mut buf).unwrap();
        assert_eq!(&buf[4094..], &[31, 31, 0, 0]);
    }

    #[test]
    fn read_nonexistent_block() {
        let mut c = Cursor::new(make_vec(4096 * 4));
        let mut buf = [0; 4098];
        match c.read_block(5, &mut buf) {
            Ok(()) => panic!("Unexpected success"),
            Err(err) => match err.kind() {
                io::ErrorKind::UnexpectedEof => {},
                kind => panic!("Got unexpected error: {:?}", kind),
            },
        };
    }

    #[test]
    fn can_write() {
        let mut c = Cursor::new(make_vec(4096 * 2));
        let buf = [255; 4096];
        c.write_block(0, &buf).unwrap();
        c.sync_disk().unwrap();
        assert_eq!(&c.into_inner()[4094..4098], &[255, 255, 32, 32]);
    }
}

extern crate ext2;
extern crate image;

use std::fs::File;
use std::io::{self, Read};

use image::{DecodingResult, ImageDecoder};
use image::jpeg::{JPEGDecoder, JPEGEncoder};

use ext2::{Disk, Ext2};

fn main() {
    let ext2 = Ext2::new(File::open("basic.ext2").unwrap()).unwrap();
    let mut f = ext2.open("/sub/michelle.jpg").unwrap();
    let mut im = JPEGDecoder::new(f);
    let mut decoded = im.read_image().unwrap();
    if let DecodingResult::U8(ref mut v) = decoded {
        let pixels: Vec<_> = v.iter_mut().map(|pix| 255 - *pix).collect();
        let mut out = File::create("michelle.jpg").unwrap();
        let mut oim = JPEGEncoder::new(&mut out);
        oim.encode(
            &pixels,
            im.dimensions().unwrap().0,
            im.dimensions().unwrap().1,
            im.colortype().unwrap(),
        ).unwrap();
    } else {
        panic!("Halp.");
    }
}

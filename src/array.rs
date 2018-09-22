//! array.rs: Utilities for copying bytes to fixed-length arrays.

pub(crate) fn array64(input: &[u8]) -> [u8; 64] {
    let mut a = [0; 64];
    copy_array(input, &mut a[..]);
    a
}

pub(crate) fn array16(input: &[u8]) -> [u8; 16] {
    let mut a = [0; 16];
    copy_array(input, &mut a[..]);
    a
}

pub(crate) fn array12(input: &[u8]) -> [u8; 12] {
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

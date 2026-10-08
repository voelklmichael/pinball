//! `CMP1` container: `"CMP1"`, little-endian u32 unpacked size, then an lh5
//! bitstream. Files without the magic are stored uncompressed (the loader at
//! 0x44b524 falls back to reading the raw file).

use crate::lh5;

pub const MAGIC: &[u8; 4] = b"CMP1";

pub fn is_compressed(data: &[u8]) -> bool {
    data.starts_with(MAGIC)
}

/// Return the file contents, decompressing them if they are `CMP1`.
pub fn unpack(data: &[u8]) -> Result<Vec<u8>, lh5::Error> {
    if !is_compressed(data) || data.len() < 8 {
        return Ok(data.to_vec());
    }
    let size = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
    lh5::decompress(&data[8..], size)
}

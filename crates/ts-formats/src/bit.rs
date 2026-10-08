//! `.BIT` images: headerless 24-bit BGR pixels (after `CMP1` unpacking). The
//! dimensions are not stored; they follow from the resolution set the file
//! belongs to, so we infer them from the size.

/// Screen resolutions the game ships art for.
pub const RESOLUTIONS: [(u32, u32); 5] =
    [(640, 480), (800, 600), (1024, 768), (1280, 1024), (1600, 1200)];

pub struct Image {
    pub width: u32,
    pub height: u32,
    /// RGB, 3 bytes per pixel, rows top to bottom.
    pub pixels: Vec<u8>,
}

/// Interpret unpacked `.BIT` data as one of the known screen resolutions.
pub fn from_unpacked(mut pixels: Vec<u8>) -> Option<Image> {
    let (width, height) = RESOLUTIONS
        .into_iter()
        .find(|&(w, h)| (w * h * 3) as usize == pixels.len())?;
    for px in pixels.chunks_exact_mut(3) {
        px.swap(0, 2);
    }
    Some(Image { width, height, pixels })
}

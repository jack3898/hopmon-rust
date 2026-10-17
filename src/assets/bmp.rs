//! Decoder for the uncompressed DIBs stored as `RT_BITMAP` resources.

pub struct Image {
    pub width: u32,
    pub height: u32,
    /// Tightly packed RGBA8, top row first.
    pub rgba: Vec<u8>,
}

impl Image {
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.width + x) * 4) as usize;
        [
            self.rgba[i],
            self.rgba[i + 1],
            self.rgba[i + 2],
            self.rgba[i + 3],
        ]
    }
}

/// Decodes a DIB (BITMAPINFOHEADER followed by palette and pixels).
/// Accepts an optional leading BITMAPFILEHEADER as well.
pub fn decode(mut d: &[u8]) -> Result<Image, String> {
    if d.starts_with(b"BM") && d.len() > 14 {
        d = &d[14..];
    }
    let u32_at = |o: usize| -> u32 { u32::from_le_bytes(d[o..o + 4].try_into().unwrap()) };
    let u16_at = |o: usize| -> u16 { u16::from_le_bytes(d[o..o + 2].try_into().unwrap()) };
    if d.len() < 40 {
        return Err("bitmap header too short".into());
    }
    let hsize = u32_at(0) as usize;
    let width = u32_at(4) as i32;
    let height = u32_at(8) as i32;
    let bpp = u16_at(14);
    let comp = u32_at(16);
    let clr_used = u32_at(32);
    if comp != 0 {
        return Err(format!("compressed bitmaps are unsupported ({comp})"));
    }
    let (w, h) = (width.unsigned_abs(), height.unsigned_abs());
    let top_down = height < 0;
    let ncolors = if bpp <= 8 {
        if clr_used == 0 {
            1usize << bpp
        } else {
            clr_used as usize
        }
    } else {
        0
    };
    let pal_off = hsize;
    let pix_off = pal_off + ncolors * 4;
    let stride = ((w * bpp as u32).div_ceil(32) * 4) as usize;
    if d.len() < pix_off + stride * h as usize {
        return Err("bitmap data truncated".into());
    }
    let pal = |i: usize| -> [u8; 4] {
        let o = pal_off + i.min(ncolors.saturating_sub(1)) * 4;
        [d[o + 2], d[o + 1], d[o], 255]
    };
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for y in 0..h as usize {
        let src_row = if top_down { y } else { h as usize - 1 - y };
        let row = &d[pix_off + src_row * stride..];
        for x in 0..w as usize {
            let c = match bpp {
                1 => pal(((row[x / 8] >> (7 - x % 8)) & 1) as usize),
                4 => pal(((row[x / 2] >> if x % 2 == 0 { 4 } else { 0 }) & 15) as usize),
                8 => pal(row[x] as usize),
                24 => [row[x * 3 + 2], row[x * 3 + 1], row[x * 3], 255],
                32 => [row[x * 4 + 2], row[x * 4 + 1], row[x * 4], 255],
                _ => return Err(format!("unsupported bit depth {bpp}")),
            };
            let o = (y * w as usize + x) * 4;
            rgba[o..o + 4].copy_from_slice(&c);
        }
    }
    Ok(Image {
        width: w,
        height: h,
        rgba,
    })
}

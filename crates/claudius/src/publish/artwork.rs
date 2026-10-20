//! What a cover image is, read from its file: its format, size and colours, whether any of it is see-through, the
//! colour profile it carries, whether it is stored turned, the quality a JPEG was saved at, and whether it holds the
//! detail its size promises.

use std::io::Cursor;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Colour {
    Rgb,
    Grey,
    /// four inks, for print
    Cmyk,
    /// at most 256 colours, from a palette
    Palette,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Artwork {
    /// JPEG or PNG
    pub format: String,
    pub width: u32,
    pub height: u32,
    /// the file's size
    pub bytes: u64,
    pub colour: Colour,
    /// bits per channel
    pub bits: u8,
    /// it has an alpha channel
    pub alpha: bool,
    /// some of it is see-through
    pub transparent: bool,
    /// the name of the colour profile it carries
    pub profile: Option<String>,
    /// it is stored turned or mirrored, with a tag that says how to show it
    pub rotated: bool,
    /// the JPEG quality (1 to 100) its quantisation tables come closest to
    pub quality: Option<u8>,
    /// the side of the smaller square that would hold all its detail, when that is well under its own: an image
    /// enlarged from a smaller one, or blurred
    pub detail: Option<u32>,
}

impl Artwork {
    pub fn is_srgb(&self) -> bool {
        self.profile.as_deref().is_none_or(|p| p.to_ascii_lowercase().contains("srgb"))
    }
}

/// Read a cover image. The error, said of the artwork, tells what is wrong: `is a WebP image`, `cannot be read: ...`.
pub fn inspect(bytes: &[u8]) -> Result<Artwork, String> {
    let mut a = match sniff(bytes) {
        Some("JPEG") => jpeg(bytes)?,
        Some("PNG") => png(bytes)?,
        Some(other) => {
            return Err(format!("is a{} {other} file", if other.starts_with(['A', 'H', 'S']) { "n" } else { "" }));
        }
        None => return Err("is not an image claudius can read".into()),
    };
    a.bytes = bytes.len() as u64;
    let pixels = u64::from(a.width) * u64::from(a.height);
    // CMYK is wrong whatever its pixels are, and a picture this large is wrong in size: neither needs decoding
    if a.colour != Colour::Cmyk && pixels <= MAX_PIXELS {
        let format = if a.format == "PNG" { image::ImageFormat::Png } else { image::ImageFormat::Jpeg };
        let img = image::load_from_memory_with_format(bytes, format).map_err(|e| format!("cannot be read: {e}"))?;
        a.transparent = a.alpha && see_through(&img);
        if pixels <= MAX_DETAIL_PIXELS {
            let luma = img.to_luma32f();
            a.detail = detail(luma.as_raw(), luma.width() as usize, luma.height() as usize);
        }
    }
    Ok(a)
}

/// larger than any store takes (10000 × 10000 is Spotify's largest), so not worth decoding
const MAX_PIXELS: u64 = 100_000_000;
/// beyond 6000 × 6000, the size is the warning: at the 3000 × 3000 stores ask for, the detail is checked
const MAX_DETAIL_PIXELS: u64 = 36_000_000;

fn see_through(img: &image::DynamicImage) -> bool {
    use image::DynamicImage as D;
    match img {
        D::ImageRgba8(b) => b.pixels().any(|p| p[3] < u8::MAX),
        D::ImageLumaA8(b) => b.pixels().any(|p| p[1] < u8::MAX),
        D::ImageRgba16(b) => b.pixels().any(|p| p[3] < u16::MAX),
        D::ImageLumaA16(b) => b.pixels().any(|p| p[1] < u16::MAX),
        other => other.color().has_alpha() && other.to_rgba8().pixels().any(|p| p[3] < u8::MAX),
    }
}

fn sniff(b: &[u8]) -> Option<&'static str> {
    let at = |range: std::ops::Range<usize>| b.get(range).unwrap_or_default();
    Some(match () {
        _ if b.starts_with(&[0xFF, 0xD8, 0xFF]) => "JPEG",
        _ if b.starts_with(b"\x89PNG\r\n\x1a\n") => "PNG",
        _ if b.starts_with(b"GIF8") => "GIF",
        _ if b.starts_with(b"II*\0") || b.starts_with(b"MM\0*") => "TIFF",
        _ if at(0..4) == b"RIFF" && at(8..12) == b"WEBP" => "WebP",
        _ if at(4..8) == b"ftyp" && matches!(at(8..12), b"avif" | b"avis") => "AVIF",
        _ if at(4..8) == b"ftyp" && matches!(at(8..12), b"heic" | b"heix" | b"mif1" | b"msf1" | b"hevc") => "HEIC",
        _ if b.starts_with(b"BM") => "BMP",
        _ if b.starts_with(b"%PDF") => "PDF",
        _ if b.starts_with(b"8BPS") => "Photoshop",
        _ if b.starts_with(b"<svg") || b.starts_with(b"<?xml") => "SVG",
        _ => return None,
    })
}

fn broken(why: impl std::fmt::Display) -> String {
    format!("cannot be read: {why}")
}

fn jpeg(b: &[u8]) -> Result<Artwork, String> {
    let mut frame = None;
    let mut icc: Vec<(u8, &[u8])> = Vec::new();
    let mut rotated = false;
    let mut luma_table = None;
    let mut i = 2;
    // the headers, up to the first scan
    while i + 4 <= b.len() && b[i] == 0xFF {
        let marker = b[i + 1];
        match marker {
            0xFF => {
                i += 1;
                continue;
            }
            0x01 | 0xD0..=0xD8 => {
                i += 2;
                continue;
            }
            0xD9 | 0xDA => break,
            _ => {}
        }
        let len = usize::from(u16::from_be_bytes([b[i + 2], b[i + 3]]));
        let end = i + 2 + len;
        if len < 2 || end > b.len() {
            return Err(broken("a header runs past the end of the file"));
        }
        let s = &b[i + 4..end];
        match marker {
            // start of frame: every kind but the Huffman and arithmetic table markers in that range
            0xC0..=0xCF if !matches!(marker, 0xC4 | 0xC8 | 0xCC) && s.len() >= 6 => {
                let (height, width) = (u16::from_be_bytes([s[1], s[2]]), u16::from_be_bytes([s[3], s[4]]));
                frame = Some((u32::from(width), u32::from(height), s[5], s[0]));
            }
            0xDB => luma_table = luma_table.or_else(|| quant_table(s)),
            0xE1 if s.starts_with(b"Exif\0\0") => rotated = exif_rotated(&s[6..]),
            0xE2 if s.starts_with(b"ICC_PROFILE\0") && s.len() > 14 => icc.push((s[12], &s[14..])),
            _ => {}
        }
        i = end;
    }
    let (width, height, components, bits) = frame.ok_or_else(|| broken("it has no picture in it"))?;
    icc.sort_by_key(|c| c.0);
    let profile = (!icc.is_empty()).then(|| {
        let p: Vec<u8> = icc.iter().flat_map(|c| c.1.iter().copied()).collect();
        icc_name(&p).unwrap_or_else(|| "unnamed".into())
    });
    Ok(Artwork {
        format: "JPEG".into(),
        width,
        height,
        bytes: 0,
        colour: match components {
            1 => Colour::Grey,
            4 => Colour::Cmyk,
            _ => Colour::Rgb,
        },
        bits,
        alpha: false,
        transparent: false,
        profile,
        rotated,
        quality: luma_table.map(|t| quality(&t)),
        detail: None,
    })
}

/// the first quantisation table a DQT segment defines when it is table 0, the luminance one encoders write first
fn quant_table(s: &[u8]) -> Option<Vec<u16>> {
    let (precision, id) = (s.first()? >> 4, s.first()? & 15);
    if id != 0 {
        return None;
    }
    Some(if precision == 0 {
        s.get(1..65)?.iter().map(|&v| u16::from(v)).collect()
    } else {
        s.get(1..129)?.chunks(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect()
    })
}

/// the luminance table of the JPEG standard's example, which libjpeg scales by quality (in any order: the sums are
/// compared)
const STANDARD_LUMA: [u32; 64] = [
    16, 11, 10, 16, 24, 40, 51, 61, 12, 12, 14, 19, 26, 58, 60, 55, 14, 13, 16, 24, 40, 57, 69, 56, 14, 17, 22, 29, 51,
    87, 80, 62, 18, 22, 37, 56, 68, 109, 103, 77, 24, 35, 55, 64, 81, 104, 113, 92, 49, 64, 78, 87, 103, 121, 120, 101,
    72, 92, 95, 98, 112, 100, 103, 99,
];

/// the libjpeg quality whose luminance table comes closest to `table` in total: exact for libjpeg's own files, and
/// about right for other encoders', which scale their tables much the same way
fn quality(table: &[u16]) -> u8 {
    let total: u32 = table.iter().map(|&v| u32::from(v)).sum();
    let scaled = |q: u32| -> u32 {
        let scale = if q < 50 { 5000 / q } else { 200 - 2 * q };
        STANDARD_LUMA.iter().map(|&v| ((v * scale + 50) / 100).clamp(1, 255)).sum()
    };
    (1..=100).rev().min_by_key(|&q| scaled(q).abs_diff(total)).unwrap_or(100) as u8
}

/// does an EXIF block (a TIFF header and its first directory) say to show the image turned or mirrored?
fn exif_rotated(t: &[u8]) -> bool {
    let little = t.starts_with(b"II");
    let u16_at = |o: usize| {
        t.get(o..o + 2)
            .map(|b| if little { u16::from_le_bytes([b[0], b[1]]) } else { u16::from_be_bytes([b[0], b[1]]) })
    };
    let u32_at = |o: usize| {
        t.get(o..o + 4).map(|b| {
            let b = [b[0], b[1], b[2], b[3]];
            if little { u32::from_le_bytes(b) } else { u32::from_be_bytes(b) }
        })
    };
    let Some(ifd) = u32_at(4).map(|o| o as usize) else { return false };
    let entries = u16_at(ifd).unwrap_or(0);
    (0..usize::from(entries)).any(|k| {
        let e = ifd + 2 + k * 12;
        u16_at(e) == Some(0x0112) && u16_at(e + 8).is_some_and(|o| (2..=8).contains(&o))
    })
}

/// the description an ICC profile gives itself ("sRGB IEC61966-2.1", "Display P3")
fn icc_name(p: &[u8]) -> Option<String> {
    let be32 = |b: &[u8], o: usize| b.get(o..o + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize);
    let tags = be32(p, 128)?;
    let entry = (0..tags.min(256)).map(|k| 132 + k * 12).find(|&e| p.get(e..e + 4) == Some(b"desc"))?;
    let (offset, len) = (be32(p, entry + 4)?, be32(p, entry + 8)?);
    let tag = p.get(offset..offset.checked_add(len)?)?;
    let name = match tag.get(0..4)? {
        b"desc" => {
            let n = be32(tag, 8)?;
            String::from_utf8_lossy(tag.get(12..12 + n)?).into_owned()
        }
        b"mluc" => {
            let (n, at) = (be32(tag, 20)?, be32(tag, 24)?);
            let units: Vec<u16> =
                tag.get(at..at.checked_add(n)?)?.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
            String::from_utf16_lossy(&units)
        }
        _ => return None,
    };
    let name = name.trim_matches(|c: char| c == '\0' || c.is_whitespace()).to_string();
    (!name.is_empty()).then_some(name)
}

fn png(b: &[u8]) -> Result<Artwork, String> {
    use png::ColorType as C;
    let reader = png::Decoder::new(Cursor::new(b)).read_info().map_err(broken)?;
    let info = reader.info();
    let exif = info.exif_metadata.as_deref();
    Ok(Artwork {
        format: "PNG".into(),
        width: info.width,
        height: info.height,
        bytes: 0,
        colour: match info.color_type {
            C::Grayscale | C::GrayscaleAlpha => Colour::Grey,
            C::Indexed => Colour::Palette,
            C::Rgb | C::Rgba => Colour::Rgb,
        },
        bits: info.bit_depth as u8,
        alpha: matches!(info.color_type, C::GrayscaleAlpha | C::Rgba) || info.trns.is_some(),
        transparent: false,
        profile: info.icc_profile.as_deref().map(|p| icc_name(p).unwrap_or_else(|| "unnamed".into())),
        rotated: exif.is_some_and(|e| exif_rotated(e.strip_prefix(b"Exif\0\0").unwrap_or(e))),
        quality: None,
        detail: None,
    })
}

/// How much of the picture's detail its own size holds, from the width of its strong edges. In a sharp picture, a
/// photo or type, the sharpest of them cross in one to three pixels; enlarged k times, none is narrower than about
/// k pixels, and blurred, none is narrow at all. Measured on song covers enlarged from a half, a third and a quarter
/// of their size, which this finds from a third on; JPEG noise only makes edges look narrower. Returns the side of
/// the square the detail would fit in, when the picture looks enlarged 2.5 times or more.
fn detail(luma: &[f32], w: usize, h: usize) -> Option<u32> {
    let mut widths = Vec::new();
    for row in luma.chunks_exact(w).step_by(2) {
        edge_widths(row, &mut widths);
    }
    let mut column = vec![0.0; h];
    for x in (0..w).step_by(2) {
        for (c, row) in column.iter_mut().zip(luma.chunks_exact(w)) {
            *c = row[x];
        }
        edge_widths(&column, &mut widths);
    }
    // too few strong edges to tell soft from smooth: gradients, mist
    if widths.len() < 500 {
        return None;
    }
    widths.sort_unstable();
    let at = |share: f64| f64::from(widths[((widths.len() - 1) as f64 * share) as usize]);
    let enlarged = (at(0.05) / 2.0).max(at(0.5) / 3.0);
    (enlarged >= 2.5).then(|| ((w.min(h) as f64 / enlarged / 100.0).round() * 100.0).max(100.0) as u32)
}

/// The widths of the strong edges along a line of pixels: each run over which brightness climbs (or falls) all the
/// way, from its steepest step out, that changes it by at least 0.15 in all.
fn edge_widths(line: &[f32], out: &mut Vec<u8>) {
    let n = line.len();
    let step = |i: usize| line[i + 1] - line[i];
    let mut x = 1;
    while x + 2 < n {
        let g = step(x);
        if g.abs() < 0.01 || g.abs() < step(x - 1).abs() || g.abs() <= step(x + 1).abs() {
            x += 1;
            continue;
        }
        let (mut l, mut r) = (x, x);
        while l > 0 && step(l - 1) * g.signum() > 0.001 {
            l -= 1;
        }
        while r + 2 < n && step(r + 1) * g.signum() > 0.001 {
            r += 1;
        }
        if (line[r + 1] - line[l]).abs() >= 0.15 {
            out.push((r - l + 1).min(64) as u8);
        }
        x = r + 1;
    }
}

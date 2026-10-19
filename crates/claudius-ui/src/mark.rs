//! Claudius' mark, a C cut out of colourful ripples, drawn into a texture at the size it's shown. The numbers are the
//! ones `scripts/icons.py` draws the app's icons from, on its 1024 grid; change them in both.

use eframe::egui::emath::GuiRounding as _;
use eframe::egui::{Color32, ColorImage, Id, Rect, TextureHandle, TextureOptions, Ui, pos2, vec2};

/// the C: centre, outer and inner radius, half the opening on the right in degrees
const C: (f32, f32, f32, f32, f32) = (524.0, 512.0, 322.0, 162.0, 44.0);
/// the ripples: centre, the first radius, the step between rings, the gap between them
const RIPPLES: (f32, f32, f32, f32, f32) = (150.0, 900.0, 57.0, 48.0, 9.0);
const BANDS: f32 = 17.0;
/// along the ripples, from the far side of the C (yellow) to the near one
const SPECTRUM: [(f32, [f32; 3]); 6] = [
    (0.0, [255.0, 214.0, 64.0]),
    (0.2, [255.0, 146.0, 56.0]),
    (0.4, [255.0, 78.0, 128.0]),
    (0.6, [176.0, 76.0, 240.0]),
    (0.8, [82.0, 110.0, 255.0]),
    (1.0, [40.0, 210.0, 230.0]),
];
/// samples a pixel, across and down
const SAMPLES: usize = 4;

/// the C's box on the grid
fn bounds() -> Rect {
    let (cx, cy, outer, _, mouth) = C;
    Rect::from_min_size(pos2(cx - outer, cy - outer), vec2(outer + outer * mouth.to_radians().cos(), 2.0 * outer))
}

/// the mark `height` points high, at its own width
pub fn size(height: f32) -> eframe::egui::Vec2 {
    let b = bounds();
    vec2(height * b.width() / b.height(), height)
}

/// paints the mark into `rect`, which [`size`] gives the shape of
pub fn paint(ui: &Ui, rect: Rect) {
    let ctx = ui.ctx();
    let rect = rect.round_to_pixels(ctx.pixels_per_point());
    let height = (rect.height() * ctx.pixels_per_point()).round().max(1.0) as usize;
    let id = Id::unique(("claudius_mark", height));
    let texture = ctx.data_mut(|d| d.get_temp::<TextureHandle>(id)).unwrap_or_else(|| {
        let texture = ctx.load_texture("claudius_mark", image(height), TextureOptions::LINEAR);
        ctx.data_mut(|d| d.insert_temp(id, texture.clone()));
        texture
    });
    let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    ui.painter().image(texture.id(), rect, uv, Color32::WHITE);
}

fn colour(t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    for pair in SPECTRUM.windows(2) {
        let ((t0, c0), (t1, c1)) = (pair[0], pair[1]);
        if t <= t1 {
            let f = (t - t0) / (t1 - t0);
            return [c0[0] + (c1[0] - c0[0]) * f, c0[1] + (c1[1] - c0[1]) * f, c0[2] + (c1[2] - c0[2]) * f];
        }
    }
    SPECTRUM[SPECTRUM.len() - 1].1
}

/// the colour at a point of the grid, or `None` outside the C and in the gaps between the rings
fn sample(x: f32, y: f32, gap: f32) -> Option<[f32; 3]> {
    let (cx, cy, outer, inner, mouth) = C;
    let (dx, dy) = (x - cx, y - cy);
    let d = dx.hypot(dy);
    if d > outer || d < inner || dy.atan2(dx).abs() < mouth.to_radians() {
        return None;
    }
    let (rx, ry, first, step, _) = RIPPLES;
    let rings = ((x - rx).hypot(y - ry) - first) / step;
    let band = rings.ceil();
    if (rings - rings.round()).abs() * step < gap / 2.0 || !(1.0..=BANDS).contains(&band) {
        return None;
    }
    Some(colour(1.0 - band / BANDS))
}

/// the mark `height` pixels high, its colour averaged over [`SAMPLES`]² points a pixel
fn image(height: usize) -> ColorImage {
    let b = bounds();
    let width = (b.width() / b.height() * height as f32).round().max(1.0) as usize;
    let unit = b.height() / height as f32;
    // the gaps stay visible down to the smallest sizes
    let gap = RIPPLES.4.max(0.75 * unit);
    let n = (SAMPLES * SAMPLES) as f32;
    let mut pixels = Vec::with_capacity(width * height);
    for py in 0..height {
        for px in 0..width {
            let mut sum = [0.0f32; 4];
            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    let x = b.left() + (px as f32 + (sx as f32 + 0.5) / SAMPLES as f32) * unit;
                    let y = b.top() + (py as f32 + (sy as f32 + 0.5) / SAMPLES as f32) * unit;
                    if let Some([r, g, bl]) = sample(x, y, gap) {
                        sum = [sum[0] + r, sum[1] + g, sum[2] + bl, sum[3] + 255.0];
                    }
                }
            }
            let [r, g, bl, a] = sum.map(|v| (v / n).round() as u8);
            pixels.push(Color32::from_rgba_premultiplied(r, g, bl, a));
        }
    }
    ColorImage::new([width, height], pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_c_is_open_on_the_right_and_coloured_along_the_ripples() {
        let (cx, cy, outer, inner, _) = C;
        let mid = (outer + inner) / 2.0;
        assert!(sample(cx + mid, cy, 9.0).is_none(), "the opening");
        assert!(sample(cx, cy, 9.0).is_none(), "the counter");
        let near = sample(cx - mid * 0.7, cy + mid * 0.7, 0.0).unwrap();
        let far = sample(cx + mid * 0.6, cy - mid * 0.8, 0.0).unwrap();
        assert!(near[2] > far[2], "blue-violet near the ripples' centre, yellow far from it: {near:?} {far:?}");
    }

    #[test]
    fn the_image_is_the_marks_shape_and_mostly_clear_round_it() {
        let image = image(64);
        assert_eq!(image.size, [55, 64]);
        let clear = image.pixels.iter().filter(|p| p.a() == 0).count();
        assert!(clear > image.pixels.len() / 4 && clear < image.pixels.len() * 3 / 4, "{clear}");
    }
}

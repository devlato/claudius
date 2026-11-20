//! `cargo xtask icons`: claudius's logo, a C cut out of colourful ripples, and every file that shows it. It was
//! scripts/icons.py, which needed librsvg and fontTools; here the SVGs are written as text and the PNG files drawn from
//! the same shapes with tiny-skia, the wordmark outlined from Ubuntu Light (the studio's font, as egui carries it) with
//! skrifa, and the .ico and .icns packed from the PNG files. It writes:
//!
//! - packaging/icons: claudius.svg (Linux), claudius-512.png (Linux, the AppImage), claudius.ico (Windows) and
//!   claudius.icns (macOS, on Apple's icon grid: a smaller body with a shadow)
//! - crates/claudius-ui/assets: the studio window's icon, icon.png and icon-macos.png
//! - packaging/macos: dmg-background.png and @2x, the disk image's window (dmg-settings.py lays it out)
//! - site and docs/theme: the favicons and the touch icon; site/assets: mark-ripples.svg, and logo-ripples.svg with the
//!   name outlined, so the SVG needs no font
//!
//! crates/claudius-ui/src/mark.rs draws the same mark in the studio, from the same numbers.

use std::path::Path;

use anyhow::{Context, Result, anyhow};
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{FontRef, MetadataProvider};
use tiny_skia::{
    Color, FillRule, GradientStop, LinearGradient, Mask, Paint, Path as SkPath, PathBuilder, Pixmap, PixmapPaint,
    Point, SpreadMode, Stroke, Transform,
};

/// Apple's icon grid: a 1024 canvas, an 824 body (x, y, size, corner radius). The other platforms get the body alone.
const BODY: (f64, f64, f64, f64) = (100.0, 100.0, 824.0, 186.0);
/// top, bottom
const BACKGROUND: ([u8; 3], [u8; 3]) = ([0x1d, 0x1f, 0x2b], [0x0b, 0x0c, 0x12]);
/// the C: centre, radii, and half the opening on the right in degrees
const C: (f64, f64, f64, f64, f64) = (524.0, 512.0, 322.0, 162.0, 44.0);
/// the ripples: centre (the body's bottom left), the first radius, the step between rings, the dark gap between them.
/// The first radius and the step keep the rings from shaving thin slivers off the C's corners and edges.
const RIPPLES: (f64, f64, f64, f64, f64) = (150.0, 900.0, 57.0, 48.0, 9.0);
const BANDS: usize = 17;
/// along the ripples, from the far side of the C (yellow) to the near one
const SPECTRUM: [(f64, [u8; 3]); 6] = [
    (0.0, [255, 214, 64]),
    (0.2, [255, 146, 56]),
    (0.4, [255, 78, 128]),
    (0.6, [176, 76, 240]),
    (0.8, [82, 110, 255]),
    (1.0, [40, 210, 230]),
];
const WORDMARK: (&str, &str) = ("Claudius", "#dde1e8");

/// the spectrum at `t`, rounded as Python rounds (half to even), so the colours are the ones the logo always had
fn colour(t: f64) -> [u8; 3] {
    for w in SPECTRUM.windows(2) {
        let ((t0, c0), (t1, c1)) = (w[0], w[1]);
        if t <= t1 {
            let f = (t - t0) / (t1 - t0);
            return [0, 1, 2]
                .map(|i| (f64::from(c0[i]) + (f64::from(c1[i]) - f64::from(c0[i])) * f).round_ties_even() as u8);
        }
    }
    SPECTRUM[SPECTRUM.len() - 1].1
}

fn hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

/// a number as the SVGs write it: two places at most, no trailing zeros
fn num(v: f64) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// the C's outline points: (outer x, outer y, inner x, inner y) of the mouth's edge
fn mouth() -> (f64, f64, f64, f64) {
    let (_, _, outer, inner, m) = C;
    let m = m.to_radians();
    (outer * m.cos(), outer * m.sin(), inner * m.cos(), inner * m.sin())
}

fn c_path() -> String {
    let (cx, cy, outer, inner, _) = C;
    let (outer_x, outer_y, inner_x, inner_y) = mouth();
    format!(
        "M{} {}A{outer} {outer} 0 1 0 {} {}L{} {}A{inner} {inner} 0 1 1 {} {}Z",
        num(cx + outer_x),
        num(cy - outer_y),
        num(cx + outer_x),
        num(cy + outer_y),
        num(cx + inner_x),
        num(cy + inner_y),
        num(cx + inner_x),
        num(cy - inner_y)
    )
}

fn c_box() -> (f64, f64, f64, f64) {
    let (cx, cy, outer, _, m) = C;
    (cx - outer, cy - outer, outer + outer * m.to_radians().cos(), 2.0 * outer)
}

/// the ripples, each band a circle stroked between the gaps, seen through the C
fn mark(clip_id: &str) -> String {
    let (x, y, first, step, gap) = RIPPLES;
    let rings: String = (1..=BANDS)
        .map(|k| {
            let r = first + step * (k as f64 - 0.5);
            format!(
                r#"<circle cx="{x}" cy="{y}" r="{}" stroke="{}"/>"#,
                num(r),
                hex(colour(1.0 - k as f64 / BANDS as f64))
            )
        })
        .collect();
    format!(
        r#"<clipPath id="{clip_id}"><path d="{}"/></clipPath><g clip-path="url(#{clip_id})" fill="none" stroke-width="{}">{rings}</g>"#,
        c_path(),
        step - gap
    )
}

/// the three kinds of icon: `Macos` on Apple's grid, `Full` the rounded body alone, `Square` the body without its
/// corners (iOS rounds them)
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Macos,
    Full,
    Square,
}

/// the icon's view box (x, y, width, height) and its corner radius
fn view(kind: Kind) -> ((f64, f64, f64, f64), f64) {
    let (x, y, size, radius) = BODY;
    match kind {
        Kind::Macos => ((0.0, 0.0, 1024.0, 1024.0), radius),
        Kind::Full => ((x - 16.0, y - 16.0, size + 32.0, size + 32.0), radius),
        Kind::Square => ((x, y, size, size), 0.0),
    }
}

fn icon_svg(kind: Kind) -> String {
    let (x, y, size, _) = BODY;
    let ((vx, vy, vw, vh), rx) = view(kind);
    let shadow = if kind == Kind::Macos {
        r##"<filter id="shadow" x="-20%" y="-20%" width="140%" height="140%"><feDropShadow dx="0" dy="14" stdDeviation="18" flood-color="#000" flood-opacity="0.35"/></filter>"##
    } else {
        ""
    };
    let mut body = format!(r#"x="{x}" y="{y}" width="{size}" height="{size}" rx="{rx}" fill="url(#bg)""#);
    if kind == Kind::Macos {
        body.push_str(r#" filter="url(#shadow)""#);
    }
    let edge = if rx > 0.0 {
        format!(
            r##"<rect x="{}" y="{}" width="{}" height="{}" rx="{}" fill="none" stroke="#fff" stroke-opacity="0.1" stroke-width="3"/>"##,
            x + 1.5,
            y + 1.5,
            size - 3.0,
            size - 3.0,
            rx - 1.5
        )
    } else {
        String::new()
    };
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{vx} {vy} {vw} {vh}" width="1024" height="1024"><defs>{shadow}<linearGradient id="bg" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{}"/><stop offset="1" stop-color="{}"/></linearGradient></defs><rect {body}/>{}{edge}</svg>
"#,
        hex(BACKGROUND.0),
        hex(BACKGROUND.1),
        mark("c")
    )
}

fn mark_svg() -> String {
    let (bx, by, bw, bh) = c_box();
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{} {} {} {}" width="{}" height="52">{}</svg>
"#,
        num(bx),
        num(by),
        num(bw),
        num(bh),
        num(bw / bh * 52.0),
        mark("c")
    )
}

/// what a glyph's outline draws into: an SVG path's commands, under a transform, and the box of its points
struct SvgPen<'a> {
    d: &'a mut String,
    box_: &'a mut [f64; 4],
    /// x' = sx * x + tx, y' = sy * y + ty
    sx: f64,
    sy: f64,
    tx: f64,
    ty: f64,
}

impl SvgPen<'_> {
    fn pt(&mut self, x: f32, y: f32) -> (String, String) {
        let (x, y) = (self.sx * f64::from(x) + self.tx, self.sy * f64::from(y) + self.ty);
        self.box_[0] = self.box_[0].min(x);
        self.box_[1] = self.box_[1].min(y);
        self.box_[2] = self.box_[2].max(x);
        self.box_[3] = self.box_[3].max(y);
        (num(x), num(y))
    }
}

impl OutlinePen for SvgPen<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.pt(x, y);
        self.d.push_str(&format!("M{x} {y}"));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.pt(x, y);
        self.d.push_str(&format!("L{x} {y}"));
    }
    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        let (a, b) = self.pt(cx0, cy0);
        let (x, y) = self.pt(x, y);
        self.d.push_str(&format!("Q{a} {b} {x} {y}"));
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        let (a, b) = self.pt(cx0, cy0);
        let (c, e) = self.pt(cx1, cy1);
        let (x, y) = self.pt(x, y);
        self.d.push_str(&format!("C{a} {b} {c} {e} {x} {y}"));
    }
    fn close(&mut self) {
        self.d.push('Z');
    }
}

/// `text` in Ubuntu Light, `size` high, its baseline at (x, baseline), as one SVG path, and its ink's box
fn text_path(text: &str, size: f64, x: f64, baseline: f64) -> Result<(String, [f64; 4])> {
    let font = FontRef::new(epaint_default_fonts::UBUNTU_LIGHT).map_err(|e| anyhow!("Ubuntu Light: {e}"))?;
    let at = LocationRef::default();
    let scale = size / f64::from(font.metrics(Size::unscaled(), at).units_per_em);
    let (charmap, glyphs, metrics) = (font.charmap(), font.outline_glyphs(), font.glyph_metrics(Size::unscaled(), at));
    let (mut d, mut box_) = (String::new(), [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY]);
    let mut cursor = 0.0;
    for ch in text.chars() {
        let gid = charmap.map(ch).with_context(|| format!("no {ch} in Ubuntu Light"))?;
        let glyph = glyphs.get(gid).with_context(|| format!("no outline for {ch}"))?;
        let mut pen =
            SvgPen { d: &mut d, box_: &mut box_, sx: scale, sy: -scale, tx: x + cursor * scale, ty: baseline };
        glyph.draw(DrawSettings::unhinted(Size::unscaled(), at), &mut pen).map_err(|e| anyhow!("{ch}: {e:?}"))?;
        cursor += f64::from(metrics.advance_width(gid).unwrap_or_default());
    }
    Ok((d, box_))
}

/// the mark 44 high, then the name, its ink centred on the mark, 52 high in all
fn logo_svg() -> Result<String> {
    let (bx, by, bw, bh) = c_box();
    let s = 44.0 / bh;
    let (text, fill) = WORDMARK;
    let (_, box_) = text_path(text, 38.0, 0.0, 0.0)?;
    let (d, box_) = text_path(text, 38.0, 4.0 + bw * s + 14.0, 26.0 - (box_[1] + box_[3]) / 2.0)?;
    let width = (box_[2] + 4.0).ceil();
    Ok(format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} 52" width="{width}" height="52" role="img" aria-label="{text}"><g transform="translate({} {}) scale({s:.5})">{}</g><path fill="{fill}" d="{d}"/></svg>
"#,
        num(4.0 - bx * s),
        num(4.0 - by * s),
        mark("c")
    ))
}

fn color(c: [u8; 3], alpha: f32) -> Color {
    Color::from_rgba8(c[0], c[1], c[2], (alpha * 255.0).round() as u8)
}

fn solid(c: Color) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(c);
    p.anti_alias = true;
    p
}

/// an arc of the circle at (cx, cy) of radius r from angle a0 to a1 (radians, y down), as cubics of a quarter turn
/// at most
fn arc(pb: &mut PathBuilder, (cx, cy, r): (f64, f64, f64), a0: f64, a1: f64) {
    let n = ((a1 - a0).abs() / std::f64::consts::FRAC_PI_2).ceil().max(1.0) as usize;
    let da = (a1 - a0) / n as f64;
    let k = 4.0 / 3.0 * (da / 4.0).tan();
    for i in 0..n {
        let (s, e) = (a0 + da * i as f64, a0 + da * (i + 1) as f64);
        let (p0, p3) = ((cx + r * s.cos(), cy + r * s.sin()), (cx + r * e.cos(), cy + r * e.sin()));
        let p1 = (p0.0 - k * r * s.sin(), p0.1 + k * r * s.cos());
        let p2 = (p3.0 + k * r * e.sin(), p3.1 - k * r * e.cos());
        pb.cubic_to(p1.0 as f32, p1.1 as f32, p2.0 as f32, p2.1 as f32, p3.0 as f32, p3.1 as f32);
    }
}

/// the C: the outer arc the long way round from the top of its mouth to the bottom, the inner one back
fn c_shape() -> Option<SkPath> {
    let (cx, cy, outer, inner, m) = C;
    let m = m.to_radians();
    let tau = std::f64::consts::TAU;
    let mut pb = PathBuilder::new();
    pb.move_to((cx + outer * m.cos()) as f32, (cy - outer * m.sin()) as f32);
    arc(&mut pb, (cx, cy, outer), -m, m - tau);
    pb.line_to((cx + inner * m.cos()) as f32, (cy + inner * m.sin()) as f32);
    arc(&mut pb, (cx, cy, inner), m, tau - m);
    pb.close();
    pb.finish()
}

/// a rectangle with corners rounded by `r`
fn rounded(x: f64, y: f64, w: f64, h: f64, r: f64) -> Option<SkPath> {
    if r <= 0.0 {
        return tiny_skia::Rect::from_xywh(x as f32, y as f32, w as f32, h as f32).map(PathBuilder::from_rect);
    }
    let mut pb = PathBuilder::new();
    let half = std::f64::consts::FRAC_PI_2;
    pb.move_to((x + r) as f32, y as f32);
    pb.line_to((x + w - r) as f32, y as f32);
    arc(&mut pb, (x + w - r, y + r, r), -half, 0.0);
    pb.line_to((x + w) as f32, (y + h - r) as f32);
    arc(&mut pb, (x + w - r, y + h - r, r), 0.0, half);
    pb.line_to((x + r) as f32, (y + h) as f32);
    arc(&mut pb, (x + r, y + h - r, r), half, 2.0 * half);
    pb.line_to(x as f32, (y + r) as f32);
    arc(&mut pb, (x + r, y + r, r), 2.0 * half, 3.0 * half);
    pb.close();
    pb.finish()
}

/// a box blur of the alpha of `p`, three times over (a Gaussian of `sigma` pixels, as SVG blurs one)
fn blur(p: &mut Pixmap, sigma: f64) {
    let d = (sigma * 3.0 * (2.0 * std::f64::consts::PI).sqrt() / 4.0 + 0.5).floor() as usize;
    if d == 0 {
        return;
    }
    let (w, h) = (p.width() as usize, p.height() as usize);
    let mut a: Vec<f64> = p.pixels().iter().map(|c| f64::from(c.alpha())).collect();
    let pass = |a: &mut Vec<f64>, horizontal: bool| {
        let (len, lines) = if horizontal { (w, h) } else { (h, w) };
        let at = |line: usize, i: usize| if horizontal { line * w + i } else { i * w + line };
        let mut out = vec![0.0; a.len()];
        for line in 0..lines {
            let mut sum = 0.0;
            let lo = d / 2;
            for i in 0..len + lo {
                if i < len {
                    sum += a[at(line, i)];
                }
                if i >= d {
                    sum -= a[at(line, i - d)];
                }
                if i >= lo && i - lo < len {
                    out[at(line, i - lo)] = sum / d as f64;
                }
            }
        }
        *a = out;
    };
    for _ in 0..3 {
        pass(&mut a, true);
        pass(&mut a, false);
    }
    for (px, v) in p.pixels_mut().iter_mut().zip(a) {
        let v = v.round().clamp(0.0, 255.0) as u8;
        *px = tiny_skia::PremultipliedColorU8::from_rgba(0, 0, 0, v).unwrap_or(*px);
    }
}

/// the icon of `kind`, `size` pixels square
fn icon_png(kind: Kind, size: u32) -> Result<Vec<u8>> {
    let ((vx, vy, vw, _), rx) = view(kind);
    let s = f64::from(size) / vw;
    let t = Transform::from_row(s as f32, 0.0, 0.0, s as f32, (-vx * s) as f32, (-vy * s) as f32);
    let mut p = Pixmap::new(size, size).context("no pixmap")?;
    let (x, y, body, _) = BODY;
    let shape = rounded(x, y, body, body, rx).context("no body")?;
    if kind == Kind::Macos {
        // a drop shadow 14 down, blurred 18, black at 35%, under the body
        let mut shadow = Pixmap::new(size, size).context("no pixmap")?;
        shadow.fill_path(
            &shape,
            &solid(Color::BLACK),
            FillRule::Winding,
            t.post_translate(0.0, (14.0 * s) as f32),
            None,
        );
        blur(&mut shadow, 18.0 * s);
        let paint = PixmapPaint { opacity: 0.35, ..PixmapPaint::default() };
        p.draw_pixmap(0, 0, shadow.as_ref(), &paint, Transform::identity(), None);
    }
    let gradient = LinearGradient::new(
        Point::from_xy(0.0, y as f32),
        Point::from_xy(0.0, (y + body) as f32),
        vec![GradientStop::new(0.0, color(BACKGROUND.0, 1.0)), GradientStop::new(1.0, color(BACKGROUND.1, 1.0))],
        SpreadMode::Pad,
        Transform::identity(),
    )
    .context("no gradient")?;
    let paint = Paint { shader: gradient, anti_alias: true, ..Paint::default() };
    p.fill_path(&shape, &paint, FillRule::Winding, t, None);
    // the ripples, seen through the C
    let mut clip = Mask::new(size, size).context("no mask")?;
    clip.fill_path(&c_shape().context("no C")?, FillRule::Winding, true, t);
    let (rx0, ry0, first, step, gap) = RIPPLES;
    let stroke = Stroke { width: (step - gap) as f32, ..Stroke::default() };
    for k in 1..=BANDS {
        let r = first + step * (k as f64 - 0.5);
        let ring = PathBuilder::from_circle(rx0 as f32, ry0 as f32, r as f32).context("no ring")?;
        let c = colour(1.0 - k as f64 / BANDS as f64);
        p.stroke_path(&ring, &solid(color(c, 1.0)), &stroke, t, Some(&clip));
    }
    if rx > 0.0 {
        let edge = rounded(x + 1.5, y + 1.5, body - 3.0, body - 3.0, rx - 1.5).context("no edge")?;
        let stroke = Stroke { width: 3.0, ..Stroke::default() };
        p.stroke_path(&edge, &solid(Color::from_rgba8(255, 255, 255, 26)), &stroke, t, None);
    }
    encode(&p)
}

/// the font the disk image's words are in: Helvetica Neue where the system has it (a Mac, which builds disk images),
/// else Ubuntu Light
fn system_font() -> Vec<u8> {
    ["/System/Library/Fonts/HelveticaNeue.ttc", "/System/Library/Fonts/Helvetica.ttc"]
        .iter()
        .find_map(|p| std::fs::read(p).ok())
        .unwrap_or_else(|| epaint_default_fonts::UBUNTU_LIGHT.to_vec())
}

/// what a glyph's outline draws into: a tiny-skia path under a transform
struct SkPen<'a> {
    pb: &'a mut PathBuilder,
    sx: f32,
    tx: f32,
    ty: f32,
}

impl OutlinePen for SkPen<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.pb.move_to(self.sx * x + self.tx, -self.sx * y + self.ty);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.pb.line_to(self.sx * x + self.tx, -self.sx * y + self.ty);
    }
    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        let (s, tx, ty) = (self.sx, self.tx, self.ty);
        self.pb.quad_to(s * cx0 + tx, -s * cy0 + ty, s * x + tx, -s * y + ty);
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        let (s, tx, ty) = (self.sx, self.tx, self.ty);
        self.pb.cubic_to(s * cx0 + tx, -s * cy0 + ty, s * cx1 + tx, -s * cy1 + ty, s * x + tx, -s * y + ty);
    }
    fn close(&mut self) {
        self.pb.close();
    }
}

/// `text` in the face `index` of `font`, `size` high, centred on `x` with its baseline at `y`, as a path
fn words(font: &[u8], index: u32, text: &str, size: f32, x: f32, y: f32) -> Option<SkPath> {
    let f = FontRef::from_index(font, index).ok()?;
    let at = LocationRef::default();
    let scale = size / f32::from(f.metrics(Size::unscaled(), at).units_per_em);
    let (charmap, glyphs, metrics) = (f.charmap(), f.outline_glyphs(), f.glyph_metrics(Size::unscaled(), at));
    let ids: Vec<_> = text.chars().filter_map(|c| charmap.map(c)).collect();
    let width: f32 = ids.iter().map(|g| metrics.advance_width(*g).unwrap_or_default()).sum::<f32>() * scale;
    let mut pb = PathBuilder::new();
    let mut cursor = x - width / 2.0;
    for g in ids {
        if let Some(glyph) = glyphs.get(g) {
            let mut pen = SkPen { pb: &mut pb, sx: scale, tx: cursor, ty: y };
            let _ = glyph.draw(DrawSettings::unhinted(Size::unscaled(), at), &mut pen);
        }
        cursor += metrics.advance_width(g).unwrap_or_default() * scale;
    }
    pb.finish()
}

/// the face of a collection with a name that holds `want` (Medium, Regular ...), else its first
fn face(font: &[u8], want: &str) -> u32 {
    let count = skrifa::raw::FileRef::new(font).map_or(1, |f| f.fonts().count() as u32);
    (0..count)
        .find(|&i| {
            FontRef::from_index(font, i).is_ok_and(|f| {
                f.localized_strings(skrifa::string::StringId::SUBFAMILY_NAME)
                    .any(|s| s.chars().collect::<String>() == want)
            })
        })
        .unwrap_or(0)
}

/// The disk image's window, 640 by 400 (at `scale`): Claudius.app at (170, 190) and Applications at (470, 190), as
/// packaging/macos/dmg-settings.py places them, with an arrow from one to the other. Light, since Finder draws the
/// icons' names in black over any background picture, whatever the system's appearance.
fn dmg_background(scale: u32) -> Result<Vec<u8>> {
    let s = scale as f32;
    let t = Transform::from_scale(s, s);
    let mut p = Pixmap::new(640 * scale, 400 * scale).context("no pixmap")?;
    let paper = LinearGradient::new(
        Point::from_xy(0.0, 0.0),
        Point::from_xy(0.0, 400.0),
        vec![
            GradientStop::new(0.0, Color::from_rgba8(0xfa, 0xf8, 0xf4, 255)),
            GradientStop::new(1.0, Color::from_rgba8(0xec, 0xe8, 0xe0, 255)),
        ],
        SpreadMode::Pad,
        Transform::identity(),
    )
    .context("no gradient")?;
    let all = tiny_skia::Rect::from_xywh(0.0, 0.0, 640.0, 400.0).map(PathBuilder::from_rect).context("no rect")?;
    p.fill_path(&all, &Paint { shader: paper, anti_alias: true, ..Paint::default() }, FillRule::Winding, t, None);
    // five of the ripples from the bottom left, faint (the group at 12%)
    let mut rings = Pixmap::new(640 * scale, 400 * scale).context("no pixmap")?;
    let (_, _, first, step, gap) = RIPPLES;
    let stroke = Stroke { width: (step - gap) as f32, ..Stroke::default() };
    for k in 1..=5 {
        let r = first + step * (k as f64 - 0.5);
        let ring = PathBuilder::from_circle(-20.0, 430.0, r as f32).context("no ring")?;
        rings.stroke_path(&ring, &solid(color(colour(1.0 - k as f64 / BANDS as f64), 1.0)), &stroke, t, None);
    }
    p.draw_pixmap(
        0,
        0,
        rings.as_ref(),
        &PixmapPaint { opacity: 0.12, ..PixmapPaint::default() },
        Transform::identity(),
        None,
    );
    let font = system_font();
    let (bold, plain) = (face(&font, "Medium"), face(&font, "Regular"));
    if let Some(w) = words(&font, bold, "Drag Claudius into Applications", 21.0, 320.0, 66.0) {
        p.fill_path(&w, &solid(Color::from_rgba8(0x1d, 0x1f, 0x2b, 255)), FillRule::Winding, t, None);
    }
    if let Some(w) = words(&font, plain, "Then open Claudius from your Applications folder.", 13.0, 320.0, 352.0) {
        p.fill_path(&w, &solid(Color::from_rgba8(0x6b, 0x6f, 0x7a, 255)), FillRule::Winding, t, None);
    }
    // the arrow, through the spectrum from the app to Applications
    let mut pb = PathBuilder::new();
    pb.move_to(262.0, 190.0);
    pb.line_to(372.0, 190.0);
    pb.move_to(352.0, 172.0);
    pb.line_to(374.0, 190.0);
    pb.line_to(352.0, 208.0);
    let arrow = pb.finish().context("no arrow")?;
    let along = LinearGradient::new(
        Point::from_xy(262.0, 0.0),
        Point::from_xy(378.0, 0.0),
        vec![
            GradientStop::new(0.0, color(colour(0.6), 1.0)),
            GradientStop::new(0.5, color(colour(0.4), 1.0)),
            GradientStop::new(1.0, color(colour(0.2), 1.0)),
        ],
        SpreadMode::Pad,
        Transform::identity(),
    )
    .context("no gradient")?;
    let stroke = Stroke {
        width: 9.0,
        line_cap: tiny_skia::LineCap::Round,
        line_join: tiny_skia::LineJoin::Round,
        ..Stroke::default()
    };
    p.stroke_path(&arrow, &Paint { shader: along, anti_alias: true, ..Paint::default() }, &stroke, t, None);
    encode(&p)
}

/// a pixmap as a PNG, its colours no longer premultiplied by their alpha; one opaque all over without an alpha, as the
/// Python script wrote the touch icons and the disk image's window
fn encode(p: &Pixmap) -> Result<Vec<u8>> {
    let opaque = p.pixels().iter().all(|c| c.alpha() == u8::MAX);
    let data: Vec<u8> = p
        .pixels()
        .iter()
        .flat_map(|c| {
            let c = c.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect::<Vec<u8>>()
        .chunks(4)
        .flat_map(|c| if opaque { &c[..3] } else { c })
        .copied()
        .collect();
    let mut out = Vec::new();
    let mut e = png::Encoder::new(&mut out, p.width(), p.height());
    e.set_color(if opaque { png::ColorType::Rgb } else { png::ColorType::Rgba });
    e.set_depth(png::BitDepth::Eight);
    e.write_header()?.write_image_data(&data)?;
    Ok(out)
}

/// an .ico of PNG images, which Windows reads at every size
fn ico(pngs: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend(0u16.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend((pngs.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * pngs.len() as u32;
    for (size, data) in pngs {
        out.extend([(*size % 256) as u8, (*size % 256) as u8, 0, 0]);
        out.extend(1u16.to_le_bytes());
        out.extend(32u16.to_le_bytes());
        out.extend((data.len() as u32).to_le_bytes());
        out.extend(offset.to_le_bytes());
        offset += data.len() as u32;
    }
    for (_, data) in pngs {
        out.extend(data);
    }
    out
}

/// an .icns of PNG images, under the types iconutil writes for each size
fn icns(pngs: &[(&[u8; 4], Vec<u8>)]) -> Vec<u8> {
    let mut chunks = Vec::new();
    for (kind, data) in pngs {
        chunks.extend(*kind);
        chunks.extend((8 + data.len() as u32).to_be_bytes());
        chunks.extend(data);
    }
    let mut out = b"icns".to_vec();
    out.extend((8 + chunks.len() as u32).to_be_bytes());
    out.extend(chunks);
    out
}

const ICNS: [(&[u8; 4], u32); 10] = [
    (b"icp4", 16),
    (b"ic11", 32),
    (b"icp5", 32),
    (b"ic12", 64),
    (b"ic07", 128),
    (b"ic13", 256),
    (b"ic08", 256),
    (b"ic14", 512),
    (b"ic09", 512),
    (b"ic10", 1024),
];

fn write(root: &Path, file: &str, bytes: impl AsRef<[u8]>) -> Result<()> {
    let path = root.join(file);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&path, bytes).with_context(|| format!("cannot write {}", path.display()))
}

/// Every file that shows the logo, written into `root` (the repository's, unless given another to compare).
pub fn icons(root: &Path) -> Result<()> {
    write(root, "packaging/icons/claudius.svg", icon_svg(Kind::Full))?;
    write(root, "packaging/icons/claudius-512.png", icon_png(Kind::Full, 512)?)?;
    let ico_pngs: Vec<(u32, Vec<u8>)> =
        [16, 24, 32, 48, 64, 128, 256].into_iter().map(|s| Ok((s, icon_png(Kind::Full, s)?))).collect::<Result<_>>()?;
    write(root, "packaging/icons/claudius.ico", ico(&ico_pngs))?;
    let icns_pngs: Vec<(&[u8; 4], Vec<u8>)> =
        ICNS.iter().map(|&(k, s)| Ok((k, icon_png(Kind::Macos, s)?))).collect::<Result<_>>()?;
    write(root, "packaging/icons/claudius.icns", icns(&icns_pngs))?;
    write(root, "crates/claudius-ui/assets/icon.png", icon_png(Kind::Full, 256)?)?;
    write(root, "crates/claudius-ui/assets/icon-macos.png", icon_png(Kind::Macos, 512)?)?;
    write(root, "packaging/macos/dmg-background.png", dmg_background(1)?)?;
    write(root, "packaging/macos/dmg-background@2x.png", dmg_background(2)?)?;
    for folder in ["site", "docs/theme"] {
        write(root, &format!("{folder}/favicon.svg"), icon_svg(Kind::Full))?;
    }
    write(root, "site/favicon-32.png", icon_png(Kind::Full, 32)?)?;
    write(root, "site/icon-512.png", icon_png(Kind::Full, 512)?)?;
    write(root, "site/apple-touch-icon.png", icon_png(Kind::Square, 180)?)?;
    write(root, "docs/theme/favicon.png", icon_png(Kind::Square, 180)?)?;
    // the site caches these for a week: a new design is a new name
    write(root, "site/assets/mark-ripples.svg", mark_svg())?;
    write(root, "site/assets/logo-ripples.svg", logo_svg()?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_numbers_and_colours_are_written_as_the_logo_always_had_them() {
        assert_eq!(
            (num(162.0), num(2.5), num(1.005), num(-0.0)),
            ("162".into(), "2.5".into(), "1".into(), "-0".into())
        );
        // the colours and the C as scripts/icons.py wrote them
        assert_eq!((hex(colour(0.0)), hex(colour(1.0))), ("#ffd640".into(), "#28d2e6".into()));
        assert_eq!((hex(colour(0.1)), hex(colour(1.0 - 3.0 / 17.0))), ("#ffb43c".into(), "#4d7afc".into()));
        let c = "M755.63 288.32A322 322 0 1 0 755.63 735.68L640.53 624.53A162 162 0 1 1 640.53 399.47Z";
        assert_eq!(c_path(), c);
        assert!(mark_svg().starts_with(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="202 190 553.63 644" width="44.7" height="52">"#
        ));
        let full = icon_svg(Kind::Full);
        assert!(full.starts_with(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="84 84 856 856" width="1024" height="1024">"#
        ));
        assert!(
            full.contains(r#"<rect x="100" y="100" width="824" height="824" rx="186" fill="url(#bg)"/>"#),
            "{full}"
        );
    }

    #[test]
    fn every_kind_of_file_comes_out_whole() {
        let png = icon_png(Kind::Full, 64).unwrap();
        assert_eq!(&png[1..4], b"PNG");
        // its colour type (IHDR's tenth byte, after the signature and the chunk's length and name): the icon with its
        // corners clear has an alpha, and the touch icon, opaque all over, has none, as the Python script wrote them
        assert_eq!(png[25], 6, "RGBA");
        assert_eq!(icon_png(Kind::Square, 180).unwrap()[25], 2, "RGB");
        let ico = ico(&[(16, vec![1, 2, 3]), (32, vec![4])]);
        assert_eq!((&ico[0..6], ico.len()), (&[0, 0, 1, 0, 2, 0][..], 6 + 32 + 4));
        assert_eq!(u32::from_le_bytes(ico[18..22].try_into().unwrap()), 38, "the first image after the directory");
        let icns = icns(&[(b"ic07", vec![9; 10])]);
        assert_eq!((&icns[0..4], u32::from_be_bytes(icns[4..8].try_into().unwrap())), (&b"icns"[..], 26));
        let svg = logo_svg().unwrap();
        assert!(svg.contains(r#"aria-label="Claudius""#) && svg.contains("<path fill=\"#dde1e8\" d=\"M"), "{svg}");
    }
}

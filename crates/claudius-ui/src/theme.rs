//! The studio look: a dark, slightly blue-grey palette, an amber accent, and one colour per track role and section
//! kind so the same instrument reads the same everywhere (arrangement, sequencer, mixer, parts).

use eframe::egui::{self, Color32, CornerRadius, FontFamily, FontId, Margin, Shadow, Stroke, TextStyle, Theme, Vec2};

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

pub const BG: Color32 = rgb(16, 18, 23);
pub const PANEL: Color32 = rgb(23, 26, 32);
pub const RAISED: Color32 = rgb(32, 36, 44);
pub const RAISED_HI: Color32 = rgb(42, 47, 57);
pub const WELL: Color32 = rgb(11, 13, 17);
pub const LINE: Color32 = rgb(46, 51, 62);
pub const LINE_SOFT: Color32 = rgb(31, 35, 43);
pub const TEXT: Color32 = rgb(221, 225, 232);
pub const TEXT_DIM: Color32 = rgb(143, 151, 164);
pub const TEXT_FAINT: Color32 = rgb(94, 101, 114);
pub const ACCENT: Color32 = rgb(255, 163, 72);
pub const GOOD: Color32 = rgb(84, 212, 134);
pub const WARN: Color32 = rgb(246, 192, 74);
pub const BAD: Color32 = rgb(240, 94, 86);
pub const LCD: Color32 = rgb(9, 15, 17);
pub const LCD_TEXT: Color32 = rgb(132, 232, 255);
pub const PLAYHEAD: Color32 = rgb(255, 236, 214);

/// `a` moved towards `b` by `t` (0..1)
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}

/// the colour with an alpha of `a` (0..1)
pub fn alpha(c: Color32, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (a.clamp(0.0, 1.0) * 255.0) as u8)
}

/// a colour for a track role or a layer / lane / track name
pub fn role_color(name: &str) -> Color32 {
    let n = name.to_ascii_lowercase();
    let has = |k: &str| n.contains(k);
    if has("kick") || n == "bd" {
        rgb(255, 94, 94)
    } else if has("snare") || has("clap") || has("snap") || has("rim") || n == "sd" || n == "cp" {
        rgb(255, 146, 66)
    } else if has("ohat") || has("open") || n == "oh" {
        rgb(255, 216, 110)
    } else if has("hat") || has("hh") || has("shaker") || has("ride") || has("crash") || has("cym") {
        rgb(236, 196, 92)
    } else if has("perc") || has("top") || has("tom") || has("cowbell") || has("conga") || has("bongo") || has("drum") {
        rgb(212, 164, 104)
    } else if has("808") || has("sub") || has("bass") || has("acid") || has("reese") {
        rgb(164, 116, 255)
    } else if has("chord") || has("keys") || has("piano") || has("organ") || has("stab") || has("epiano") {
        rgb(64, 212, 198)
    } else if has("pad") || has("string") || has("atmo") || has("drone") || has("choir") {
        rgb(88, 158, 255)
    } else if has("lead") || has("melod") || has("bell") || has("pluck") || has("vocal") || has("hook") {
        rgb(255, 108, 196)
    } else if has("arp") || has("seq") {
        rgb(112, 224, 124)
    } else if has("fx") || has("riser") || has("impact") || has("sweep") || has("transition") || has("noise") {
        rgb(152, 162, 180)
    } else if has("loop") || has("chop") || has("sample") {
        rgb(196, 206, 104)
    } else {
        const PALETTE: [Color32; 6] = [
            rgb(120, 200, 255),
            rgb(255, 140, 120),
            rgb(180, 220, 120),
            rgb(230, 150, 255),
            rgb(255, 200, 140),
            rgb(140, 230, 200),
        ];
        let h = n.bytes().fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32));
        PALETTE[(h % PALETTE.len() as u32) as usize]
    }
}

/// a colour for a section kind (intro, verse, build, drop, break, hook, groove, outro)
pub fn section_color(kind: &str) -> Color32 {
    match kind {
        "intro" => rgb(74, 122, 186),
        "verse" => rgb(84, 162, 112),
        "groove" => rgb(46, 166, 160),
        "build" => rgb(222, 166, 62),
        "drop" => rgb(232, 82, 78),
        "hook" => rgb(238, 120, 58),
        "break" => rgb(138, 104, 204),
        "outro" => rgb(98, 110, 136),
        _ => rgb(122, 132, 152),
    }
}

/// a colour per progression name (A, B, C ...)
pub fn prog_color(name: &str) -> Color32 {
    const PALETTE: [Color32; 6] = [
        rgb(52, 196, 186),
        rgb(150, 116, 226),
        rgb(236, 170, 70),
        rgb(240, 110, 170),
        rgb(110, 170, 255),
        rgb(150, 210, 100),
    ];
    let i = name.bytes().next().map_or(0, |b| b.to_ascii_uppercase().wrapping_sub(b'A') as usize);
    PALETTE[i % PALETTE.len()]
}

/// the solo colour
pub const SOLO: Color32 = rgb(255, 212, 64);

/// red .. amber .. green for a 0..1 quality
pub fn score_color(q: f64) -> Color32 {
    if q >= 0.8 {
        GOOD
    } else if q >= 0.6 {
        mix(WARN, GOOD, ((q - 0.6) / 0.2) as f32)
    } else if q >= 0.4 {
        mix(BAD, WARN, ((q - 0.4) / 0.2) as f32)
    } else {
        BAD
    }
}

pub fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

pub fn prop(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

pub fn apply(ctx: &egui::Context) {
    ctx.set_theme(Theme::Dark);
    ctx.style_mut_of(Theme::Dark, |style| {
        style.text_styles = [
            (TextStyle::Heading, prop(16.0)),
            (TextStyle::Body, prop(13.0)),
            (TextStyle::Button, prop(13.0)),
            (TextStyle::Small, prop(11.0)),
            (TextStyle::Monospace, mono(12.5)),
        ]
        .into();
        let s = &mut style.spacing;
        s.item_spacing = Vec2::new(6.0, 5.0);
        s.button_padding = Vec2::new(8.0, 3.0);
        s.interact_size = Vec2::new(36.0, 22.0);
        s.slider_width = 120.0;
        s.combo_width = 120.0;
        s.window_margin = Margin::same(12);
        s.menu_margin = Margin::same(6);
        s.scroll.bar_width = 8.0;
        s.scroll.floating = true;
        style.interaction.tooltip_delay = 0.35;

        let v = &mut style.visuals;
        v.dark_mode = true;
        v.override_text_color = None;
        v.panel_fill = PANEL;
        v.window_fill = RAISED;
        v.window_stroke = Stroke::new(1.0, LINE);
        v.window_corner_radius = CornerRadius::same(10);
        v.menu_corner_radius = CornerRadius::same(7);
        v.window_shadow = Shadow { offset: [0, 8], blur: 24, spread: 0, color: Color32::from_black_alpha(140) };
        v.popup_shadow = Shadow { offset: [0, 4], blur: 14, spread: 0, color: Color32::from_black_alpha(120) };
        v.extreme_bg_color = WELL;
        v.text_edit_bg_color = Some(WELL);
        v.faint_bg_color = rgb(21, 24, 30);
        v.code_bg_color = WELL;
        v.hyperlink_color = ACCENT;
        v.warn_fg_color = WARN;
        v.error_fg_color = BAD;
        v.selection.bg_fill = alpha(ACCENT, 0.32);
        v.selection.stroke = Stroke::new(1.0, ACCENT);
        v.slider_trailing_fill = true;
        v.striped = false;
        v.indent_has_left_vline = false;

        let radius = CornerRadius::same(4);
        let w = &mut v.widgets;
        w.noninteractive.bg_fill = PANEL;
        w.noninteractive.weak_bg_fill = PANEL;
        w.noninteractive.bg_stroke = Stroke::new(1.0, LINE_SOFT);
        w.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_DIM);
        w.noninteractive.corner_radius = radius;
        w.inactive.bg_fill = RAISED;
        w.inactive.weak_bg_fill = RAISED;
        w.inactive.bg_stroke = Stroke::new(1.0, LINE);
        w.inactive.fg_stroke = Stroke::new(1.0, TEXT);
        w.inactive.corner_radius = radius;
        w.hovered.bg_fill = RAISED_HI;
        w.hovered.weak_bg_fill = RAISED_HI;
        w.hovered.bg_stroke = Stroke::new(1.0, rgb(78, 86, 102));
        w.hovered.fg_stroke = Stroke::new(1.5, Color32::WHITE);
        w.hovered.corner_radius = radius;
        w.hovered.expansion = 0.0;
        w.active.bg_fill = rgb(56, 62, 75);
        w.active.weak_bg_fill = rgb(56, 62, 75);
        w.active.bg_stroke = Stroke::new(1.0, ACCENT);
        w.active.fg_stroke = Stroke::new(1.5, Color32::WHITE);
        w.active.corner_radius = radius;
        w.active.expansion = 0.0;
        w.open.bg_fill = rgb(48, 54, 66);
        w.open.weak_bg_fill = rgb(48, 54, 66);
        w.open.bg_stroke = Stroke::new(1.0, LINE);
        w.open.fg_stroke = Stroke::new(1.0, TEXT);
        w.open.corner_radius = radius;
    });
}

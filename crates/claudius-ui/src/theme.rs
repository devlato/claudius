//! The studio's looks: a dark one, slightly blue-grey, and a light one, both with an amber accent, and one colour per
//! track role and section kind so the same instrument reads the same everywhere (arrangement, sequencer, mixer, parts).
//! The person picks dark, light or as the system is (`Look`), which egui follows, in a browser too. Each colour the
//! studio draws with is a `Swatch`, named for what it is for, which turns into the look's own wherever it is drawn.

use std::sync::atomic::{AtomicBool, Ordering};

use eframe::egui::{
    self, Color32, CornerRadius, FontFamily, FontId, Margin, Shadow, Stroke, TextStyle, Theme, ThemePreference, Vec2,
};
use serde::{Deserialize, Serialize};

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// The colours of a look, one for each thing drawn: what the swatches turn into.
pub struct Palette {
    pub bg: Color32,
    pub panel: Color32,
    pub raised: Color32,
    pub raised_hi: Color32,
    pub well: Color32,
    pub control: Color32,
    pub control_hi: Color32,
    pub line: Color32,
    pub line_soft: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub text_faint: Color32,
    pub accent: Color32,
    pub good: Color32,
    pub warn: Color32,
    pub bad: Color32,
    pub lcd: Color32,
    pub lcd_text: Color32,
    /// the LCD's cyan off the LCD, as the song's genre and the verdict's notes are drawn
    pub info: Color32,
    pub playhead: Color32,
    pub solo: Color32,
    /// the strongest ink, which the text and icons of a control under the pointer are drawn in: white on the dark
    /// look, black on the light one
    pub strong: Color32,
    /// what egui draws with itself: the faintest fill, the outline under the pointer, a pressed control's fill, and
    /// how dark the shadows are
    faint_bg: Color32,
    hover_line: Color32,
    pressed: Color32,
    shadow: u8,
}

/// the dark look
pub const DARK: Palette = Palette {
    bg: rgb(16, 18, 23),
    panel: rgb(23, 26, 32),
    raised: rgb(32, 36, 44),
    raised_hi: rgb(42, 47, 57),
    well: rgb(11, 13, 17),
    control: rgb(39, 44, 54),
    control_hi: rgb(50, 56, 68),
    line: rgb(46, 51, 62),
    line_soft: rgb(31, 35, 43),
    text: rgb(221, 225, 232),
    text_dim: rgb(143, 151, 164),
    text_faint: rgb(134, 141, 155),
    accent: rgb(255, 163, 72),
    good: rgb(84, 212, 134),
    warn: rgb(246, 192, 74),
    bad: rgb(240, 94, 86),
    lcd: rgb(9, 15, 17),
    lcd_text: rgb(132, 232, 255),
    info: rgb(132, 232, 255),
    playhead: rgb(255, 236, 214),
    solo: rgb(255, 212, 64),
    faint_bg: rgb(21, 24, 30),
    hover_line: rgb(66, 73, 88),
    pressed: rgb(60, 67, 81),
    strong: Color32::WHITE,
    shadow: 150,
};

/// The light look: the same parts, light, with text dark enough to read on every panel (the faintest at 4.5:1, as
/// small text needs), and the accent and the verdict's colours darkened to read on white. The LCD stays a display.
pub const LIGHT: Palette = Palette {
    bg: rgb(229, 232, 238),
    panel: rgb(241, 243, 246),
    raised: rgb(252, 252, 253),
    raised_hi: rgb(234, 237, 242),
    well: rgb(255, 255, 255),
    control: rgb(226, 230, 236),
    control_hi: rgb(214, 219, 227),
    line: rgb(196, 202, 212),
    line_soft: rgb(218, 223, 230),
    text: rgb(24, 28, 36),
    text_dim: rgb(76, 84, 98),
    text_faint: rgb(92, 100, 114),
    accent: rgb(180, 86, 0),
    good: rgb(16, 120, 58),
    warn: rgb(140, 92, 0),
    bad: rgb(196, 40, 34),
    lcd: rgb(9, 15, 17),
    lcd_text: rgb(132, 232, 255),
    info: rgb(0, 112, 140),
    playhead: rgb(196, 80, 0),
    solo: rgb(176, 128, 0),
    faint_bg: rgb(235, 238, 242),
    hover_line: rgb(170, 178, 190),
    pressed: rgb(200, 206, 216),
    strong: Color32::BLACK,
    shadow: 40,
};

/// A colour of the looks, by what it is for: the look's own wherever it is drawn (`Color32::from`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Swatch {
    Bg,
    Panel,
    Raised,
    RaisedHi,
    Well,
    ControlHi,
    Line,
    LineSoft,
    Text,
    TextDim,
    TextFaint,
    Accent,
    Good,
    Warn,
    Bad,
    Lcd,
    LcdText,
    Info,
    Playhead,
    Solo,
    Strong,
}

impl Swatch {
    /// its colour in `p`
    pub fn of(self, p: &Palette) -> Color32 {
        match self {
            Swatch::Bg => p.bg,
            Swatch::Panel => p.panel,
            Swatch::Raised => p.raised,
            Swatch::RaisedHi => p.raised_hi,
            Swatch::Well => p.well,
            Swatch::ControlHi => p.control_hi,
            Swatch::Line => p.line,
            Swatch::LineSoft => p.line_soft,
            Swatch::Text => p.text,
            Swatch::TextDim => p.text_dim,
            Swatch::TextFaint => p.text_faint,
            Swatch::Accent => p.accent,
            Swatch::Good => p.good,
            Swatch::Warn => p.warn,
            Swatch::Bad => p.bad,
            Swatch::Lcd => p.lcd,
            Swatch::LcdText => p.lcd_text,
            Swatch::Info => p.info,
            Swatch::Playhead => p.playhead,
            Swatch::Solo => p.solo,
            Swatch::Strong => p.strong,
        }
    }
}

impl From<Swatch> for Color32 {
    fn from(s: Swatch) -> Color32 {
        s.of(palette())
    }
}

pub const BG: Swatch = Swatch::Bg;
pub const PANEL: Swatch = Swatch::Panel;
pub const RAISED: Swatch = Swatch::Raised;
pub const RAISED_HI: Swatch = Swatch::RaisedHi;
pub const WELL: Swatch = Swatch::Well;
/// a control's fill under the pointer, a step above every panel's (a raised one's too)
pub const CONTROL_HI: Swatch = Swatch::ControlHi;
pub const LINE: Swatch = Swatch::Line;
pub const LINE_SOFT: Swatch = Swatch::LineSoft;
pub const TEXT: Swatch = Swatch::Text;
pub const TEXT_DIM: Swatch = Swatch::TextDim;
/// the quietest text: still 4.5:1 against every panel, a raised one's too, as small text needs to be read
pub const TEXT_FAINT: Swatch = Swatch::TextFaint;
pub const ACCENT: Swatch = Swatch::Accent;
pub const GOOD: Swatch = Swatch::Good;
pub const WARN: Swatch = Swatch::Warn;
pub const BAD: Swatch = Swatch::Bad;
pub const LCD: Swatch = Swatch::Lcd;
pub const LCD_TEXT: Swatch = Swatch::LcdText;
/// the LCD's cyan off the LCD: dark teal on the light look
pub const INFO: Swatch = Swatch::Info;
pub const PLAYHEAD: Swatch = Swatch::Playhead;
/// the solo colour
pub const SOLO: Swatch = Swatch::Solo;
/// the strongest ink: white on the dark look, black on the light one
pub const STRONG: Swatch = Swatch::Strong;

/// which look the studio wears: dark (its own, until the person chooses another), light, or as the system is
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Look {
    #[default]
    Dark,
    Light,
    System,
}

impl Look {
    pub const ALL: [Look; 3] = [Look::Dark, Look::Light, Look::System];

    /// what egui follows
    pub fn preference(self) -> ThemePreference {
        match self {
            Look::Dark => ThemePreference::Dark,
            Look::Light => ThemePreference::Light,
            Look::System => ThemePreference::System,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Look::Dark => "Dark",
            Look::Light => "Light",
            Look::System => "As the system is",
        }
    }

    /// its name in a word, where there is room for no more
    pub fn word(self) -> &'static str {
        match self {
            Look::System => "System",
            look => look.name(),
        }
    }
}

/// the light look is the one drawn with (`follow`)
static LIGHT_IN_USE: AtomicBool = AtomicBool::new(false);

/// the look drawn with
pub fn palette() -> &'static Palette {
    if light() { &LIGHT } else { &DARK }
}

/// whether the light look is the one drawn with
pub fn light() -> bool {
    LIGHT_IN_USE.load(Ordering::Relaxed)
}

/// Takes the look egui draws with this frame, which follows the person's (and the system's), for the swatches to turn
/// into: at the start of every frame.
pub fn follow(ctx: &egui::Context) {
    LIGHT_IN_USE.store(ctx.theme() == Theme::Light, Ordering::Relaxed);
}

/// `a` moved towards `b` by `t` (0..1)
pub fn mix(a: impl Into<Color32>, b: impl Into<Color32>, t: f32) -> Color32 {
    let (a, b): (Color32, Color32) = (a.into(), b.into());
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}

/// `c` moved by `t` (0..1) towards the look's strongest ink, as a control under the pointer or pressed is: lighter on
/// the dark look, darker on the light one
pub fn lit(c: impl Into<Color32>, t: f32) -> Color32 {
    mix(c, STRONG, t)
}

/// how bright a colour is to the eye, as WCAG measures it: 0 for black, 1 for white
pub fn luminance(c: Color32) -> f32 {
    let ch = |x: u8| {
        let x = f32::from(x) / 255.0;
        if x <= 0.039_28 { x / 12.92 } else { ((x + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * ch(c.r()) + 0.7152 * ch(c.g()) + 0.0722 * ch(c.b())
}

/// The ink that reads on a fill of `c`, as a filled button's label or a lit cell's is: black on a bright fill, white on
/// a dark one. Either reads at 4.5:1 or more on any fill, as they read alike at the brightness where one gives way to
/// the other.
pub fn ink_on(c: impl Into<Color32>) -> Color32 {
    if luminance(c.into()) > 0.179 { Color32::BLACK } else { Color32::WHITE }
}

/// the colour with an alpha of `a` (0..1)
pub fn alpha(c: impl Into<Color32>, a: f32) -> Color32 {
    let c: Color32 = c.into();
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (a.clamp(0.0, 1.0) * 255.0) as u8)
}

/// a role's, a section's or a progression's colour as the look draws it: a shade darker on the light one, where the
/// colours stand on light panels
fn ink(c: Color32) -> Color32 {
    if light() { mix(c, Color32::BLACK, 0.22) } else { c }
}

/// a colour for a track role or a layer / lane / track name
pub fn role_color(name: &str) -> Color32 {
    ink(role_hue(name))
}

fn role_hue(name: &str) -> Color32 {
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
    ink(match kind {
        "intro" => rgb(74, 122, 186),
        "verse" => rgb(84, 162, 112),
        "groove" => rgb(46, 166, 160),
        "build" => rgb(222, 166, 62),
        "drop" => rgb(232, 82, 78),
        "hook" => rgb(238, 120, 58),
        "break" => rgb(138, 104, 204),
        "outro" => rgb(98, 110, 136),
        _ => rgb(122, 132, 152),
    })
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
    ink(PALETTE[i % PALETTE.len()])
}

/// red .. amber .. green for a 0..1 quality
pub fn score_color(q: f64) -> Color32 {
    if q >= 0.8 {
        GOOD.into()
    } else if q >= 0.6 {
        mix(WARN, GOOD, ((q - 0.6) / 0.2) as f32)
    } else if q >= 0.4 {
        mix(BAD, WARN, ((q - 0.4) / 0.2) as f32)
    } else {
        BAD.into()
    }
}

pub fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

pub fn prop(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

/// the font family of headings and buttons: Inter's medium weight
pub const MEDIUM: &str = "medium";

/// the font family of the icons (`widgets::Icon`): Lucide's, a glyph each
pub const ICONS: &str = "icons";

pub fn medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(MEDIUM.into()))
}

/// The studio's fonts: Inter for its text, in two weights, with egui's own behind it for what Inter does not draw
/// (emoji, the arrows and shapes some labels use), and Lucide's icons. Both are cut down to what the studio shows
/// (assets/fonts, with their licences).
pub fn fonts() -> egui::FontDefinitions {
    let mut f = egui::FontDefinitions::default();
    let data = |bytes: &'static [u8]| std::sync::Arc::new(egui::FontData::from_static(bytes));
    f.font_data.insert("inter".into(), data(include_bytes!("../assets/fonts/Inter-Regular.ttf")));
    f.font_data.insert("inter-medium".into(), data(include_bytes!("../assets/fonts/Inter-Medium.ttf")));
    f.font_data.insert("lucide".into(), data(include_bytes!("../assets/fonts/lucide.ttf")));
    let behind = f.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let first = |name: &str| std::iter::once(name.to_string()).chain(behind.iter().cloned()).collect::<Vec<_>>();
    f.families.insert(FontFamily::Proportional, first("inter"));
    f.families.insert(FontFamily::Name(MEDIUM.into()), first("inter-medium"));
    f.families.insert(FontFamily::Name(ICONS.into()), vec!["lucide".into()]);
    f
}

/// The studio's fonts and both looks' styles, and the look the person picked (`set_look` changes it after).
pub fn apply(ctx: &egui::Context, look: Look) {
    ctx.set_fonts(fonts());
    ctx.style_mut_of(Theme::Dark, |style| styled(style, &DARK, true));
    ctx.style_mut_of(Theme::Light, |style| styled(style, &LIGHT, false));
    set_look(ctx, look);
}

/// the look the studio wears from now on
pub fn set_look(ctx: &egui::Context, look: Look) {
    ctx.set_theme(look.preference());
    follow(ctx);
}

/// a look's style: the studio's sizes and spacing, and its colours
fn styled(style: &mut egui::Style, p: &Palette, dark: bool) {
    style.text_styles = [
        (TextStyle::Heading, medium(16.0)),
        (TextStyle::Body, prop(13.5)),
        (TextStyle::Button, medium(13.0)),
        (TextStyle::Small, prop(11.5)),
        (TextStyle::Monospace, mono(12.5)),
    ]
    .into();
    let s = &mut style.spacing;
    s.item_spacing = Vec2::new(6.0, 6.0);
    s.button_padding = Vec2::new(10.0, 4.0);
    s.interact_size = Vec2::new(36.0, 24.0);
    s.slider_width = 120.0;
    s.slider_rail_height = 4.0;
    s.combo_width = 120.0;
    s.icon_width = 15.0;
    s.icon_width_inner = 9.0;
    s.window_margin = Margin::same(14);
    s.menu_margin = Margin::same(6);
    s.scroll.bar_width = 8.0;
    s.scroll.floating = true;
    style.interaction.tooltip_delay = 0.35;

    let v = &mut style.visuals;
    v.dark_mode = dark;
    v.override_text_color = None;
    v.panel_fill = p.panel;
    v.window_fill = p.raised;
    v.window_stroke = Stroke::new(1.0, p.line_soft);
    v.window_corner_radius = CornerRadius::same(12);
    v.menu_corner_radius = CornerRadius::same(8);
    v.window_shadow = Shadow { offset: [0, 12], blur: 32, spread: 0, color: Color32::from_black_alpha(p.shadow) };
    let popup = Color32::from_black_alpha(p.shadow.saturating_sub(20));
    v.popup_shadow = Shadow { offset: [0, 6], blur: 18, spread: 0, color: popup };
    // a slim slider handle, as a fader's cap, rather than a ball
    v.handle_shape = egui::style::HandleShape::Rect { aspect_ratio: 0.55 };
    v.extreme_bg_color = p.well;
    v.text_edit_bg_color = Some(p.well);
    v.faint_bg_color = p.faint_bg;
    v.code_bg_color = p.well;
    v.hyperlink_color = p.accent;
    v.warn_fg_color = p.warn;
    v.error_fg_color = p.bad;
    v.selection.bg_fill = alpha(p.accent, 0.32);
    v.selection.stroke = Stroke::new(1.0, p.accent);
    v.slider_trailing_fill = true;
    v.striped = false;
    v.indent_has_left_vline = false;

    // quiet controls: a fill a step above the panel and no outline until the pointer is on them
    let radius = CornerRadius::same(6);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.panel;
    w.noninteractive.weak_bg_fill = p.panel;
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.line_soft);
    w.noninteractive.fg_stroke = Stroke::new(1.0, p.text_dim);
    w.noninteractive.corner_radius = radius;
    w.inactive.bg_fill = p.control;
    w.inactive.weak_bg_fill = p.control;
    w.inactive.bg_stroke = Stroke::new(1.0, Color32::TRANSPARENT);
    w.inactive.fg_stroke = Stroke::new(1.0, p.text);
    w.inactive.corner_radius = radius;
    w.hovered.bg_fill = p.control_hi;
    w.hovered.weak_bg_fill = p.control_hi;
    w.hovered.bg_stroke = Stroke::new(1.0, p.hover_line);
    w.hovered.fg_stroke = Stroke::new(1.5, p.strong);
    w.hovered.corner_radius = radius;
    w.hovered.expansion = 0.0;
    w.active.bg_fill = p.pressed;
    w.active.weak_bg_fill = p.pressed;
    w.active.bg_stroke = Stroke::new(1.0, alpha(p.accent, 0.8));
    w.active.fg_stroke = Stroke::new(1.5, p.strong);
    w.active.corner_radius = radius;
    w.active.expansion = 0.0;
    w.open.bg_fill = p.control_hi;
    w.open.weak_bg_fill = p.control_hi;
    w.open.bg_stroke = Stroke::new(1.0, p.line);
    w.open.fg_stroke = Stroke::new(1.0, p.text);
    w.open.corner_radius = radius;
}

#[cfg(test)]
mod tests {
    use super::{DARK, LIGHT, Palette, ink_on, luminance};
    use eframe::egui::Color32;

    /// WCAG's contrast of two colours, 1 to 21
    fn contrast(a: Color32, b: Color32) -> f32 {
        let (x, y) = (luminance(a), luminance(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    #[test]
    fn text_reads_on_every_panel_of_both_looks() {
        for (name, p) in [("dark", &DARK), ("light", &LIGHT)] {
            let p: &Palette = p;
            for (under, bg) in [("bg", p.bg), ("panel", p.panel), ("raised", p.raised), ("well", p.well)] {
                for (what, fg, least) in
                    [("text", p.text, 7.0), ("text_dim", p.text_dim, 4.5), ("faint", p.text_faint, 4.5)]
                {
                    let c = contrast(fg, bg);
                    assert!(c >= least, "{name}: {what} on {under} is {c:.1}:1, below {least}:1");
                }
            }
            let marks = [("accent", p.accent), ("good", p.good), ("warn", p.warn), ("bad", p.bad), ("info", p.info)];
            for (what, fg) in marks {
                let c = contrast(fg, p.panel);
                assert!(c >= 3.0, "{name}: {what} on a panel is {c:.1}:1, below the 3:1 a control needs");
            }
            // a filled button's label, a solo and a lit cell, written on their fills
            for (what, fill) in marks.into_iter().chain([("solo", p.solo), ("playhead", p.playhead)]) {
                let c = contrast(ink_on(fill), fill);
                assert!(c >= 4.5, "{name}: the ink on {what} is {c:.1}:1, below 4.5:1");
            }
        }
    }

    #[test]
    fn the_ink_on_any_fill_reads() {
        let steps = || (0..=255u8).step_by(15);
        let worst = steps()
            .flat_map(|r| steps().flat_map(move |g| steps().map(move |b| Color32::from_rgb(r, g, b))))
            .map(|fill| contrast(ink_on(fill), fill))
            .fold(f32::MAX, f32::min);
        assert!(worst >= 4.5, "the ink reads at {worst:.2}:1 on some fill");
    }
}

//! What claudius hears in a recording, shown while it plays: its tempo, key and feel; its form, with the chord of each
//! bar under it and the playhead going through them; its beat as a step grid, and its bass. The recording plays, or a
//! part claudius took it apart into, alone, to hear what it heard. And what can be made of it: a song like it, or its
//! tempo, key and chords for the song that is open.

use std::sync::Arc;
use std::time::Duration;

use claudius_api::{Command, Listened, SectionHeard};
use eframe::egui::{self, Align2, Color32, FontId, Rect, RichText, Sense, Stroke, Ui, pos2, vec2};

use super::{Cx, small};
use crate::audio::{Player, Source};
use crate::pick::Picked;
use crate::theme::{self, ACCENT, TEXT, TEXT_DIM, TEXT_FAINT, WELL};
use crate::widgets::{self, Icon};

/// the tracks of a recording heard, by the names claudius keeps them under, and what the window calls each
const TRACKS: [(&str, &str); 5] =
    [("recording", "All of it"), ("vocals", "Voice"), ("drums", "Drums"), ("bass", "Bass"), ("other", "The rest")];

/// the window, while it shows what was heard, and its player
#[derive(Default)]
pub struct Hears {
    heard: Option<Box<Listened>>,
    /// what was heard, in the words the answer said it in, to copy
    said: String,
    /// the file it came in, which plays where claudius keeps nothing of it (the demo's page)
    file: Option<Arc<Picked>>,
    /// plays the recording or a part of it, made when it first plays
    player: Option<Player>,
    /// the track the player has, and the track to play
    loaded: Option<String>,
    track: String,
    /// the genre a song like it is made in
    genre: String,
    /// for a plan: play (true) or pause (false), as the window's button does
    pub asked: Option<bool>,
}

impl Hears {
    /// shows what was heard in `file`, a song like it to be made in `genre`
    pub fn show(&mut self, heard: Box<Listened>, file: Option<Arc<Picked>>, genre: &str) {
        self.player = None;
        self.loaded = None;
        self.track = TRACKS[0].0.to_string();
        self.genre = genre.to_string();
        self.heard = Some(heard);
        self.file = file;
        self.said.clear();
    }

    /// the words the answer to `analyze` said it in, which come after what was heard
    pub fn said(&mut self, text: &str) {
        self.said = text.to_string();
    }

    pub fn is_open(&self) -> bool {
        self.heard.is_some()
    }

    /// the words what was heard was said in, while the window shows it
    #[cfg(any(not(target_arch = "wasm32"), feature = "plan"))]
    pub fn words(&self) -> Option<&str> {
        self.heard.as_ref().map(|_| self.said.as_str()).filter(|s| !s.is_empty())
    }

    /// what the window shows, for a plan to report: `120 BPM in D minor, 4 bars, playing drums at 3.2 s`
    #[cfg(any(not(target_arch = "wasm32"), feature = "plan"))]
    pub fn told(&self) -> String {
        let Some(l) = &self.heard else { return String::new() };
        let (playing, at) = self.player.as_ref().map_or((false, 0.0), |p| (p.is_playing(), p.position()));
        let doing = if playing { "playing" } else { "paused on" };
        format!("{:.0} BPM in {}, {} bars, {doing} {} at {at:.1} s", l.bpm, l.key, l.bars.len(), self.track)
    }
}

pub fn dialog(ctx: &egui::Context, cx: &mut Cx) {
    if !cx.view.hears.is_open() {
        return;
    }
    let mut h = std::mem::take(&mut cx.view.hears);
    if let (Some(play_it), Some(heard)) = (h.asked.take(), h.heard.clone()) {
        match play_it {
            true => play(ctx, cx, &mut h, &heard),
            false => _ = h.player.as_mut().map(Player::pause),
        }
    }
    let mut open = true;
    let name = h.heard.as_ref().map(|l| l.name.clone()).unwrap_or_default();
    egui::Window::new(format!("What claudius hears in {}", short(&name)))
        .id(egui::Id::unique("hears"))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(780.0)
        .pivot(Align2::CENTER_TOP)
        .default_pos(ctx.content_rect().center_top() + vec2(0.0, 90.0))
        .show(ctx, |ui| body(ui, cx, &mut h));
    if let Some(p) = h.player.as_mut() {
        p.tick();
        // the song and the recording do not play over each other: the one started last plays
        if cx.player.is_playing() && p.is_playing() {
            p.pause();
        }
        if p.is_playing() || p.is_loading() {
            ctx.request_repaint_after(Duration::from_millis(33));
        }
    }
    if open {
        cx.view.hears = h;
    }
}

fn body(ui: &mut Ui, cx: &mut Cx, h: &mut Hears) {
    let Some(heard) = h.heard.clone() else { return };
    facts(ui, &heard);
    ui.add_space(8.0);
    let at = h.player.as_ref().map_or(0.0, Player::position);
    transport(ui, cx, h, &heard, at);
    ui.add_space(6.0);
    now(ui, &heard, at);
    ui.add_space(4.0);
    let shown = h.loaded.is_some();
    if let Some(t) = form(ui, &heard, at, shown).or_else(|| bars_near(ui, &heard, at, shown)) {
        seek(ui.ctx(), cx, h, &heard, t);
    }
    ui.add_space(10.0);
    ui.horizontal_top(|ui| {
        beat(ui, &heard);
        ui.add_space(18.0);
        ui.vertical(|ui| {
            ui.label(RichText::new("Its bass").strong());
            ui.label(small(&heard.bass).color(TEXT_DIM));
            ui.add_space(6.0);
            ui.label(RichText::new("Its chords").strong());
            ui.label(small(progressions(&heard)).color(TEXT_DIM));
        });
    });
    ui.add_space(12.0);
    ui.separator();
    use_it(ui, cx, h, &heard);
}

/// its tempo, key, feel, length and loudness, as chips
fn facts(ui: &mut Ui, l: &Listened) {
    ui.horizontal_wrapped(|ui| {
        let tempo = if l.steady { "steady" } else { "moves with the band" };
        widgets::pill(ui, &format!("{:.0} BPM, {tempo}", l.bpm), ACCENT);
        widgets::pill(ui, &format!("{}, {}", l.key, sureness(l.key_confidence)), theme::INFO);
        widgets::pill(ui, feel(l.swing), theme::GOOD);
        widgets::pill(ui, &format!("{} bars, {}", l.bars.len(), clock(l.secs)), TEXT_DIM);
        if l.lufs.is_finite() {
            widgets::pill(ui, &format!("{:.1} LUFS", l.lufs), TEXT_DIM);
        }
    });
}

/// play and pause, the track to play, and where it is
fn transport(ui: &mut Ui, cx: &mut Cx, h: &mut Hears, l: &Listened, at: f64) {
    let kept: Vec<&str> = l.audio.as_ref().map(|a| a.tracks.iter().map(String::as_str).collect()).unwrap_or_default();
    let playable = !kept.is_empty() && cx.server.is_some() || h.file.is_some();
    ui.horizontal(|ui| {
        let playing = h.player.as_ref().is_some_and(Player::is_playing);
        let (icon, tip) = if playing { (Icon::Pause, "pause it") } else { (Icon::Play, "play it") };
        let r = widgets::icon_button(ui, icon, 28.0, playing, playable, tip);
        if r.clicked() {
            if playing {
                h.player.as_mut().map(Player::pause);
            } else {
                play(ui.ctx(), cx, h, l);
            }
        }
        let length = h.player.as_ref().map(Player::duration).filter(|d| *d > 0.0).unwrap_or(l.secs);
        ui.label(RichText::new(format!("{} / {}", clock(at), clock(length))).monospace().color(TEXT_DIM));
        if h.player.as_ref().is_some_and(Player::is_loading) {
            ui.add(egui::Spinner::new().color(ACCENT));
        }
        if let Some(e) = h.player.as_ref().and_then(Player::error) {
            ui.label(small(e).color(theme::BAD));
        }
        if kept.len() > 1 {
            ui.add_space(12.0);
            ui.label(small("hear").color(TEXT_FAINT));
            for (track, shown) in TRACKS.iter().filter(|(t, _)| kept.contains(t)) {
                let on = h.track == *track;
                let tip = match *track {
                    "recording" => "the recording as it is".to_string(),
                    t => format!("only its {} as claudius took them apart from the rest", shown_of(t)),
                };
                if widgets::chip(ui, on, shown, ACCENT).on_hover_text(tip).clicked() && !on {
                    h.track = track.to_string();
                    if playing {
                        play(ui.ctx(), cx, h, l);
                    }
                }
            }
        }
    });
    if !playable {
        ui.label(small("claudius kept nothing of this recording to play").color(TEXT_FAINT));
    }
}

/// what a part of a recording is called in a sentence
fn shown_of(track: &str) -> &str {
    match track {
        "vocals" => "voice",
        "other" => "other instruments",
        t => t,
    }
}

/// plays the track chosen, from where it was, the song paused
fn play(ctx: &egui::Context, cx: &mut Cx, h: &mut Hears, l: &Listened) {
    let p = h.player.get_or_insert_with(|| Player::new(ctx));
    if h.loaded.as_deref() != Some(h.track.as_str()) {
        let source = match (&l.audio, cx.server, &h.file) {
            (Some(a), Some(base), _) if a.tracks.contains(&h.track) => {
                Source::Url(format!("{base}/api/listened/{}/{}", a.id, h.track))
            }
            (_, _, Some(f)) => Source::File(f.clone()),
            _ => return,
        };
        p.load(source, true);
        h.loaded = Some(h.track.clone());
    }
    cx.player.pause();
    p.play();
}

/// moves the playhead to `t` seconds, playing the recording from there if it was not playing
fn seek(ctx: &egui::Context, cx: &mut Cx, h: &mut Hears, l: &Listened, t: f64) {
    if h.loaded.is_none() {
        play(ctx, cx, h, l);
    }
    if let Some(p) = h.player.as_mut() {
        p.seek(t.max(0.0));
    }
}

/// the section and the chord playing, and the chord after it
fn now(ui: &mut Ui, l: &Listened, at: f64) {
    let Some(b) = bar_at(l, at) else {
        ui.label(small("press play: the form and the chords below follow it").color(TEXT_FAINT));
        return;
    };
    let spans = chord_spans(l);
    let i = spans.iter().position(|s| (s.0..s.1).contains(&b)).unwrap_or(0);
    ui.horizontal(|ui| {
        if let Some(s) = section_of(l, b) {
            ui.label(RichText::new(&s.kind).strong().color(theme::section_color(&s.kind)));
        }
        ui.label(small(format!("bar {} of {}", b + 1, l.bars.len())).color(TEXT_DIM));
        ui.add_space(10.0);
        let chord = spans.get(i).map_or("", |s| s.2.as_str());
        ui.label(RichText::new(if chord.is_empty() { "no chord" } else { chord }).size(22.0).strong().color(TEXT));
        if let Some(next) = spans.get(i + 1) {
            ui.label(small(format!("then {}", next.2)).color(TEXT_DIM));
        }
    });
}

/// The form: each section a block as long as it plays, its kind and energy in its colour, the chords under it as long
/// as each lasts, and the playhead through both. Where a press or a drag on it asks to play from, in seconds.
fn form(ui: &mut Ui, l: &Listened, at: f64, shown: bool) -> Option<f64> {
    let total = l.secs.max(l.end).max(1e-3);
    let (rect, r) = ui.allocate_exact_size(vec2(ui.available_width(), 62.0), Sense::click_and_drag());
    let p = ui.painter_at(rect);
    let x = |t: f64| rect.left() + (t / total).clamp(0.0, 1.0) as f32 * rect.width();
    let start = |b: usize| l.bars.get(b).map_or(l.end, |bar| bar.start);
    p.rect_filled(rect, 4.0, WELL);
    let band = |top: f32, h: f32, t0: f64, t1: f64| Rect::from_min_max(pos2(x(t0), top), pos2(x(t1), top + h));
    let playing = bar_at(l, at).filter(|_| shown);
    for s in &l.sections {
        let cell =
            band(rect.top() + 2.0, 30.0, start(s.start_bar), start(s.start_bar + s.bars)).shrink2(vec2(1.0, 0.0));
        let c = theme::section_color(&s.kind);
        p.rect_filled(cell, 3.0, theme::alpha(c, 0.3 + 0.55 * s.energy.clamp(0.0, 1.0)));
        if playing.is_some_and(|b| (s.start_bar..s.start_bar + s.bars).contains(&b)) {
            p.rect_stroke(cell, 3.0, Stroke::new(1.5, c), egui::StrokeKind::Inside);
        }
        let label = format!("{} {}", s.kind, s.bars);
        fit(&p, cell, &label, 11.0, TEXT);
    }
    // each chord in the colour of its root, so that a progression coming round shows as its colours coming round
    for (b0, b1, chord) in chord_spans(l) {
        let cell = band(rect.top() + 35.0, 24.0, start(b0), start(b1)).shrink2(vec2(0.5, 0.0));
        let on = playing.is_some_and(|b| (b0..b1).contains(&b));
        p.rect_filled(cell, 2.0, theme::alpha(root_color(&chord), if on { 0.95 } else { 0.55 }));
        fit(&p, cell, &chord, 11.5, TEXT);
    }
    if shown {
        p.vline(x(at), rect.y_range(), Stroke::new(2.0, ACCENT));
    }
    let r = r.on_hover_text("press or drag to play from there");
    let t = r.interact_pointer_pos().filter(|_| r.clicked() || r.dragged());
    t.map(|pos| ((pos.x - rect.left()) / rect.width().max(1.0)).clamp(0.0, 1.0) as f64 * total)
}

/// how many bars the strip of bars around the playhead shows, and how many of them come before the bar playing
const NEAR: usize = 12;
const BEFORE: usize = 2;

/// The bars around the playhead, each as wide as a chord's name needs, with its chord written in it, the bar playing
/// lit: the chords to read as it plays, which the form is too small for. Where a press on a bar asks to play from.
fn bars_near(ui: &mut Ui, l: &Listened, at: f64, shown: bool) -> Option<f64> {
    if l.bars.is_empty() {
        return None;
    }
    let playing = bar_at(l, at).filter(|_| shown);
    let first = playing.map_or(0, |b| b.saturating_sub(BEFORE)).min(l.bars.len().saturating_sub(NEAR));
    let last = (first + NEAR).min(l.bars.len());
    let (rect, r) = ui.allocate_exact_size(vec2(ui.available_width(), 46.0), Sense::click());
    let p = ui.painter_at(rect);
    let w = rect.width() / NEAR as f32;
    let mut asked = None;
    for b in first..last {
        let cell = Rect::from_min_size(pos2(rect.left() + (b - first) as f32 * w, rect.top()), vec2(w, rect.height()))
            .shrink(1.5);
        let on = playing == Some(b);
        let chord = &l.bars[b].chord;
        let held = b > first && l.bars[b - 1].chord == *chord;
        p.rect_filled(cell, 3.0, theme::alpha(root_color(chord), if on { 0.9 } else { 0.28 }));
        if on {
            p.rect_stroke(cell, 3.0, Stroke::new(2.0, ACCENT), egui::StrokeKind::Inside);
        }
        let font = FontId::monospace(9.5);
        p.text(cell.left_top() + vec2(4.0, 3.0), Align2::LEFT_TOP, (b + 1).to_string(), font, TEXT_FAINT.into());
        // a chord held from the bar before is a dash, as a chart writes it
        let name = if held { "-" } else { chord.as_str() };
        let room = cell.translate(vec2(0.0, 6.0));
        let _ = [15.0, 12.5, 10.5].into_iter().find(|&size| fit(&p, room, name, size, TEXT));
        if r.clicked() && r.interact_pointer_pos().is_some_and(|pos| cell.contains(pos)) {
            asked = Some(l.bars[b].start);
        }
    }
    asked
}

/// a colour for a chord by its root, the twelve roots around the colour wheel by fifths, so that chords a fifth apart
/// are near in colour; grey for none
fn root_color(chord: &str) -> Color32 {
    let mut letters = chord.chars();
    let pc = match letters.next() {
        Some('C') => 0,
        Some('D') => 2,
        Some('E') => 4,
        Some('F') => 5,
        Some('G') => 7,
        Some('A') => 9,
        Some('B') => 11,
        _ => return Color32::from_gray(120),
    };
    let pc = match letters.next() {
        Some('#') => pc + 1,
        Some('b') => pc + 11,
        _ => pc,
    } % 12;
    let fifths = (pc * 7) % 12;
    egui::ecolor::Hsva::new(fifths as f32 / 12.0, 0.55, 0.85, 1.0).into()
}

/// `text` in `cell`, from its left, when it fits; whether it did
fn fit(p: &egui::Painter, cell: Rect, text: &str, size: f32, color: impl Into<Color32>) -> bool {
    let color = color.into();
    let galley = p.layout_no_wrap(text.to_string(), FontId::proportional(size), color);
    let fits = galley.size().x + 8.0 <= cell.width();
    if fits {
        p.galley(pos2(cell.left() + 4.0, cell.center().y - galley.size().y / 2.0), galley, color);
    }
    fits
}

/// its beat: how often the kick, the snare and the hats strike each sixteenth of a bar
fn beat(ui: &mut Ui, l: &Listened) {
    ui.vertical(|ui| {
        ui.label(RichText::new("Its beat").strong());
        let (cell, gap) = (14.0, 2.0);
        let rows = [("kick", &l.kick), ("snare", &l.snare), ("hats", &l.hats)];
        let width = 42.0 + 16.0 * (cell + gap) + 3.0 * 4.0;
        let (rect, _) = ui.allocate_exact_size(vec2(width, rows.len() as f32 * (cell + gap)), Sense::hover());
        let p = ui.painter_at(rect);
        for (row, (name, steps)) in rows.iter().enumerate() {
            let y = rect.top() + row as f32 * (cell + gap);
            let font = FontId::proportional(11.0);
            p.text(pos2(rect.left(), y + cell / 2.0), Align2::LEFT_CENTER, name, font, TEXT_DIM.into());
            for step in 0..16 {
                let v = steps.get(step).copied().unwrap_or(0.0).clamp(0.0, 1.0);
                let left = rect.left() + 42.0 + step as f32 * (cell + gap) + (step / 4) as f32 * 4.0;
                let r = Rect::from_min_size(pos2(left, y), vec2(cell, cell));
                p.rect_filled(r, 2.0, theme::alpha(TEXT, 0.06));
                if v > 0.0 {
                    p.rect_filled(r, 2.0, theme::alpha(ACCENT, 0.15 + 0.85 * v));
                }
            }
        }
    });
}

/// its progressions as Roman numerals in its key, and how long each chord lasts
fn progressions(l: &Listened) -> String {
    if l.progression_a.is_empty() {
        return "no progression claudius is sure of".into();
    }
    let each = match l.bars_per_chord {
        1 => "a bar each".to_string(),
        n => format!("{n} bars each"),
    };
    match l.progression_b.is_empty() || l.progression_b == l.progression_a {
        true => format!("{}, {each}", l.progression_a.join(" ")),
        false => format!("{}, then {}, {each}", l.progression_a.join(" "), l.progression_b.join(" ")),
    }
}

/// a song like it, its tempo, key and chords for the song that is open, and what it heard in words
fn use_it(ui: &mut Ui, cx: &mut Cx, h: &mut Hears, l: &Listened) {
    ui.horizontal_wrapped(|ui| {
        let label = RichText::new("  Make a song like it  ").strong().color(theme::ink_on(ACCENT));
        let like = egui::Button::new(label).fill(ACCENT);
        let tip = "a new song in the genre beside: its tempo, key, chords, form and swing, none of its sound";
        if ui.add(like).on_hover_text(tip).clicked() {
            cx.out.push(Command::SongLike { genre: Some(h.genre.clone()), seed: None });
        }
        ui.label("in");
        egui::ComboBox::from_id_salt("hears_genre")
            .width(160.0)
            .height(320.0)
            .selected_text(super::genre_words(&h.genre))
            .show_ui(ui, |ui| {
                for g in &cx.state.genres {
                    if ui.selectable_label(h.genre == g.name, super::genre_words(&g.name)).clicked() {
                        h.genre = g.name.clone();
                    }
                }
            });
        ui.add_space(12.0);
        let song = cx.state.song.is_some();
        let tempo = ui
            .add_enabled(song, egui::Button::new(format!("Its tempo, {:.0} BPM", l.bpm)))
            .on_hover_text("the open song at its tempo")
            .on_disabled_hover_text("make or open a song first");
        if tempo.clicked() {
            cx.out.push(Command::RunScript { code: format!("tempo({:.0});", l.bpm) });
        }
        let chords = ui
            .add_enabled(song && !l.progression_a.is_empty(), egui::Button::new("Its key and chords"))
            .on_hover_text(format!("the open song in {}, on its chords", l.key))
            .on_disabled_hover_text(if song {
                "claudius is sure of no progression in it"
            } else {
                "make or open a song first"
            });
        if chords.clicked() {
            cx.out.push(Command::RunScript { code: key_and_chords(l) });
        }
        let copy = ui
            .add_enabled(!h.said.is_empty(), egui::Button::new("Copy it as text"))
            .on_hover_text("what claudius hears in it, in words, to paste into a note or for an assistant");
        if copy.clicked() {
            ui.ctx().copy_text(h.said.clone());
        }
    });
}

/// the script that gives a song a recording's key and its two progressions
pub fn key_and_chords(l: &Listened) -> String {
    let b = if l.progression_b.is_empty() { &l.progression_a } else { &l.progression_b };
    format!(
        "key(\"{}\");\nprog(\"A\", \"{}\");\nprog(\"B\", \"{}\");\nbars_per_chord({});",
        l.key,
        l.progression_a.join(" "),
        b.join(" "),
        l.bars_per_chord.max(1)
    )
}

/// the bar playing at `t` seconds, counting from 0: none before the first or after the last
fn bar_at(l: &Listened, t: f64) -> Option<usize> {
    let i = l.bars.partition_point(|b| b.start <= t);
    (i > 0 && t < l.end).then(|| i - 1)
}

fn section_of(l: &Listened, bar: usize) -> Option<&SectionHeard> {
    l.sections.iter().find(|s| (s.start_bar..s.start_bar + s.bars).contains(&bar))
}

/// the bars a chord lasts, one after another: its first bar, the bar after its last, and its name
fn chord_spans(l: &Listened) -> Vec<(usize, usize, String)> {
    let mut spans: Vec<(usize, usize, String)> = Vec::new();
    for (b, bar) in l.bars.iter().enumerate() {
        match spans.last_mut() {
            Some(s) if s.2 == bar.chord => s.1 = b + 1,
            _ => spans.push((b, b + 1, bar.chord.clone())),
        }
    }
    spans
}

/// how sure claudius is of a key, in a word
fn sureness(confidence: f64) -> &'static str {
    match confidence {
        c if c >= 0.7 => "sure",
        c if c >= 0.45 => "likely",
        _ => "a guess",
    }
}

/// how its sixteenths swing, in words
fn feel(swing: f32) -> &'static str {
    match swing {
        s if s < 0.54 => "straight",
        s if s < 0.6 => "a light swing",
        _ => "swung",
    }
}

/// seconds as minutes and seconds: `3:07`
fn clock(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

/// a file's name, short enough for a title
fn short(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    match base.chars().count() {
        n if n > 48 => format!("{}...", base.chars().take(45).collect::<String>()),
        _ => base.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use claudius_api::BarHeard;

    fn heard() -> Listened {
        let bar = |start: f64, chord: &str| BarHeard { start, chord: chord.into(), numeral: String::new() };
        Listened {
            name: "song.mp3".into(),
            secs: 10.0,
            bpm: 120.0,
            key: "D minor".into(),
            bars: vec![bar(0.5, "Dm"), bar(2.5, "Dm"), bar(4.5, "Bb"), bar(6.5, "C")],
            end: 8.5,
            progression_a: vec!["i".into(), "VI".into(), "VII".into()],
            bars_per_chord: 2,
            ..Default::default()
        }
    }

    #[test]
    fn the_playhead_is_in_the_bar_it_has_reached_and_in_none_outside_them() {
        let l = heard();
        assert_eq!(bar_at(&l, 0.2), None);
        assert_eq!(bar_at(&l, 0.5), Some(0));
        assert_eq!(bar_at(&l, 4.4), Some(1));
        assert_eq!(bar_at(&l, 8.4), Some(3));
        assert_eq!(bar_at(&l, 8.5), None);
    }

    #[test]
    fn a_chord_is_coloured_by_its_root_whatever_its_quality() {
        assert_eq!(root_color("Bbmaj7"), root_color("A#"));
        assert_eq!(root_color("Dm7"), root_color("D"));
        assert_ne!(root_color("C"), root_color("G"));
        assert_eq!(root_color(""), Color32::from_gray(120));
    }

    #[test]
    fn a_chord_held_over_bars_is_one_span() {
        let spans = chord_spans(&heard());
        assert_eq!(spans, vec![(0, 2, "Dm".into()), (2, 3, "Bb".into()), (3, 4, "C".into())]);
    }

    #[test]
    fn the_key_and_chords_are_given_as_a_script_the_song_runs() {
        let code = key_and_chords(&heard());
        assert_eq!(
            code,
            "key(\"D minor\");\nprog(\"A\", \"i VI VII\");\nprog(\"B\", \"i VI VII\");\nbars_per_chord(2);"
        );
    }
}

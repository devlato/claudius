//! The script editor: the song as a claudius script (Rhai with Strudel-style patterns), highlighted (strings, numbers,
//! comments, functions and methods), with line numbers, Run (Cmd/Ctrl + Enter), the result of the last run (an error
//! marks its line), the sounds a script may use (click one to insert it at the cursor) and the language reference.

use std::sync::Arc;

use claudius_api::SoundList;
use eframe::egui::{
    self, Color32, FontId, RichText, Ui,
    text::{LayoutJob, TextFormat},
};

use super::{Cx, page_frame, small};
use crate::theme::{self, ACCENT, TEXT, TEXT_DIM, TEXT_FAINT, WELL};
use crate::widgets::{self, Icon};

const KEYWORDS: [&str; 16] = [
    "let", "const", "fn", "if", "else", "for", "in", "while", "loop", "return", "true", "false", "switch", "break",
    "continue", "this",
];

#[derive(Clone, Copy, PartialEq)]
enum Tok {
    Plain,
    Comment,
    Str,
    Num,
    Func,
    Method,
    Keyword,
    Punct,
}

fn color(t: Tok) -> Color32 {
    match t {
        Tok::Plain => Color32::from_rgb(214, 218, 226),
        Tok::Comment => Color32::from_rgb(106, 118, 110),
        Tok::Str => Color32::from_rgb(166, 214, 128),
        Tok::Num => Color32::from_rgb(200, 150, 255),
        Tok::Func => Color32::from_rgb(255, 196, 112),
        Tok::Method => Color32::from_rgb(110, 205, 240),
        Tok::Keyword => Color32::from_rgb(255, 120, 170),
        Tok::Punct => Color32::from_rgb(140, 148, 160),
    }
}

/// a small tokenizer: good enough for colouring
fn tokens(src: &str) -> Vec<(std::ops::Range<usize>, Tok)> {
    let b = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        let start = i;
        let tok = if c == b'/' && b.get(i + 1) == Some(&b'/') {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            Tok::Comment
        } else if c == b'/' && b.get(i + 1) == Some(&b'*') {
            i += 2;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(b.len());
            Tok::Comment
        } else if c == b'"' || c == b'\'' || c == b'`' {
            i += 1;
            while i < b.len() && b[i] != c && b[i] != b'\n' {
                if b[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            i = (i + 1).min(b.len());
            Tok::Str
        } else if c.is_ascii_digit()
            || (c == b'.' && b.get(i + 1).is_some_and(|d| d.is_ascii_digit()))
            || (c == b'-'
                && b.get(i + 1).is_some_and(|d| d.is_ascii_digit())
                && (i == 0 || !b[i - 1].is_ascii_alphanumeric()))
        {
            i += 1;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'.' || b[i] == b'_') {
                i += 1;
            }
            Tok::Num
        } else if c.is_ascii_alphabetic() || c == b'_' || c == b'$' {
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'$') {
                i += 1;
            }
            let word = &src[start..i];
            let mut j = i;
            while j < b.len() && b[j] == b' ' {
                j += 1;
            }
            let call = b.get(j) == Some(&b'(');
            let after_dot = start > 0 && b[start - 1] == b'.';
            if KEYWORDS.contains(&word) {
                Tok::Keyword
            } else if call && after_dot {
                Tok::Method
            } else if call {
                Tok::Func
            } else {
                Tok::Plain
            }
        } else if b"()[]{}<>,.;:=+*|!?&%^~".contains(&c) {
            i += 1;
            Tok::Punct
        } else {
            // whitespace or a multi-byte char: copy as plain up to the next interesting byte
            i += 1;
            while i < b.len() && !b[i].is_ascii_graphic() && b[i] != b'\n' && !src.is_char_boundary(i) {
                i += 1;
            }
            while i < b.len() && !src.is_char_boundary(i) {
                i += 1;
            }
            Tok::Plain
        };
        out.push((start..i, tok));
    }
    out
}

/// The number in `src` the byte `at` is in or just after (a cursor in it, after typing it, or past the bracket or comma
/// after it): its bytes, by the tokens `tokens` reads, so that none in a string or a comment counts.
fn number_at(src: &str, at: usize) -> Option<std::ops::Range<usize>> {
    let numbers: Vec<std::ops::Range<usize>> =
        tokens(src).into_iter().filter(|(_, t)| *t == Tok::Num).map(|(r, _)| r).collect();
    let within = numbers.iter().find(|r| r.start <= at && at <= r.end);
    within.or_else(|| numbers.iter().find(|r| r.end < at && at == r.end + 1)).cloned()
}

/// `number` moved `by` steps of its last digit (1 for 120, 0.1 for 0.5, 0.01 for 0.56) and written as it was, with as
/// many decimals; none for a number written otherwise (hex, an exponent, digits apart)
fn nudged(number: &str, by: i32) -> Option<String> {
    if !number.bytes().all(|c| c.is_ascii_digit() || c == b'.' || c == b'-') {
        return None;
    }
    let v: f64 = number.parse().ok()?;
    let decimals = number.split_once('.').map_or(0, |(_, d)| d.len());
    let step = 10f64.powi(-(decimals as i32));
    let x = v + f64::from(by) * step;
    // no "-0.0" for a number nudged to nothing
    let x = if x.abs() < step / 2.0 { 0.0 } else { x };
    Some(format!("{x:.decimals$}"))
}

/// a frequency in words: "120 Hz", "4.5 kHz"
fn hz(v: f64) -> String {
    if v >= 1000.0 { format!("{} kHz", (v / 100.0).round() / 10.0) } else { format!("{v:.0} Hz") }
}

/// a share in words: "56%"
fn share(v: f64) -> String {
    format!("{:.0}%", v * 100.0)
}

/// where a pan puts a sound, in words
fn pan_words(v: f64) -> String {
    match v {
        v if v.abs() < 0.005 => "in the middle".into(),
        v if v < 0.0 => format!("{} left", share(-v)),
        v => format!("{} right", share(v)),
    }
}

/// a number as the script writes it: whole numbers without a point
fn plain(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1e9 { format!("{}", v as i64) } else { format!("{v}") }
}

/// What a number given to call `name`, as its argument `arg` (from 0), sets, in a few words: "118 BPM", "16 bars"
fn call_hint(name: &str, arg: usize, v: f64) -> Option<String> {
    Some(match (name, arg) {
        ("tempo" | "bpm", 0) => format!("{} BPM", plain(v)),
        ("swing", 0) => format!("swing {}", share(v)),
        ("humanize", 0) => format!("humanized {}", share(v)),
        ("bars_per_chord", 0) => format!("{} bar{} a chord", plain(v), if v == 1.0 { "" } else { "s" }),
        ("sec", 1) => format!("{} bars", plain(v)),
        ("energy", 0) => format!("energy {}", share(v)),
        ("build", 1) => format!("builds {} hard", share(v)),
        ("octave", 0) => format!("octave {}", plain(v)),
        ("lpf" | "hpf" | "cutoff", 0) => hz(v),
        ("room", 0) => format!("{} reverb", share(v)),
        ("delay", 0) => format!("{} delay", share(v)),
        ("level" | "gain", 0) => format!("{} dB", signed(v)),
        ("legato", 0) => format!("{} of each step", share(v)),
        ("pan", 0) => pan_words(v),
        ("duck", 0) => format!("{} ducked under the kick", share(v)),
        ("transpose", 0) => format!("{} semitones", signed(v)),
        ("voicing", 0) => format!("from {}", crate::music::note_name(v as i32)),
        ("voicing", 1) => format!("up to {}", crate::music::note_name(v as i32)),
        _ => return None,
    })
}

/// a number with its sign: "+2", "-4.5"
fn signed(v: f64) -> String {
    if v > 0.0 { format!("+{}", plain(v)) } else { plain(v) }
}

/// What a number given as a map's field `key` sets, in a few words: "-4 dB", "30% left"
fn key_hint(key: &str, v: f64) -> Option<String> {
    Some(match key {
        "level" | "gain_db" | "threshold_db" | "makeup_db" | "drive_db" | "drive" | "hiss_db" => {
            format!("{} dB", signed(v))
        }
        "ceiling" => format!("{} dBTP", plain(v)),
        "lufs" => format!("{} LUFS", plain(v)),
        "pan" => pan_words(v),
        "reverb" | "delay" | "duck" | "mix" | "density" | "amount" | "prob" | "depth" | "crackle" => share(v),
        "cutoff" | "freq" | "rate_hz" | "mono_below_hz" | "lowpass_hz" | "highpass_hz" => hz(v),
        "bars" => format!("{} bars", plain(v)),
        "beats" => format!("{} beats", plain(v)),
        "transpose" => format!("{} semitones", signed(v)),
        "width" => format!("{} wide", share(v)),
        "decay" => format!("{} s", plain(v)),
        "attack_ms" | "release_ms" | "delay_ms" | "depth_ms" | "predelay_ms" | "late_ms" => format!("{} ms", plain(v)),
        "ratio" => format!("{}:1", plain(v)),
        "res" => format!("resonance {}", share(v)),
        _ => return None,
    })
}

/// What each number of `src` sets, in a few words, by the call it is given to or the map field it fills: each by its
/// bytes in `src`, for the hints the editor shows at the end of its line and over it.
fn hints(src: &str) -> Vec<(std::ops::Range<usize>, String)> {
    enum Frame {
        Call(String, usize),
        Map,
        List,
    }
    let mut out = Vec::new();
    let mut frames: Vec<Frame> = Vec::new();
    let (mut word, mut key): (Option<String>, Option<String>) = (None, None);
    for (r, t) in tokens(src) {
        let s = &src[r.clone()];
        match t {
            Tok::Func | Tok::Method => word = Some(s.to_string()),
            Tok::Plain if !s.trim().is_empty() && !s.starts_with('#') => word = Some(s.trim().to_string()),
            Tok::Punct => match s {
                "(" => frames.push(Frame::Call(word.take().unwrap_or_default(), 0)),
                "{" => frames.push(Frame::Map),
                "[" => frames.push(Frame::List),
                ")" | "}" | "]" => {
                    frames.pop();
                }
                "," => {
                    if let Some(Frame::Call(_, arg)) = frames.last_mut() {
                        *arg += 1;
                    }
                    key = None;
                }
                ":" if matches!(frames.last(), Some(Frame::Map)) => key = word.take(),
                _ => {}
            },
            Tok::Num => {
                let Ok(v) = s.replace('_', "").parse::<f64>() else { continue };
                let hint = match (&key, frames.last()) {
                    (Some(k), Some(Frame::Map)) => key_hint(k, v),
                    (_, Some(Frame::Call(name, arg))) => call_hint(name, *arg, v),
                    _ => None,
                };
                if let Some(h) = hint {
                    out.push((r, h));
                }
                key = None;
            }
            _ => {}
        }
    }
    out
}

/// the script coloured; `error_line` (1-based) gets a red background
pub(crate) fn highlight(src: &str, font: FontId, error_line: Option<usize>) -> LayoutJob {
    let mut job = LayoutJob::default();
    let mut line = 1;
    for (range, tok) in tokens(src) {
        let piece = &src[range];
        let background = if error_line == Some(line) && piece != "\n" {
            theme::alpha(theme::BAD, 0.28)
        } else {
            Color32::TRANSPARENT
        };
        let fmt = TextFormat {
            font_id: font.clone(),
            color: color(tok),
            italics: tok == Tok::Comment,
            background,
            ..Default::default()
        };
        job.append(piece, 0.0, fmt);
        line += piece.matches('\n').count();
    }
    job
}

/// where a Rhai error points: "... (line 3, position 14)" -> (3, 14), both 1-based
pub fn error_location(msg: &str) -> Option<(usize, usize)> {
    let at = msg.rfind("(line ")?;
    let rest = &msg[at + "(line ".len()..];
    let digits = |s: &str| s.chars().take_while(char::is_ascii_digit).collect::<String>().parse::<usize>().ok();
    let line = digits(rest)?;
    let pos = rest.find("position ").and_then(|i| digits(&rest[i + "position ".len()..])).unwrap_or(1);
    Some((line.max(1), pos.max(1)))
}

/// the char index of (line, column), both 1-based
fn char_index(text: &str, line: usize, col: usize) -> usize {
    let mut idx = 0;
    for (n, l) in text.split('\n').enumerate() {
        let len = l.chars().count();
        if n + 1 == line {
            return idx + (col - 1).min(len);
        }
        idx += len + 1;
    }
    text.chars().count()
}

fn insert(ed: &mut super::ScriptEditor, what: &str) {
    let n = ed.text.chars().count();
    let at = ed.cursor.unwrap_or(n).min(n);
    let byte = ed.text.char_indices().nth(at).map_or(ed.text.len(), |(b, _)| b);
    ed.text.insert_str(byte, what);
    ed.cursor = Some(at + what.chars().count());
    ed.move_cursor = ed.cursor;
    ed.dirty = true;
}

/// the sound browser: drums, synth presets, sampled instruments by pack, and the short names
fn sounds(ui: &mut Ui, ed: &mut super::ScriptEditor, list: &SoundList) {
    let f = ed.filter.to_lowercase();
    let filtering = !f.is_empty();
    let chips = |ui: &mut Ui, ed: &mut super::ScriptEditor, names: &[(String, String)], tip: &str| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
            for (label, insert_as) in names {
                let about = list.sound_about(insert_as).map_or(String::new(), |a| format!("{a}\n"));
                if ui
                    .small_button(label.as_str())
                    .on_hover_text(format!("{about}{tip}: inserts \"{insert_as}\" at the cursor"))
                    .clicked()
                {
                    insert(ed, &format!("\"{insert_as}\""));
                }
            }
        });
    };
    let found = |n: &&String| super::finds_sound(&f, n, Some(list));
    let drums: Vec<(String, String)> = list.drums.iter().filter(found).map(|n| (n.clone(), n.clone())).collect();
    let synths: Vec<(String, String)> = list.synths.iter().filter(found).map(|n| (n.clone(), n.clone())).collect();
    let mut packs: Vec<(&str, Vec<(String, String)>)> = Vec::new();
    for full in list.instruments.iter().filter(|n| n.contains(':') && found(n)) {
        let (pack, name) = full.split_once(':').unwrap_or(("", full));
        match packs.iter_mut().find(|(p, _)| *p == pack) {
            Some((_, v)) => v.push((name.to_string(), full.clone())),
            None => packs.push((pack, vec![(name.to_string(), full.clone())])),
        }
    }
    let aliases: Vec<(String, String)> =
        list.instruments.iter().filter(|n| !n.contains(':') && found(n)).map(|n| (n.clone(), n.clone())).collect();
    let sampled: usize = packs.iter().map(|(_, v)| v.len()).sum();
    let group = |title: String, open: bool| {
        egui::CollapsingHeader::new(RichText::new(title).size(12.0).strong())
            .default_open(open)
            .open(filtering.then_some(true))
    };
    if !drums.is_empty() {
        group(format!("Drums · {}", drums.len()), true).show(ui, |ui| chips(ui, ed, &drums, "a drum voice for s(...)"));
    }
    if !synths.is_empty() {
        group(format!("Synth presets · {}", synths.len()), true)
            .show(ui, |ui| chips(ui, ed, &synths, "a synth preset"));
    }
    if sampled > 0 {
        group(format!("Sampled instruments · {sampled} in {} packs", packs.len()), false).show(ui, |ui| {
            for (pack, names) in &packs {
                egui::CollapsingHeader::new(
                    RichText::new(format!("{pack} · {}", names.len())).size(11.5).color(TEXT_DIM),
                )
                .id_salt(("pack", *pack))
                .default_open(false)
                .open(filtering.then_some(true))
                .show(ui, |ui| {
                    chips(ui, ed, names, &format!("from the {pack} pack"));
                });
            }
        });
    }
    if !aliases.is_empty() {
        group(format!("Short names · {}", aliases.len()), false)
            .show(ui, |ui| chips(ui, ed, &aliases, "a short name for a sampled instrument or kit sound"));
    }
    if drums.is_empty() && synths.is_empty() && sampled == 0 && aliases.is_empty() {
        ui.label(small(format!("nothing matches '{}'", ed.filter)));
    }
}

pub fn show(ui: &mut Ui, cx: &mut Cx) {
    let Some(song) = cx.state.song.as_ref() else {
        super::empty(ui, "No song yet.");
        return;
    };
    let now = cx.now;
    let ed = &mut cx.view.script;
    let reference_open = &mut cx.view.reference_open;
    let mut run = false;
    let error = ed.result.as_ref().filter(|(ok, _)| !ok).and_then(|(_, m)| error_location(m));
    page_frame().show(ui, |ui| {
        widgets::row(ui, "script_tools", |ui, side| {
            if side == widgets::Side::Right {
                ui.label(small(format!("{} lines", ed.text.lines().count())));
                return;
            }
            widgets::heading(ui, "Script");
            let cmd = widgets::cmd_key(ui.ctx());
            if widgets::tool_button(ui, Icon::Play, "Run", !ed.awaiting, true)
                .on_hover_text(format!("run the script against the song ({cmd}+Enter)"))
                .clicked()
            {
                run = true;
            }
            if widgets::tool_button(ui, Icon::Undo, "Revert", ed.dirty, false)
                .on_hover_text("throw away your edits and show the song's script")
                .clicked()
            {
                ed.text = song.script.clone();
                ed.base = song.script.clone();
                ed.dirty = false;
                ed.result = None;
                ed.nudged = None;
            }
            if widgets::icon_button(ui, Icon::Copy, 28.0, false, true, "copy the script").clicked() {
                ui.ctx().copy_text(ed.text.clone());
            }
            if widgets::tool_button(ui, Icon::Open, "Reference", true, *reference_open)
                .on_hover_text("the script language reference")
                .clicked()
            {
                *reference_open = !*reference_open;
            }
            let tip = "what each number sets, at the end of its line and over it. Alt+↑ ↓ change the number at the \
                       cursor (Shift: ten times as much), and the song plays the change";
            if widgets::chip(ui, !ed.no_hints, "Hints", ACCENT).on_hover_text(tip).clicked() {
                ed.no_hints = !ed.no_hints;
            }
            if ed.dirty {
                widgets::pill(ui, "edited", ACCENT);
                if ed.base != song.script {
                    ui.label(RichText::new("the song changed since you started editing").color(theme::WARN).size(11.0));
                }
            } else {
                widgets::pill(ui, "in sync with the song", TEXT_DIM);
            }
        });
        if let Some((ok, msg)) = ed.result.clone() {
            let c = if ok { theme::GOOD } else { theme::BAD };
            egui::Frame::new()
                .fill(theme::alpha(c, 0.1))
                .stroke(egui::Stroke::new(1.0, theme::alpha(c, 0.5)))
                .corner_radius(4)
                .inner_margin(egui::Margin::symmetric(8, 4))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        if let Some((line, pos)) = error
                            && ui
                                .small_button(format!("go to line {line}"))
                                .on_hover_text("put the cursor where the error is")
                                .clicked()
                        {
                            ed.move_cursor = Some(char_index(&ed.text, line, pos));
                        }
                        let mark = if ok { "ok:" } else { "error:" };
                        ui.add(
                            egui::Label::new(
                                RichText::new(format!("{mark} {msg}"))
                                    .color(theme::mix(c, TEXT, 0.4))
                                    .monospace()
                                    .size(12.0),
                            )
                            .wrap(),
                        );
                    });
                });
        }
        ui.add_space(4.0);
        let list = cx.sounds;
        egui::Panel::right("script_sounds")
            .resizable(true)
            .default_size(240.0)
            .size_range(170.0..=420.0)
            .frame(egui::Frame::new().inner_margin(egui::Margin { left: 8, right: 0, top: 0, bottom: 0 }))
            .show(ui, |ui| {
                widgets::heading(ui, "Sounds");
                ui.add(
                    egui::TextEdit::singleline(&mut ed.filter)
                        .hint_text("search drums, synths, instruments")
                        .desired_width(f32::INFINITY),
                );
                egui::ScrollArea::vertical().id_salt("sound_list").auto_shrink([false, false]).show(
                    ui,
                    |ui| match list {
                        Some(list) => sounds(ui, ed, list),
                        None => {
                            ui.label(small("the sounds appear here once claudius has listed them"));
                        }
                    },
                );
            });
        let font = theme::mono(13.0);
        let lines = ed.text.lines().count().max(1) + usize::from(ed.text.ends_with('\n'));
        egui::Frame::new().fill(WELL).corner_radius(6).inner_margin(egui::Margin::symmetric(6, 6)).show(ui, |ui| {
            egui::ScrollArea::both().id_salt("script_scroll").auto_shrink([false, false]).show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    let mut numbers = LayoutJob::default();
                    for n in 1..=lines {
                        let bad = error.is_some_and(|(l, _)| l == n);
                        let fmt = TextFormat {
                            font_id: font.clone(),
                            color: if bad { theme::BAD } else { TEXT_FAINT },
                            ..Default::default()
                        };
                        numbers.append(&format!("{n:>3}{}", if n < lines { "\n" } else { "" }), 0.0, fmt);
                    }
                    ui.label(numbers);
                    let error_line = error.map(|(l, _)| l);
                    let mut layouter = |ui: &Ui, text: &dyn egui::TextBuffer, _wrap: f32| -> Arc<egui::Galley> {
                        let mut job = highlight(text.as_str(), font.clone(), error_line);
                        job.wrap.max_width = f32::INFINITY;
                        ui.fonts_mut(|f| f.layout_job(job))
                    };
                    let id = ui.scope_id().with("script_text");
                    // the keys that nudge the number at the cursor, before the text takes them
                    if ui.memory(|m| m.has_focus(id))
                        && let Some(at) = ed.cursor
                    {
                        let by = ui.input_mut(|i| {
                            let (alt, more) = (egui::Modifiers::ALT, egui::Modifiers::ALT | egui::Modifiers::SHIFT);
                            [(more, egui::Key::ArrowUp, 10), (more, egui::Key::ArrowDown, -10)]
                                .into_iter()
                                .chain([(alt, egui::Key::ArrowUp, 1), (alt, egui::Key::ArrowDown, -1)])
                                .find(|(m, k, _)| i.consume_key(*m, *k))
                                .map_or(0, |(_, _, by)| by)
                        });
                        if by != 0 && nudge(ed, at, by, now) {
                            ed.dirty = ed.text != song.script;
                        }
                    }
                    if let Some(at) = ed.move_cursor.take() {
                        let mut state = egui::TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
                        state.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(at))));
                        state.store(ui.ctx(), id);
                        ui.memory_mut(|m| m.request_focus(id));
                    }
                    let output = egui::TextEdit::multiline(&mut ed.text)
                        .id(id)
                        .font(font.clone())
                        .code_editor()
                        .frame(egui::Frame::NONE)
                        .desired_width(f32::INFINITY)
                        .desired_rows(24)
                        .lock_focus(true)
                        .layouter(&mut layouter)
                        .show(ui);
                    if let Some(row) = error_line.and_then(|l| output.galley.rows.get(l - 1)) {
                        let r = row.rect().translate(output.galley_pos.to_vec2());
                        let band = egui::Rect::from_min_max(
                            egui::pos2(output.text_clip_rect.left(), r.top()),
                            egui::pos2(output.text_clip_rect.right(), r.bottom()),
                        );
                        ui.painter().rect_filled(band, 2.0, theme::alpha(theme::BAD, 0.1));
                    }
                    if output.response.response.changed() {
                        ed.dirty = ed.text != song.script;
                        // typing after a nudge: a line half written is not run
                        ed.nudged = None;
                    }
                    if !ed.no_hints {
                        let found = hints(&ed.text);
                        let mut lines: std::collections::BTreeMap<usize, Vec<&str>> = Default::default();
                        for (r, h) in &found {
                            lines.entry(ed.text[..r.start].matches('\n').count()).or_default().push(h);
                        }
                        let painter = ui.painter_at(output.text_clip_rect);
                        for (line, words) in &lines {
                            let Some(row) = output.galley.rows.get(*line) else { continue };
                            let r = row.rect().translate(output.galley_pos.to_vec2());
                            painter.text(
                                egui::pos2(r.right() + 18.0, r.center().y),
                                egui::Align2::LEFT_CENTER,
                                words.join(" · "),
                                theme::prop(11.0),
                                TEXT_FAINT,
                            );
                        }
                        // over a number: what it sets, and the keys that change it
                        if let Some(pos) = output.response.response.hover_pos() {
                            let c = output.galley.cursor_from_pos(pos - output.galley_pos);
                            let byte = ed.text.char_indices().nth(c.index.0).map_or(ed.text.len(), |(b, _)| b);
                            if let Some((_, h)) = found.iter().find(|(r, _)| r.start <= byte && byte < r.end) {
                                output.response.response.clone().on_hover_text_at_pointer(format!(
                                    "{h}. With the cursor on it, Alt+↑ ↓ change it (Shift: ten times as much), and \
                                     the song plays the change"
                                ));
                            }
                        }
                    }
                    if let Some(r) = output.cursor_range {
                        ed.cursor = Some(r.primary.index.0);
                    }
                    if output.response.response.has_focus()
                        && ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter))
                    {
                        run = true;
                    }
                });
            });
        });
    });
    // a number nudged runs the script once the keys have rested a moment, as a knob turned plays as it turns
    match ed.nudged {
        Some(t) if now - t >= NUDGE_REST && !ed.awaiting => {
            ed.nudged = None;
            run = true;
        }
        Some(_) => ui.ctx().request_repaint_after(std::time::Duration::from_millis(150)),
        None => {}
    }
    if run && !ed.awaiting {
        cx.out.push(ed.run());
    }
}

/// how long the keys rest after a number is nudged before the script runs, in seconds
const NUDGE_REST: f64 = 0.6;

/// The number at char `at` of the editor's text nudged `by` steps of its last digit (`nudged`), the cursor kept at its
/// end, and when, so that the script runs once the keys rest; whether there was one.
fn nudge(ed: &mut super::ScriptEditor, at: usize, by: i32, now: f64) -> bool {
    let byte = ed.text.char_indices().nth(at).map_or(ed.text.len(), |(b, _)| b);
    let Some(r) = number_at(&ed.text, byte) else { return false };
    let Some(new) = nudged(&ed.text[r.clone()], by) else { return false };
    let end = ed.text[..r.start].chars().count() + new.chars().count();
    ed.text.replace_range(r, &new);
    ed.move_cursor = Some(end);
    ed.nudged = Some(now);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rhai_error_locations() {
        assert_eq!(error_location("Function not found: tempoo (i64) (line 3, position 1)"), Some((3, 1)));
        assert_eq!(error_location("Syntax error: Expecting ')' (line 12, position 40)"), Some((12, 40)));
        assert_eq!(error_location("no song yet"), None);
        assert_eq!(char_index("ab\ncdef\ng", 2, 3), 5);
        assert_eq!(char_index("ab\ncdef\ng", 9, 1), 9);
    }

    #[test]
    fn the_number_at_the_cursor_moves_by_its_last_digit() {
        let src = "tempo(118); swing(0.56); // 99\ndrum(\"kick\", \"x.x3\");";
        let at = |s: &str| src.find(s).unwrap();
        assert_eq!(number_at(src, at("118") + 1).map(|r| &src[r]), Some("118"));
        assert_eq!(number_at(src, at("118") + 3).map(|r| &src[r]), Some("118"), "just after it, as after typing it");
        assert_eq!(number_at(src, at("118") + 4).map(|r| &src[r]), Some("118"), "past the bracket after it");
        assert_eq!(number_at(src, at("118") + 5), None);
        assert_eq!(number_at(src, at("99")), None, "none in a comment");
        assert_eq!(number_at(src, at("x.x3") + 3), None, "none in a string");
        assert_eq!(nudged("118", 1).as_deref(), Some("119"));
        assert_eq!(nudged("0.56", -10).as_deref(), Some("0.46"), "by its last digit, ten times with Shift");
        assert_eq!(nudged("-4", 10).as_deref(), Some("6"));
        assert_eq!(nudged("0.1", -1).as_deref(), Some("0.0"), "no -0.0");
        assert_eq!(nudged("0x1f", 1), None);
    }

    #[test]
    fn hints_say_what_each_number_sets() {
        let src = "tempo(118); swing(0.56);\nsec(\"drop\", 16, \"*\").energy(0.9).build(\"sweep\", 0.8),\n\
                   mix(\"pad\", #{ level: -4, pan: -0.3, reverb: 0.25 });\n\
                   track(\"x\", n(\"0 2\").lpf(4500).voicing(48, 72).level(2));";
        let found = hints(src);
        let words: Vec<&str> = found.iter().map(|(_, h)| h.as_str()).collect();
        assert_eq!(
            words,
            [
                "118 BPM",
                "swing 56%",
                "16 bars",
                "energy 90%",
                "builds 80% hard",
                "-4 dB",
                "30% left",
                "25%",
                "4.5 kHz",
                "from C3",
                "up to C5",
                "+2 dB"
            ]
        );
    }

    #[test]
    fn highlights_every_byte() {
        let src = "// hi\nsong(\"x\").bpm(124) // ø\nlet a = #{ k: 1.5 };";
        let job = highlight(src, FontId::monospace(12.0), Some(2));
        assert_eq!(job.text, src);
    }
}

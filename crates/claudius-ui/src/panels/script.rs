//! The script editor: the song as a claudius script (Rhai with Strudel-style patterns), highlighted (strings, numbers,
//! comments, functions and methods), with line numbers, Run (Cmd/Ctrl + Enter), the result of the last run (an error
//! marks its line), the sounds a script may use (click one to insert it at the cursor) and the language reference.

use std::sync::Arc;

use claudius_api::{Command, SoundList};
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
    let hit = |n: &str| f.is_empty() || n.to_lowercase().contains(&f);
    let filtering = !f.is_empty();
    let chips = |ui: &mut Ui, ed: &mut super::ScriptEditor, names: &[(String, String)], tip: &str| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
            for (label, insert_as) in names {
                if ui
                    .small_button(label.as_str())
                    .on_hover_text(format!("{tip}: inserts \"{insert_as}\" at the cursor"))
                    .clicked()
                {
                    insert(ed, &format!("\"{insert_as}\""));
                }
            }
        });
    };
    let drums: Vec<(String, String)> = list.drums.iter().filter(|n| hit(n)).map(|n| (n.clone(), n.clone())).collect();
    let synths: Vec<(String, String)> = list.synths.iter().filter(|n| hit(n)).map(|n| (n.clone(), n.clone())).collect();
    let mut packs: Vec<(&str, Vec<(String, String)>)> = Vec::new();
    for full in list.instruments.iter().filter(|n| n.contains(':') && hit(n)) {
        let (pack, name) = full.split_once(':').unwrap_or(("", full));
        match packs.iter_mut().find(|(p, _)| *p == pack) {
            Some((_, v)) => v.push((name.to_string(), full.clone())),
            None => packs.push((pack, vec![(name.to_string(), full.clone())])),
        }
    }
    let aliases: Vec<(String, String)> =
        list.instruments.iter().filter(|n| !n.contains(':') && hit(n)).map(|n| (n.clone(), n.clone())).collect();
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
    let ed = &mut cx.view.script;
    let reference_open = &mut cx.view.reference_open;
    let mut run = false;
    let error = ed.result.as_ref().filter(|(ok, _)| !ok).and_then(|(_, m)| error_location(m));
    page_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
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
            }
            if widgets::icon_button(ui, Icon::Copy, 28.0, false, true, "copy the script").clicked() {
                ui.ctx().copy_text(ed.text.clone());
            }
            if widgets::tool_button(ui, Icon::Open, "Reference", true, *reference_open)
                .on_hover_text("the script language reference (from the server)")
                .clicked()
            {
                *reference_open = !*reference_open;
            }
            if ed.dirty {
                widgets::pill(ui, "edited", ACCENT);
                if ed.base != song.script {
                    ui.label(RichText::new("the song changed since you started editing").color(theme::WARN).size(11.0));
                }
            } else {
                widgets::pill(ui, "in sync with the song", TEXT_DIM);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(small(format!("{} lines", ed.text.lines().count())));
            });
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
                            ui.label(small("the server's sound list appears here"));
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
    if run && !ed.awaiting {
        ed.awaiting = true;
        ed.result = None;
        cx.out.push(Command::RunScript { code: ed.text.clone() });
    }
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
    fn highlights_every_byte() {
        let src = "// hi\nsong(\"x\").bpm(124) // ø\nlet a = #{ k: 1.5 };";
        let job = highlight(src, FontId::monospace(12.0), Some(2));
        assert_eq!(job.text, src);
    }
}

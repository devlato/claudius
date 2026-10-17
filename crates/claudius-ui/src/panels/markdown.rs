//! A small markdown renderer for the script reference: headings, paragraphs, bullet lists, fenced code (highlighted
//! as script), tables (two-column ones as definition rows), inline `code`, **bold** and [links](url).

use eframe::egui::{self, Color32, FontId, RichText, Ui, text::LayoutJob, text::TextFormat};

use crate::theme::{self, ACCENT, TEXT, TEXT_DIM, WELL};

const CODE: Color32 = Color32::from_rgb(232, 196, 128);

/// inline markdown as a layout job
fn inline(text: &str, size: f32, color: Color32, width: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = width;
    let plain = TextFormat { font_id: theme::prop(size), color, ..Default::default() };
    let code = TextFormat {
        font_id: theme::mono(size - 1.0),
        color: CODE,
        background: theme::alpha(Color32::BLACK, 0.35),
        ..Default::default()
    };
    let bold = TextFormat { color: Color32::WHITE, ..plain.clone() };
    let link =
        TextFormat { color: ACCENT, underline: egui::Stroke::new(1.0, theme::alpha(ACCENT, 0.6)), ..plain.clone() };
    let text = text.replace("\\|", "|");
    let mut rest = text.as_str();
    while !rest.is_empty() {
        let next = ['`', '*', '['].iter().filter_map(|c| rest.find(*c).map(|i| (i, *c))).min();
        let Some((i, c)) = next else {
            job.append(rest, 0.0, plain.clone());
            break;
        };
        job.append(&rest[..i], 0.0, plain.clone());
        let after = &rest[i..];
        let consumed = match c {
            '`' => after[1..].find('`').map(|end| {
                job.append(&after[1..1 + end], 0.0, code.clone());
                end + 2
            }),
            '*' if after.starts_with("**") => after[2..].find("**").map(|end| {
                job.append(&after[2..2 + end], 0.0, bold.clone());
                end + 4
            }),
            '[' => after.find("](").and_then(|mid| {
                let close = after[mid..].find(')')? + mid;
                job.append(&after[1..mid], 0.0, link.clone());
                Some(close + 1)
            }),
            _ => None,
        };
        match consumed {
            Some(n) => rest = &after[n..],
            None => {
                job.append(&after[..1], 0.0, plain.clone());
                rest = &after[1..];
            }
        }
    }
    job
}

fn paragraph(ui: &mut Ui, text: &str) {
    let job = inline(text, 13.0, TEXT, ui.available_width());
    ui.label(job);
    ui.add_space(4.0);
}

fn cells(line: &str) -> Vec<String> {
    let inner = line.trim().trim_start_matches('|').trim_end_matches('|');
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                cur.push_str("\\|");
                chars.next();
            }
            '|' => out.push(std::mem::take(&mut cur).trim().to_string()),
            _ => cur.push(c),
        }
    }
    out.push(cur.trim().to_string());
    out
}

fn table(ui: &mut Ui, rows: &[&str]) {
    let rows: Vec<Vec<String>> = rows
        .iter()
        .map(|l| cells(l))
        .filter(|c| !c.iter().all(|x| !x.is_empty() && x.chars().all(|ch| matches!(ch, '-' | ':' | ' '))))
        .collect();
    let header = rows.first().filter(|h| h.iter().any(|c| !c.is_empty()));
    if let Some(h) = header {
        ui.label(RichText::new(h.join("  ·  ").to_uppercase()).size(10.5).color(TEXT_DIM).strong());
    }
    let body = if header.is_some() { &rows[1..] } else { rows.get(1..).unwrap_or(&[]) };
    egui::Frame::new()
        .fill(theme::alpha(WELL, 0.6))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            for (i, row) in body.iter().enumerate() {
                if i > 0 {
                    ui.add_space(5.0);
                }
                let width = ui.available_width();
                if let Some(first) = row.first().filter(|c| !c.is_empty()) {
                    ui.label(inline(first, 12.5, TEXT, width));
                }
                let rest: Vec<&str> = row.iter().skip(1).map(String::as_str).filter(|c| !c.is_empty()).collect();
                if !rest.is_empty() {
                    ui.horizontal_top(|ui| {
                        ui.add_space(14.0);
                        let width = ui.available_width();
                        ui.label(inline(&rest.join("  ·  "), 12.0, TEXT_DIM, width));
                    });
                }
            }
        });
    ui.add_space(6.0);
}

fn code_block(ui: &mut Ui, code: &str) {
    egui::Frame::new().fill(WELL).corner_radius(6).inner_margin(egui::Margin::symmetric(10, 8)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        egui::ScrollArea::horizontal().id_salt(("md_code", code.len(), code.lines().next().unwrap_or(""))).show(
            ui,
            |ui| {
                let job = super::script::highlight(code, FontId::monospace(12.0), None);
                ui.label(job);
            },
        );
    });
    ui.add_space(6.0);
}

pub fn show(ui: &mut Ui, text: &str) {
    let lines: Vec<&str> = text.lines().collect();
    let mut para: Vec<&str> = Vec::new();
    let mut i = 0;
    let flush = |ui: &mut Ui, para: &mut Vec<&str>| {
        if !para.is_empty() {
            paragraph(ui, &para.join(" "));
            para.clear();
        }
    };
    while i < lines.len() {
        let line = lines[i].trim_end();
        let t = line.trim_start();
        if t.starts_with("```") {
            flush(ui, &mut para);
            let start = i + 1;
            let mut end = start;
            while end < lines.len() && !lines[end].trim_start().starts_with("```") {
                end += 1;
            }
            code_block(ui, &lines[start..end].join("\n"));
            i = end + 1;
            continue;
        }
        if t.starts_with('|') {
            flush(ui, &mut para);
            let start = i;
            while i < lines.len() && lines[i].trim_start().starts_with('|') {
                i += 1;
            }
            table(ui, &lines[start..i]);
            continue;
        }
        if let Some((level, title)) = [("### ", 14.5), ("## ", 17.0), ("# ", 21.0)]
            .iter()
            .find_map(|(p, s)| t.strip_prefix(p).map(|rest| (*s, rest)))
        {
            flush(ui, &mut para);
            ui.add_space(if level > 15.0 { 10.0 } else { 6.0 });
            ui.label(RichText::new(title).size(level).strong().color(Color32::WHITE));
            ui.add_space(3.0);
        } else if let Some(item) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
            flush(ui, &mut para);
            ui.horizontal_top(|ui| {
                ui.label(RichText::new("•").color(ACCENT));
                let width = ui.available_width();
                ui.label(inline(item, 13.0, TEXT, width));
            });
        } else if t.is_empty() {
            flush(ui, &mut para);
        } else {
            para.push(t);
        }
        i += 1;
    }
    flush(ui, &mut para);
}

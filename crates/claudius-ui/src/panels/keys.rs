//! The keys the studio answers to: each action of the command palette's catalogue that has one
//! (`claudius_intent::catalogue`), done as the palette does it (`palette::act`), and the keys that move about the
//! studio, its arrangement and its windows; and the window that lists them all (`?`, or the help menu).

use claudius_intent::catalogue::{ENTRIES, Entry};
use eframe::egui::{self, Key, KeyboardShortcut, Modifiers, RichText};

use super::palette::{self, Asked};
use super::{Cx, small};
use crate::theme::{TEXT, TEXT_DIM, TEXT_FAINT};

/// the keys that move about the studio rather than do one of the palette's actions, and what each does
pub const MOVING: [(&str, &str); 7] = [
    ("Tab, Shift+Tab", "the next control, or the one before, as a screen reader reads them"),
    ("Space, Enter", "press the control that has the keyboard"),
    ("Arrows", "a knob or a fader that has the keyboard up or down; Shift, finer"),
    ("Left, Right", "the section before or after"),
    ("Up, Down", "the track above or below, in the arrangement"),
    ("Enter, Delete", "the selected track on or off in the section"),
    ("Escape", "let go of the control, close the window on top, or let go of the track"),
];

/// the window of the keys
#[derive(Default)]
pub struct Keys {
    pub open: bool,
}

/// A catalogue entry's keys as egui reads them: `Cmd+O`, `Shift+Cmd+Z`, `N`, `1`, `?`, `Space`, `Home`; none for an
/// entry without keys.
pub fn shortcut(keys: &str) -> Option<KeyboardShortcut> {
    let mut modifiers = Modifiers::NONE;
    let mut key = None;
    for part in keys.split('+').filter(|p| !p.is_empty()) {
        match part {
            "Cmd" => modifiers |= Modifiers::COMMAND,
            "Shift" => modifiers |= Modifiers::SHIFT,
            "Alt" => modifiers |= Modifiers::ALT,
            name => key = Some(Key::from_name(name)?),
        }
    }
    Some(KeyboardShortcut::new(modifiers, key?))
}

/// keys as this computer writes them: `Cmd` is Ctrl off a Mac
pub fn shown(keys: &str, mac: bool) -> String {
    if mac { keys.to_string() } else { keys.replace("Cmd", "Ctrl") }
}

/// the entries that have keys, those with more modifiers first: egui's matching lets Cmd+Z answer Shift+Cmd+Z too
fn keyed() -> Vec<(&'static Entry, KeyboardShortcut)> {
    let mut keyed: Vec<_> = ENTRIES.iter().filter_map(|e| Some((e, shortcut(e.keys)?))).collect();
    let held = |s: &KeyboardShortcut| {
        [s.modifiers.shift, s.modifiers.command, s.modifiers.alt].map(u8::from).iter().sum::<u8>()
    };
    keyed.sort_by_key(|(_, s)| std::cmp::Reverse(held(s)));
    keyed
}

/// The catalogue's action whose keys were pressed, done as the palette does it: none while a field is typed in, while
/// the palette, the New song or the Change the genre dialog is open, or for Space and Enter while a control has the
/// keyboard, which press it instead. What the app does itself for it (`Asked`), if anything.
pub fn answer(ctx: &egui::Context, cx: &mut Cx) -> Option<Asked> {
    if ctx.text_edit_focused() || cx.view.palette.open || cx.view.new_song.open || cx.view.change_genre.open {
        return None;
    }
    let focused = ctx.memory(|m| m.focused().is_some());
    for (e, s) in keyed() {
        let pressing = s.modifiers.is_none() && matches!(s.logical_key, Key::Space | Key::Enter);
        if focused && pressing {
            continue;
        }
        if ctx.input_mut(|i| i.consume_shortcut(&s)) {
            return palette::act(ctx, cx, palette::run_of(e.id)?);
        }
    }
    None
}

/// the window: the actions with keys, by where they live, and the keys that move about the studio
pub fn dialog(ctx: &egui::Context, cx: &mut Cx) {
    if !cx.view.keys.open {
        return;
    }
    let mac = ctx.os() == egui::os::OperatingSystem::Mac;
    let mut open = true;
    egui::Window::new("Keyboard shortcuts")
        .id(egui::Id::unique("keys"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(820.0)
        .pivot(egui::Align2::CENTER_CENTER)
        .default_pos(ctx.content_rect().center())
        .show(ctx, |ui| {
            let palette = shown("Cmd+K or Cmd+P", mac);
            let said = format!("Every action of the studio is in the command palette too ({palette}), with its key.");
            ui.label(small(said).color(TEXT_DIM));
            ui.add_space(6.0);
            let row = |ui: &mut egui::Ui, keys: &str, what: &str| {
                ui.label(RichText::new(shown(keys, mac)).monospace().strong().color(TEXT));
                ui.add(egui::Label::new(RichText::new(what).color(TEXT_DIM)).wrap());
                ui.end_row();
            };
            ui.columns(2, |cols| {
                cols[0].label(RichText::new("Actions").strong());
                egui::Grid::new("keys_actions").num_columns(2).spacing([18.0, 4.0]).striped(true).show(
                    &mut cols[0],
                    |ui| {
                        for e in ENTRIES.iter().filter(|e| !e.keys.is_empty()) {
                            row(ui, e.keys, e.name.trim_end_matches("..."));
                        }
                    },
                );
                cols[1].label(RichText::new("Moving about").strong());
                egui::Grid::new("keys_moving").num_columns(2).spacing([18.0, 4.0]).striped(true).show(
                    &mut cols[1],
                    |ui| {
                        for (keys, what) in MOVING {
                            row(ui, keys, what);
                        }
                    },
                );
                cols[1].add_space(8.0);
                cols[1].label(small("The keys work while no field is being typed in.").color(TEXT_FAINT));
            });
        });
    if !open {
        cx.view.keys.open = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_of_the_catalogue_is_one_egui_reads() {
        for e in ENTRIES.iter().filter(|e| !e.keys.is_empty()) {
            assert!(shortcut(e.keys).is_some(), "{}: '{}' is no key", e.id, e.keys);
        }
        assert_eq!(shortcut("Shift+Cmd+Z"), Some(KeyboardShortcut::new(Modifiers::SHIFT | Modifiers::COMMAND, Key::Z)));
        assert_eq!(shortcut("?"), Some(KeyboardShortcut::new(Modifiers::NONE, Key::Questionmark)));
        assert_eq!(shortcut(""), None);
    }

    #[test]
    fn a_key_with_more_held_is_tried_before_one_it_holds() {
        let order: Vec<&str> = keyed().iter().map(|(e, _)| e.id).collect();
        let at = |id: &str| order.iter().position(|x| *x == id).unwrap();
        assert!(at("redo") < at("undo"), "{order:?}");
    }

    #[test]
    fn off_a_mac_cmd_is_ctrl() {
        assert_eq!(shown("Shift+Cmd+Z", false), "Shift+Ctrl+Z");
        assert_eq!(shown("Shift+Cmd+Z", true), "Shift+Cmd+Z");
    }
}

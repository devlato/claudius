//! The command palette (Cmd+K, Ctrl+K elsewhere): the studio's actions by name, found by typing a few letters of them,
//! each with where it lives in the studio and the key that does it too, as editors' palettes are. The arrows pick one,
//! Enter or a click does it, Escape closes the palette.

use claudius_api::Command;
use eframe::egui::{self, Align2, Key, Modifiers, RichText, text::LayoutJob, vec2};

use super::{Cx, Tab};
use crate::theme::{self, TEXT, TEXT_FAINT};
use crate::widgets;

/// the palette while it is open: the words typed, and the action picked among those they find
#[derive(Default)]
pub struct Palette {
    pub open: bool,
    query: String,
    at: usize,
    /// the field is to take the keys, as it does when the palette opens
    focus: bool,
}

impl Palette {
    /// opened afresh, or closed
    pub fn toggle(&mut self) {
        *self = Palette { open: !self.open, focus: true, ..Palette::default() };
    }
}

/// what an action does
#[derive(Clone, Copy, Debug, PartialEq)]
enum Run {
    NewSong,
    Example,
    Open,
    Save,
    Play,
    ToStart,
    Stop,
    Loop,
    Undo,
    Redo,
    Render,
    Record,
    AddRecording,
    Phone,
    Paint,
    Tab(Tab),
    FlowLarge,
    Docs,
    Tour,
}

/// what the app does itself for an action the palette picked
pub enum Asked {
    Open,
    Tour,
}

/// An action: its name, the words it is also found by, where it lives in the studio, the key that does it too, and
/// what it does.
struct Act {
    name: &'static str,
    words: &'static str,
    place: &'static str,
    keys: &'static str,
    run: Run,
}

/// every action, in the order the palette lists them before anything is typed
const ACTS: &[Act] = &[
    Act { name: "Play or pause", words: "start listen hear", place: "transport", keys: "Space", run: Run::Play },
    Act {
        name: "Back to the start",
        words: "rewind beginning home",
        place: "transport",
        keys: "Home",
        run: Run::ToStart,
    },
    Act { name: "Stop", words: "halt rewind", place: "transport", keys: "", run: Run::Stop },
    Act { name: "Loop", words: "repeat cycle section", place: "transport", keys: "L", run: Run::Loop },
    Act { name: "Undo", words: "back revert", place: "toolbar", keys: "Cmd+Z", run: Run::Undo },
    Act { name: "Redo", words: "again forward", place: "toolbar", keys: "Shift+Cmd+Z", run: Run::Redo },
    Act {
        name: "New song...",
        words: "create genre generate make start",
        place: "toolbar",
        keys: "",
        run: Run::NewSong,
    },
    Act {
        name: "Open an example song...",
        words: "examples library demo pop rock house learn",
        place: "New song, From an example",
        keys: "",
        run: Run::Example,
    },
    Act { name: "Open a song...", words: "load file blueprint script", place: "toolbar", keys: "", run: Run::Open },
    Act {
        name: "Save or download",
        words: "download export file keep flac",
        place: "toolbar",
        keys: "",
        run: Run::Save,
    },
    Act { name: "Render", words: "listen judge verdict score bounce", place: "status bar", keys: "", run: Run::Render },
    Act {
        name: "Record a take...",
        words: "microphone mic sing voice instrument",
        place: "transport, the red button",
        keys: "",
        run: Run::Record,
    },
    Act {
        name: "Add a recording or sample one...",
        words: "import file sample vocals search free libraries freesound jamendo track chops passage stab",
        place: "transport, the + button",
        keys: "",
        run: Run::AddRecording,
    },
    Act {
        name: "Play from a phone...",
        words: "mobile pads keys turntable controller",
        place: "transport, the phone button",
        keys: "",
        run: Run::Phone,
    },
    Act {
        name: "Paint tool",
        words: "draw cells arrangement switch on off",
        place: "arrangement",
        keys: "B",
        run: Run::Paint,
    },
    Act {
        name: "Drums",
        words: "beat steps sequencer kick snare hat",
        place: "tab",
        keys: "",
        run: Run::Tab(Tab::Sequencer),
    },
    Act {
        name: "Instruments",
        words: "parts sounds synth bass lead pad layers",
        place: "tab",
        keys: "",
        run: Run::Tab(Tab::Parts),
    },
    Act { name: "Notes", words: "piano roll melody midi", place: "tab", keys: "", run: Run::Tab(Tab::Notes) },
    Act { name: "Chords", words: "harmony progression key", place: "tab", keys: "", run: Run::Tab(Tab::Harmony) },
    Act {
        name: "Mixer",
        words: "levels volume pan faders effects fx mix",
        place: "tab",
        keys: "",
        run: Run::Tab(Tab::Mixer),
    },
    Act { name: "Sounds", words: "samples swap kit browse", place: "tab", keys: "", run: Run::Tab(Tab::Sounds) },
    Act { name: "Script", words: "code rhai text edit", place: "tab", keys: "", run: Run::Tab(Tab::Script) },
    Act {
        name: "Signal flow, large",
        words: "graph whiteboard routing buses chain big",
        place: "arrangement, its Signal flow chip",
        keys: "G",
        run: Run::FlowLarge,
    },
    Act {
        name: "Signal flow",
        words: "graph whiteboard routing buses chain",
        place: "tab",
        keys: "",
        run: Run::Tab(Tab::Graph),
    },
    Act { name: "Log", words: "history messages what happened", place: "tab", keys: "", run: Run::Tab(Tab::Log) },
    Act { name: "Take the tour", words: "help guide learn start", place: "", keys: "", run: Run::Tour },
    Act { name: "Read the docs", words: "help manual guide tutorial", place: "on the web", keys: "", run: Run::Docs },
];

/// The actions `query` finds, the best first: those whose name starts with it, then those with a word starting with
/// each word typed (in their name, the words they are found by, or where they live), then those that hold it; all of
/// them, in their order, when nothing is typed.
fn found(query: &str) -> Vec<&'static Act> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return ACTS.iter().collect();
    }
    let mut scored: Vec<(u8, usize, &Act)> = ACTS
        .iter()
        .enumerate()
        .filter_map(|(i, a)| {
            let name = a.name.to_lowercase();
            let all = format!("{name} {} {}", a.words, a.place.to_lowercase());
            let words: Vec<&str> = all.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
            let score = if name.starts_with(&q) {
                0
            } else if q.split_whitespace().all(|t| words.iter().any(|w| w.starts_with(t))) {
                1
            } else if all.contains(&q) {
                2
            } else {
                return None;
            };
            Some((score, i, a))
        })
        .collect();
    scored.sort_by_key(|(score, i, _)| (*score, *i));
    scored.into_iter().map(|(_, _, a)| a).collect()
}

/// The palette, while it is open: a field to type in and the actions it finds. What the app does itself comes back.
pub fn show(ctx: &egui::Context, cx: &mut Cx) -> Option<Asked> {
    if !cx.view.palette.open {
        return None;
    }
    let p = &mut cx.view.palette;
    let (down, up, enter, escape) = ctx.input_mut(|i| {
        (
            i.consume_key(Modifiers::NONE, Key::ArrowDown),
            i.consume_key(Modifiers::NONE, Key::ArrowUp),
            i.consume_key(Modifiers::NONE, Key::Enter),
            i.consume_key(Modifiers::NONE, Key::Escape),
        )
    });
    if escape {
        p.open = false;
        return None;
    }
    let acts = found(&p.query);
    let last = acts.len().saturating_sub(1);
    p.at = if down {
        (p.at + 1).min(last)
    } else if up {
        p.at.saturating_sub(1)
    } else {
        p.at.min(last)
    };
    let mut picked = enter.then(|| acts.get(p.at).map(|a| a.run)).flatten();
    let cmd = widgets::cmd_key(ctx);
    egui::Window::new("command palette")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(Align2::CENTER_TOP, vec2(0.0, 72.0))
        .show(ctx, |ui| {
            ui.set_width(520.0);
            let field = ui.add(
                egui::TextEdit::singleline(&mut p.query)
                    .hint_text("Type what you want to do: new song, mixer, loop, sample ...")
                    .desired_width(f32::INFINITY)
                    .font(egui::TextStyle::Heading),
            );
            if std::mem::take(&mut p.focus) {
                field.request_focus();
            }
            if field.changed() {
                p.at = 0;
            }
            ui.add_space(4.0);
            if acts.is_empty() {
                ui.label(RichText::new("Nothing by that name. Try other words, or read the docs.").color(TEXT_FAINT));
            }
            egui::ScrollArea::vertical().max_height(360.0).auto_shrink([false, true]).show(ui, |ui| {
                for (i, a) in acts.iter().enumerate() {
                    let mut job = LayoutJob::default();
                    job.append(a.name, 0.0, egui::TextFormat::simple(theme::prop(13.5), TEXT));
                    if !a.place.is_empty() {
                        job.append(a.place, 10.0, egui::TextFormat::simple(theme::prop(11.0), TEXT_FAINT));
                    }
                    let keys = a.keys.replace("Cmd", cmd);
                    let r = ui.add_sized(
                        vec2(ui.available_width(), 26.0),
                        egui::Button::new(job)
                            .selected(i == p.at)
                            .shortcut_text(RichText::new(keys).size(11.0).color(TEXT_FAINT)),
                    );
                    if i == p.at && (down || up) {
                        r.scroll_to_me(None);
                    }
                    if r.clicked() {
                        picked = Some(a.run);
                    }
                }
            });
            ui.add_space(2.0);
            ui.label(
                RichText::new(format!("{cmd}+K opens this from anywhere · arrows pick · Enter does it · Esc closes"))
                    .size(10.5)
                    .color(TEXT_FAINT),
            );
        });
    let run = picked?;
    cx.view.palette.open = false;
    match run {
        Run::NewSong | Run::Example => {
            cx.view.new_song.open = true;
            if let Some(song) = &cx.state.song {
                cx.view.new_song.set_genre(&song.genre);
            }
            if run == Run::Example {
                cx.view.new_song.start = super::Start::Example;
            }
        }
        Run::Open => return Some(Asked::Open),
        Run::Save => super::transport::ask_download(cx.state, cx.view, cx.out, cx.server.is_none(), cx.now, None),
        Run::Play if cx.player.is_playing() => cx.player.pause(),
        Run::Play => cx.player.play(),
        Run::ToStart => cx.player.seek(0.0),
        Run::Stop => cx.player.stop(),
        Run::Loop => cx.view.looping = !cx.view.looping,
        Run::Undo => cx.out.push(Command::Undo),
        Run::Redo => cx.out.push(Command::Redo),
        Run::Render => cx.out.push(Command::Render { stems: false }),
        Run::Record => cx.view.record.show(),
        Run::AddRecording => cx.view.import.open = true,
        Run::Phone => cx.view.phone.show(cx.state.phone.is_some()),
        Run::Paint => cx.view.paint = !cx.view.paint,
        Run::Tab(t) => cx.view.tab = t,
        Run::FlowLarge => cx.view.flow_large = cx.state.song.is_some() && !cx.view.flow_large,
        Run::Docs => ctx.open_url(egui::OpenUrl::new_tab(widgets::DOCS)),
        Run::Tour => return Some(Asked::Tour),
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_finds_actions_by_their_name_then_by_their_words() {
        let names = |q: &str| found(q).iter().map(|a| a.name).collect::<Vec<_>>();
        assert_eq!(names("").len(), ACTS.len(), "everything before anything is typed");
        assert_eq!(names("mix")[0], "Mixer");
        assert_eq!(names("loop")[0], "Loop");
        assert_eq!(names("volume")[0], "Mixer", "found by a word it goes by");
        assert_eq!(names("whiteboard")[0], "Signal flow, large");
        assert_eq!(names("sample vocals")[0], "Add a recording or sample one...");
        assert_eq!(names("free lib")[0], "Add a recording or sample one...");
        assert!(names("zzzz").is_empty());
    }

    #[test]
    fn every_tab_and_every_kind_of_action_is_in_the_palette() {
        for t in Tab::ALL {
            assert!(ACTS.iter().any(|a| a.run == Run::Tab(t) && a.name == t.label()), "{}", t.label());
        }
        let names: std::collections::HashSet<&str> = ACTS.iter().map(|a| a.name).collect();
        assert_eq!(names.len(), ACTS.len(), "no two actions by one name");
    }
}

//! The command palette (Cmd+K, Ctrl+K elsewhere): the studio's actions (`claudius_intent::catalogue`), found from a few
//! words, each with where it lives in the studio and the key that does it too, as editors' palettes are. The words find
//! actions by name at once, here; the engine finds them by meaning too, as the person would say it ("make it louder",
//! "sing over it"), once its embedding model is there, and its answer replaces what the name found. The arrows pick one,
//! Enter or a click does it, Escape closes the palette.

use claudius_api::Command;
use claudius_intent::catalogue::ENTRIES;
use claudius_intent::{Answer, By, Finder, Found};
use eframe::egui::{self, Align2, Key, Modifiers, RichText, text::LayoutJob, vec2};

use super::{Cx, Tab};
use crate::theme::{self, TEXT, TEXT_FAINT};
use crate::widgets;

/// how many actions the palette shows at most
pub const SHOWN: usize = 12;

/// the palette while it is open: the words typed, and the action picked among those they find
#[derive(Default)]
pub struct Palette {
    pub open: bool,
    query: String,
    at: usize,
    /// the field is to take the keys, as it does when the palette opens
    focus: bool,
    /// where the status bar's link to it is, which opens and closes it itself: a press there is not one outside it
    pub link: Option<egui::Rect>,
    /// the words to ask the engine about, which the app sends it (`Backend::find`)
    pub ask: Option<String>,
    /// what the engine found for the words asked about last
    answer: Option<Answer>,
    /// when the words last changed, and when the engine was last asked about them (`Cx::now`)
    typed_at: f64,
    asked_at: f64,
    /// the action highlighted as the palette was last drawn, which Enter does, and whether the arrows picked it
    lit: Option<String>,
    moved: bool,
}

/// how often the palette asks again about words an engine found by name only, as its model may have come since, and
/// for how long after they were typed (seconds)
const AGAIN: (f64, f64) = (1.0, 60.0);

impl Palette {
    /// opened afresh, or closed
    pub fn toggle(&mut self) {
        *self = Palette { open: !self.open, focus: true, link: self.link, ..Palette::default() };
    }

    /// The words again, to the engine, while what it found for them it found by name only: a server answers so until
    /// its model has downloaded, and the demo once it has fetched its own.
    fn ask_again(&mut self, now: f64) -> bool {
        let words = self.query.trim();
        let by_name = self.answer.as_ref().is_some_and(|a| a.query == words && !a.by_meaning);
        let waiting = by_name && !words.is_empty() && now - self.typed_at < AGAIN.1;
        if waiting && now - self.asked_at >= AGAIN.0 {
            self.ask = Some(words.to_string());
            self.asked_at = now;
        }
        waiting
    }

    /// Which of `acts` is highlighted after the arrows: the one picked with them stays picked as the list is ranked
    /// anew (an answer by meaning replacing one by name), and without them the best is.
    fn follow(&mut self, acts: &[Found], down: bool, up: bool) {
        let held = self.lit.as_ref().and_then(|id| acts.iter().position(|a| &a.id == id));
        let last = acts.len().saturating_sub(1);
        self.at = match (down, up) {
            (true, _) => held.map_or(0, |i| (i + 1).min(last)),
            (_, true) => held.map_or(0, |i| i.saturating_sub(1)),
            _ if self.moved => held.unwrap_or(0),
            _ => 0,
        };
        self.moved |= down || up;
    }

    /// what the engine found for some words: kept while they are what is typed
    pub fn heard(&mut self, answer: Answer) {
        if answer.query == self.query.trim() {
            self.answer = Some(answer);
        }
    }

    /// the actions shown for what is typed: what the engine found for it, by name and by meaning, or until it answers,
    /// what the words find by name here
    fn shown(&self) -> Vec<Found> {
        match &self.answer {
            Some(a) if a.query == self.query.trim() => a.found.clone(),
            _ => Finder::new(ENTRIES, None).find(&self.query, None, SHOWN),
        }
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
    Variations,
    Improve,
    FixAll,
    Cancel,
    Stems,
    Packs,
    Publish,
    Assistants,
    Reference,
    Docs,
    Tour,
}

/// the action of the catalogue's id
fn run_of(id: &str) -> Option<Run> {
    Some(match id {
        "play" => Run::Play,
        "to_start" => Run::ToStart,
        "stop" => Run::Stop,
        "loop" => Run::Loop,
        "undo" => Run::Undo,
        "redo" => Run::Redo,
        "new_song" => Run::NewSong,
        "example" => Run::Example,
        "open" => Run::Open,
        "save" => Run::Save,
        "render" => Run::Render,
        "record" => Run::Record,
        "add_recording" => Run::AddRecording,
        "phone" => Run::Phone,
        "paint" => Run::Paint,
        "flow_large" => Run::FlowLarge,
        "variations" => Run::Variations,
        "improve" => Run::Improve,
        "fix_all" => Run::FixAll,
        "cancel" => Run::Cancel,
        "stems" => Run::Stems,
        "packs" => Run::Packs,
        "publish" => Run::Publish,
        "assistants" => Run::Assistants,
        "reference" => Run::Reference,
        "docs" => Run::Docs,
        "tour" => Run::Tour,
        other => Run::Tab(match other.strip_prefix("tab:")? {
            "drums" => Tab::Sequencer,
            "instruments" => Tab::Parts,
            "notes" => Tab::Notes,
            "chords" => Tab::Harmony,
            "mixer" => Tab::Mixer,
            "sounds" => Tab::Sounds,
            "script" => Tab::Script,
            "flow" => Tab::Graph,
            "log" => Tab::Log,
            _ => return None,
        }),
    })
}

/// what the app does itself for an action the palette picked
pub enum Asked {
    Open,
    Tour,
}

/// The palette, while it is open: a field to type in and the actions it finds. What the app does itself comes back.
pub fn show(ctx: &egui::Context, cx: &mut Cx) -> Option<Asked> {
    if !cx.view.palette.open {
        return None;
    }
    let p = &mut cx.view.palette;
    // the keys are the palette's, but for a field elsewhere being typed in
    let field_id = egui::Id::unique("command palette field");
    let ours = !ctx.text_edit_focused() || ctx.memory(|m| m.has_focus(field_id));
    let (down, up, enter, escape) = ctx.input_mut(|i| {
        (
            ours && i.consume_key(Modifiers::NONE, Key::ArrowDown),
            ours && i.consume_key(Modifiers::NONE, Key::ArrowUp),
            ours && i.consume_key(Modifiers::NONE, Key::Enter),
            ours && i.consume_key(Modifiers::NONE, Key::Escape),
        )
    });
    if escape {
        p.open = false;
        return None;
    }
    let now = cx.now;
    let acts = p.shown();
    // Enter does what was highlighted as the palette was drawn, whatever the list has been ranked as since
    let lit = p.lit.clone().or_else(|| acts.get(p.at).map(|a| a.id.clone()));
    let mut picked = enter.then(|| lit.as_deref().and_then(run_of)).flatten();
    p.follow(&acts, down, up);
    let cmd = widgets::cmd_key(ctx);
    let shown = egui::Window::new("command palette")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(Align2::CENTER_TOP, vec2(0.0, 72.0))
        .show(ctx, |ui| {
            ui.set_width(520.0);
            let field = ui.add(
                egui::TextEdit::singleline(&mut p.query)
                    .id(field_id)
                    .hint_text("Type what you want to do: new song, make it louder, sing over it ...")
                    .desired_width(f32::INFINITY)
                    .font(egui::TextStyle::Heading),
            );
            if std::mem::take(&mut p.focus) {
                field.request_focus();
            }
            if field.changed() {
                (p.at, p.moved, p.typed_at, p.asked_at) = (0, false, now, now);
                let words = p.query.trim();
                p.ask = (!words.is_empty()).then(|| words.to_string());
            }
            ui.add_space(4.0);
            if acts.is_empty() {
                ui.label(RichText::new("Nothing by that name. Try other words, or read the docs.").color(TEXT_FAINT));
            }
            egui::ScrollArea::vertical().max_height(360.0).auto_shrink([false, true]).show(ui, |ui| {
                for (i, a) in acts.iter().enumerate() {
                    let mut job = LayoutJob::default();
                    job.append(&a.name, 0.0, egui::TextFormat::simple(theme::prop(13.5), TEXT));
                    if !a.place.is_empty() {
                        job.append(&a.place, 10.0, egui::TextFormat::simple(theme::prop(11.0), TEXT_FAINT));
                    }
                    let keys = a.keys.replace("Cmd", cmd);
                    let r = ui.add_sized(
                        vec2(ui.available_width(), 26.0),
                        egui::Button::new(job)
                            .selected(i == p.at)
                            .shortcut_text(RichText::new(keys).size(11.0).color(TEXT_FAINT)),
                    );
                    let r = if a.by == By::Meaning { r.on_hover_text("found by what your words mean") } else { r };
                    if i == p.at && (down || up) {
                        r.scroll_to_me(None);
                    }
                    if r.clicked() {
                        picked = run_of(&a.id);
                    }
                }
            });
            ui.add_space(2.0);
            let by = if p.answer.as_ref().is_some_and(|a| a.by_meaning) {
                "finds what you mean, on this computer"
            } else {
                "finds actions by name"
            };
            ui.label(
                RichText::new(format!(
                    "{cmd}+K opens this from anywhere · {by} · arrows pick · Enter does it · Esc closes"
                ))
                .size(10.5)
                .color(TEXT_FAINT),
            );
        });
    p.lit = acts.get(p.at).map(|a| a.id.clone());
    if p.ask_again(now) {
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(AGAIN.0));
    }
    // a click outside it closes it, as a menu closes
    let link = cx.view.palette.link;
    let outside = |at: egui::Pos2| {
        shown.as_ref().is_some_and(|s| !s.response.rect.contains(at)) && !link.is_some_and(|l| l.contains(at))
    };
    if ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(outside)) {
        cx.view.palette.open = false;
    }
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
        // as Space does: a take recording ends, and the song plays only once it has its audio
        Run::Play if cx.view.record.recording() => cx.view.record.stop = true,
        Run::Play if !cx.player.has_audio() => {}
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
        // the dock is not drawn while the signal flow is large: a tab picked shows it again
        Run::Tab(t) => {
            cx.view.tab = t;
            super::graph::open_large(cx, false);
        }
        Run::FlowLarge => {
            let large = cx.state.song.is_some() && !cx.view.flow_large;
            super::graph::open_large(cx, large);
        }
        // as the status bar's buttons do
        Run::Variations => cx.out.push(Command::Mutate { count: cx.view.mutate_count, keep_best: cx.view.keep_best }),
        Run::Improve => cx.out.push(Command::Evolve { generations: cx.view.generations, children: cx.view.children }),
        Run::FixAll => cx.out.push(Command::QuickFix { target: None }),
        Run::Cancel => cx.out.push(Command::Cancel),
        Run::Stems => cx.out.push(Command::Render { stems: true }),
        Run::Packs => cx.out.push(Command::InstallPacks { names: Vec::new() }),
        Run::Publish => cx.view.publish.show(),
        Run::Assistants => cx.view.assistants.show(),
        Run::Reference => cx.view.reference_open = true,
        Run::Docs => ctx.open_url(egui::OpenUrl::new_tab(widgets::DOCS)),
        Run::Tour => return Some(Asked::Tour),
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_action_of_the_catalogue_is_one_the_palette_does_and_every_tab_is_there() {
        for e in ENTRIES {
            assert!(run_of(e.id).is_some(), "{}: the palette does nothing for it", e.id);
        }
        for t in Tab::ALL {
            assert!(ENTRIES.iter().any(|e| run_of(e.id) == Some(Run::Tab(t)) && e.name == t.label()), "{}", t.label());
        }
    }

    #[test]
    fn what_is_typed_is_found_by_name_until_the_engine_answers_for_it() {
        let mut p = Palette { query: "mix".into(), ..Palette::default() };
        assert_eq!(p.shown()[0].name, "Mixer");
        let found = |name: &str| Found {
            id: "tab:sounds".into(),
            name: name.into(),
            place: "tab".into(),
            keys: String::new(),
            score: 0.8,
            by: By::Meaning,
        };
        p.heard(Answer { query: "mi".into(), found: vec![found("an old answer")], by_meaning: true });
        assert_eq!(p.shown()[0].name, "Mixer", "an answer for other words is not shown");
        p.heard(Answer { query: "mix".into(), found: vec![found("Sounds")], by_meaning: true });
        assert_eq!(p.shown()[0].name, "Sounds", "the engine's answer, by meaning too");
        p.query = "mixer".into();
        assert_eq!(p.shown()[0].name, "Mixer", "words typed since are found by name until it answers for them");
    }

    fn act(id: &str) -> Found {
        Found { id: id.into(), name: id.into(), place: String::new(), keys: String::new(), score: 0.5, by: By::Name }
    }

    #[test]
    fn an_action_picked_with_the_arrows_stays_picked_as_the_list_is_ranked_anew_and_else_the_best_is() {
        let mut p = Palette::default();
        let by_name = [act("new_song"), act("tab:mixer"), act("improve")];
        let by_meaning = [act("tab:mixer"), act("improve"), act("new_song")];
        p.follow(&by_name, false, false);
        p.lit = Some(by_name[p.at].id.clone());
        p.follow(&by_meaning, false, false);
        assert_eq!(by_meaning[p.at].id, "tab:mixer", "nothing picked: the best of the new ranking");
        p.lit = Some(by_meaning[p.at].id.clone());
        p.follow(&by_meaning, true, false);
        assert_eq!(by_meaning[p.at].id, "improve");
        p.lit = Some(by_meaning[p.at].id.clone());
        p.follow(&by_name, false, false);
        assert_eq!(by_name[p.at].id, "improve", "picked with the arrows: it stays picked where the list moved it");
    }

    #[test]
    fn words_found_by_name_only_are_asked_about_again_a_second_apart_for_a_minute() {
        let mut p = Palette { query: "make it louder".into(), ..Palette::default() };
        let answer = |by_meaning| Answer { query: "make it louder".into(), found: vec![act("tab:mixer")], by_meaning };
        p.heard(answer(false));
        assert!(p.ask_again(0.5) && p.ask.is_none(), "not within a second of asking");
        assert!(p.ask_again(1.0) && p.ask.take().as_deref() == Some("make it louder"));
        assert!(p.ask_again(1.5) && p.ask.is_none());
        assert!(!p.ask_again(61.0) && p.ask.is_none(), "a minute after the words were typed, no more");
        p.heard(answer(true));
        assert!(!p.ask_again(2.0) && p.ask.is_none(), "found by meaning: nothing more to ask");
    }
}

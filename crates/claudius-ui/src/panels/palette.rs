//! The command palette (Cmd+K, Ctrl+K elsewhere): the studio's actions (`claudius_intent::catalogue`), found from a few
//! words, each with where it lives in the studio and the key that does it too, as editors' palettes are. The words find
//! actions by name at once, here; the engine finds them by meaning too, as the person would say it ("make it louder",
//! "sing over it"), once its embedding model is there, and its answer replaces what the name found. A request of the
//! song in plain words ("more reverb on the snare", "undo the last 3 changes") is read too (`claudius_intent::ask`),
//! and what claudius would do for it comes first, a button for each reading, which says what it changes from what to
//! what: nothing is done until one is picked. The arrows pick one, Enter or a click does it, Escape closes the palette.

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
    /// the latest change made to the song when the palette last looked (the state's `history`)
    seen: Option<String>,
}

/// how often the palette asks again about words an engine found by name only, as its model may have come since, and
/// for how long after they were typed (seconds)
const AGAIN: (f64, f64) = (1.0, 60.0);

/// how often it asks again while the small model on the device reads the words, and for how long (seconds): the first
/// words it reads wait for it to download
const READING: (f64, f64) = (0.3, 60.0);

/// what a row's id starts with when it goes back to before a change made to the song (`Answer::done`), the change's
/// id after it
const BACK: &str = "back:";

/// what a row's id starts with when it does what a request asked (`Answer::proposed`), what it says it does after it
const DO: &str = "do:";

/// `text`, cut to `most` characters with an ellipsis
fn brief(text: &str, most: usize) -> String {
    if text.chars().count() <= most {
        return text.to_string();
    }
    format!("{}…", text.chars().take(most - 1).collect::<String>().trim_end())
}

impl Palette {
    /// opened afresh, or closed
    pub fn toggle(&mut self) {
        *self = Palette { open: !self.open, focus: true, link: self.link, ..Palette::default() };
    }

    /// The words again, to the engine, while what it found for them it found by name only: a server answers so until
    /// its model has downloaded, and the demo once it has fetched its own.
    /// While the small model reads words the readings read nothing of, they are asked about again more often. The
    /// interval it waits for, while it does.
    fn ask_again(&mut self, now: f64) -> Option<f64> {
        let words = self.query.trim();
        let (every, waiting) = match self.answer.as_ref().filter(|a| a.query == words) {
            Some(a) if a.reading => (READING.0, now - self.typed_at < READING.1),
            Some(a) if !a.by_meaning => (AGAIN.0, now - self.typed_at < AGAIN.1),
            _ => return None,
        };
        if !waiting || words.is_empty() {
            return None;
        }
        if now - self.asked_at >= every {
            self.ask = Some(words.to_string());
            self.asked_at = now;
        }
        Some(every)
    }

    /// the small model is reading what is typed, and the palette still waits for what it reads
    fn reading(&self, now: f64) -> bool {
        self.answered().is_some_and(|a| a.reading) && now - self.typed_at < READING.1
    }

    /// The latest change made to the song (the state's `history`): one made since the palette last looked (a command
    /// the engine was busy for, a job's result) has the words asked about again, as the changes they find have changed.
    fn follow_history(&mut self, latest: Option<&str>) {
        if self.seen.as_deref() != latest {
            self.seen = latest.map(str::to_string);
            let words = self.query.trim();
            if !words.is_empty() {
                self.ask = Some(words.to_string());
            }
        }
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

    /// the engine's answer, while it is for what is typed
    fn answered(&self) -> Option<&Answer> {
        self.answer.as_ref().filter(|a| a.query == self.query.trim())
    }

    /// the rows shown for what is typed: what the engine read it to ask of the song, then the actions it found, by
    /// name and by meaning, then the changes made to the song it found, to go back to before (but those a reading goes
    /// back to before already); or until it answers, what the words find by name here
    fn shown(&self) -> Vec<Found> {
        let Some(a) = self.answered() else { return Finder::new(ENTRIES, None).find(&self.query, None, SHOWN) };
        let asked = a.proposed.iter().map(|p| Found {
            id: format!("{DO}{}", p.said),
            name: p.said.clone(),
            place: brief(&p.detail, 72),
            keys: String::new(),
            score: p.score,
            by: By::Meaning,
        });
        let proposed_back = |id: &str| {
            a.proposed.iter().any(|p| p.commands.iter().any(|c| matches!(c, Command::GoBack { to } if to == id)))
        };
        let back = a.done.iter().filter(|d| !proposed_back(&d.id)).map(|d| Found {
            id: format!("{BACK}{}", d.id),
            name: format!("Go back to before {}", d.name),
            place: brief(&d.place, 72),
            keys: String::new(),
            score: d.score,
            by: d.by,
        });
        asked.chain(a.found.iter().cloned()).chain(back).collect()
    }

    /// what the row `id` does: a reading's commands, while it is the engine's for what is typed, or an action's
    fn run(&self, id: &str) -> Option<Run> {
        let Some(said) = id.strip_prefix(DO) else { return run_of(id) };
        let p = self.answered()?.proposed.iter().find(|p| p.said == said)?;
        Some(Run::Do { said: p.said.clone(), commands: p.commands.clone() })
    }
}

/// what an action does
#[derive(Clone, Debug, PartialEq)]
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
    /// back to before a change made to the song: its id in the history
    GoBack(String),
    /// what a request asked, as the person confirmed it: what the button said, and the commands
    Do {
        said: String,
        commands: Vec<Command>,
    },
}

/// The commands that do what a request asked: its edits as one change, which undo takes back at once and calls what
/// the button said; undo, redo and going back as they are, each a step.
fn commands_of(said: String, commands: Vec<Command>) -> Vec<Command> {
    if !commands.is_empty() && commands.iter().all(Command::is_edit) {
        vec![Command::Together { said, commands }]
    } else {
        commands
    }
}

/// the action of the catalogue's id, or of a row going back to before a change (`BACK`)
fn run_of(id: &str) -> Option<Run> {
    if let Some(change) = id.strip_prefix(BACK) {
        return Some(Run::GoBack(change.to_string()));
    }
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
    p.follow_history(cx.state.history.first().map(|d| d.id.as_str()));
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
    let mut picked = enter.then(|| lit.as_deref().and_then(|id| p.run(id))).flatten();
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
                    .hint_text("Type what to do: new song, more reverb on the snare, undo the last 3 changes ...")
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
            if p.reading(now) {
                ui.label(RichText::new("Reading your words…").size(11.0).color(TEXT_FAINT));
            } else if acts.is_empty() {
                ui.label(RichText::new("Nothing by that name. Try other words, or read the docs.").color(TEXT_FAINT));
            }
            egui::ScrollArea::vertical().max_height(360.0).auto_shrink([false, true]).show(ui, |ui| {
                for (i, a) in acts.iter().enumerate() {
                    // what the words asked comes first, then the actions, then the changes made to the song, each of
                    // those with what it changes or changed under it
                    let (asked, back) = (a.id.starts_with(DO), a.id.starts_with(BACK));
                    if asked && i == 0 {
                        ui.label(RichText::new("What your words ask").size(10.5).color(TEXT_FAINT));
                    }
                    if !asked && !back && i > 0 && acts[i - 1].id.starts_with(DO) {
                        ui.add_space(4.0);
                        ui.label(RichText::new("Actions").size(10.5).color(TEXT_FAINT));
                    }
                    if back && !acts[..i].iter().any(|b| b.id.starts_with(BACK)) {
                        ui.add_space(4.0);
                        ui.label(RichText::new("In the song's history").size(10.5).color(TEXT_FAINT));
                    }
                    let under = (asked || back) && !a.place.is_empty();
                    let mut job = LayoutJob::default();
                    job.append(&a.name, 0.0, egui::TextFormat::simple(theme::prop(13.5), TEXT));
                    if under {
                        job.append(
                            &format!("\n{}", a.place),
                            0.0,
                            egui::TextFormat::simple(theme::prop(11.0), TEXT_FAINT),
                        );
                    } else if !a.place.is_empty() {
                        job.append(&a.place, 10.0, egui::TextFormat::simple(theme::prop(11.0), TEXT_FAINT));
                    }
                    let keys = a.keys.replace("Cmd", cmd);
                    let r = ui.add_sized(
                        vec2(ui.available_width(), if under { 40.0 } else { 26.0 }),
                        egui::Button::new(job)
                            .selected(i == p.at)
                            .shortcut_text(RichText::new(keys).size(11.0).color(TEXT_FAINT)),
                    );
                    let by_model = asked
                        && p.answered().is_some_and(|ans| {
                            ans.proposed.iter().any(|x| x.reading.by_model && a.id.strip_prefix(DO) == Some(x.said.as_str()))
                        });
                    let r = match (asked, a.by) {
                        (true, _) if by_model => r.on_hover_text(
                            "what the small model on this computer read your words to ask: nothing changes until you pick it",
                        ),
                        (true, _) => {
                            r.on_hover_text("what claudius read your words to ask: nothing changes until you pick it")
                        }
                        (_, By::Meaning) => r.on_hover_text("found by what your words mean"),
                        _ => r,
                    };
                    if i == p.at && (down || up) {
                        r.scroll_to_me(None);
                    }
                    if r.clicked() {
                        picked = p.run(&a.id);
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
    if let Some(every) = p.ask_again(now) {
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(every));
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
    let example = run == Run::Example;
    match run {
        Run::NewSong | Run::Example => {
            cx.view.new_song.open = true;
            if let Some(song) = &cx.state.song {
                cx.view.new_song.set_genre(&song.genre);
            }
            if example {
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
        Run::GoBack(to) => cx.out.push(Command::GoBack { to }),
        Run::Do { said, commands } => cx.out.extend(commands_of(said, commands)),
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
        p.heard(Answer {
            query: "mi".into(),
            found: vec![found("an old answer")],
            by_meaning: true,
            ..Answer::default()
        });
        assert_eq!(p.shown()[0].name, "Mixer", "an answer for other words is not shown");
        p.heard(Answer { query: "mix".into(), found: vec![found("Sounds")], by_meaning: true, ..Answer::default() });
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
    fn a_change_the_words_found_is_a_row_after_the_actions_that_goes_back_to_before_it() {
        let mut p = Palette { query: "before the whistles".into(), ..Palette::default() };
        let change = Found {
            id: "7@4f2a".into(),
            name: "track whistle".into(),
            place: "added the whistle track (lead), playing the synth whistle, and a long while more of it".into(),
            keys: String::new(),
            score: 0.7,
            by: By::Meaning,
        };
        p.heard(Answer {
            query: p.query.clone(),
            found: vec![act("undo")],
            done: vec![change],
            by_meaning: true,
            ..Answer::default()
        });
        let rows = p.shown();
        assert_eq!(
            rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
            ["undo", "Go back to before track whistle"]
        );
        assert!(rows[1].place.ends_with('…') && rows[1].place.chars().count() == 72, "{}", rows[1].place);
        assert_eq!(run_of(&rows[1].id), Some(Run::GoBack("7@4f2a".into())));
    }

    #[test]
    fn a_change_made_while_the_palette_is_open_has_its_words_asked_about_again() {
        let mut p = Palette::default();
        p.follow_history(Some("1@a"));
        assert!(p.ask.is_none(), "nothing typed, nothing to ask");
        p.query = "faster".into();
        p.follow_history(Some("1@a"));
        assert!(p.ask.is_none(), "no change since it last looked");
        p.follow_history(Some("2@a"));
        assert_eq!(p.ask.take().as_deref(), Some("faster"), "a change the engine was busy for, made since");
        p.follow_history(Some("2@a"));
        assert!(p.ask.is_none());
    }

    #[test]
    fn words_found_by_name_only_are_asked_about_again_a_second_apart_for_a_minute() {
        let mut p = Palette { query: "make it louder".into(), ..Palette::default() };
        let answer = |by_meaning| Answer {
            query: "make it louder".into(),
            found: vec![act("tab:mixer")],
            by_meaning,
            ..Answer::default()
        };
        p.heard(answer(false));
        assert!(p.ask_again(0.5).is_some() && p.ask.is_none(), "not within a second of asking");
        assert!(p.ask_again(1.0).is_some() && p.ask.take().as_deref() == Some("make it louder"));
        assert!(p.ask_again(1.5).is_some() && p.ask.is_none());
        assert!(p.ask_again(61.0).is_none() && p.ask.is_none(), "a minute after the words were typed, no more");
        p.heard(answer(true));
        assert!(p.ask_again(2.0).is_none() && p.ask.is_none(), "found by meaning: nothing more to ask");
    }

    #[test]
    fn words_the_small_model_is_reading_are_asked_about_again_until_it_has_read_them() {
        let mut p = Palette { query: "can the kick be a bit softer".into(), ..Palette::default() };
        let query = p.query.clone();
        let answer = |reading| Answer { query: query.clone(), by_meaning: true, reading, ..Answer::default() };
        p.heard(answer(true));
        assert!(p.reading(0.1));
        assert_eq!(p.ask_again(0.1), Some(READING.0), "not within its interval of asking");
        assert!(p.ask.is_none());
        assert_eq!(p.ask_again(0.3), Some(READING.0));
        assert_eq!(p.ask.take().as_deref(), Some("can the kick be a bit softer"));
        assert!(p.ask_again(30.0).is_some(), "the first words wait for the model to download");
        assert!(p.ask_again(61.0).is_none(), "long after the words were typed, it stops asking");
        assert!(!p.reading(61.0), "and says no more that it is reading");
        p.heard(answer(false));
        assert!(!p.reading(0.6) && p.ask_again(0.6).is_none(), "read: nothing more to ask");
    }

    fn proposal(said: &str, commands: Vec<Command>) -> claudius_intent::Proposal {
        claudius_intent::Proposal {
            said: said.into(),
            detail: String::new(),
            commands,
            reading: claudius_intent::ask::Reading::default(),
            score: 0.9,
        }
    }

    fn muted(track: &str) -> Command {
        Command::SetMix {
            track: track.into(),
            level_db: None,
            pan: None,
            reverb: None,
            delay: None,
            duck: None,
            drive_db: None,
            muted: Some(true),
            solo: None,
            bus: None,
        }
    }

    #[test]
    fn what_a_request_asks_comes_first_each_reading_a_row_that_does_its_edits_as_one_change() {
        let mut p = Palette { query: "mute the drums".into(), ..Palette::default() };
        let mute = proposal("Mute kick and snare", vec![muted("kick"), muted("snare")]);
        p.heard(Answer {
            query: p.query.clone(),
            found: vec![act("tab:mixer")],
            proposed: vec![mute],
            by_meaning: true,
            ..Answer::default()
        });
        let rows = p.shown();
        assert_eq!(rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["Mute kick and snare", "tab:mixer"]);
        let Some(Run::Do { said, commands }) = p.run(&rows[0].id) else { panic!("{:?}", p.run(&rows[0].id)) };
        let sent = commands_of(said, commands);
        assert_eq!(
            sent,
            [Command::Together { said: "Mute kick and snare".into(), commands: vec![muted("kick"), muted("snare")] }],
            "one change, which undo calls what the button said"
        );
        p.query = "mute the drum".into();
        assert!(p.run(&rows[0].id).is_none(), "words typed since: what it read for others is done no more");
        assert_eq!(
            commands_of("Undo the last 2 changes".into(), vec![Command::Undo, Command::Undo]),
            [Command::Undo, Command::Undo],
            "undo is steps of its own"
        );
    }

    #[test]
    fn a_change_a_reading_goes_back_to_before_is_not_offered_again_in_the_history() {
        let mut p = Palette { query: "go back to before the whistles".into(), ..Palette::default() };
        let change = Found {
            id: "7@4f2a".into(),
            name: "track whistle".into(),
            place: "added the whistle track".into(),
            keys: String::new(),
            score: 0.7,
            by: By::Meaning,
        };
        let back = proposal("Go back to before track whistle", vec![Command::GoBack { to: "7@4f2a".into() }]);
        p.heard(Answer {
            query: p.query.clone(),
            done: vec![change],
            proposed: vec![back],
            by_meaning: true,
            ..Answer::default()
        });
        let rows = p.shown();
        assert_eq!(rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["Go back to before track whistle"]);
        assert_eq!(
            p.run(&rows[0].id),
            Some(Run::Do {
                said: "Go back to before track whistle".into(),
                commands: vec![Command::GoBack { to: "7@4f2a".into() }]
            })
        );
    }
}

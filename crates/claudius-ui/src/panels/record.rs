//! "Record": a take from a microphone or an audio interface. The song plays from a count-in before the section the
//! take starts with, and what the input hears from that section's first downbeat on goes to claudius, which plays it
//! once from there (`POST /api/takes`).

use claudius_api::SongView;
use eframe::egui::{self, RichText, Sense, Ui, vec2};
use serde::{Deserialize, Serialize};

use super::{Cx, small};
use crate::theme::{self, TEXT, TEXT_DIM, WELL};
use crate::widgets::{self, Icon};

/// what a take can be in the mix (`claudius::take::ROLES`)
const ROLES: &[&str] = &["vocal", "lead", "chords", "keys", "bass", "drums", "texture"];

/// how the window was set up last time
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// the input, by name (None: the computer's default)
    pub input: Option<String>,
    /// the first input channel recorded, and whether the one after it too
    pub channel: usize,
    pub stereo: bool,
    /// bars of count-in
    pub count_in: u32,
    /// the click goes on while the take records
    pub click: bool,
    pub role: String,
    /// milliseconds the setup adds that the devices do not report, to an audio take (Measure sets it)
    pub offset_ms: f32,
    /// and to a MIDI take, where only the song's way out counts: a key is timed as it is pressed
    pub midi_offset_ms: f32,
    /// a MIDI input to record what is played on, instead of an audio input
    pub midi: Option<String>,
    /// what plays the notes of a MIDI take, and the part they make
    pub sound: String,
    pub part_role: String,
    /// the grid a MIDI take's notes go to, in beats (0: as played)
    pub grid: f64,
    /// what a MIDI take's notes become (`claudius_api::NOTE_STYLES`)
    pub style: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            input: None,
            channel: 0,
            stereo: false,
            count_in: 1,
            click: false,
            role: "vocal".into(),
            offset_ms: 0.0,
            midi_offset_ms: 0.0,
            midi: None,
            sound: "epiano".into(),
            part_role: "keys".into(),
            grid: 0.25,
            style: "line".into(),
        }
    }
}

/// what a MIDI take's part can be
const PART_ROLES: &[&str] = &["keys", "lead", "chords", "pad", "arp", "bass"];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::record::Played;

    #[test]
    fn played_notes_go_to_the_grid_in_beats() {
        // half a second a beat (120 BPM): a note a little late and a little short, and a staccato one
        let played = [
            Played { start: 0.03, end: 0.49, pitch: 60, velocity: 0.7 },
            Played { start: 1.01, end: 1.06, pitch: 64, velocity: 0.9 },
        ];
        let on = to_beats(&played, 0.5, 0.25);
        assert_eq!(on.iter().map(|n| (n.t, n.d, n.p)).collect::<Vec<_>>(), [(0.0, 1.0, 60), (2.0, 0.25, 64)]);
        let loose = to_beats(&played, 0.5, 0.0);
        assert!((loose[0].t - 0.06).abs() < 1e-9 && (loose[1].d - 0.1).abs() < 1e-9, "as played");
    }

    #[test]
    fn a_take_that_dropped_audio_or_fell_silent_says_so_with_claudius_answer() {
        assert_eq!(take_note(0, 0), "");
        assert_eq!(take_note(1, 0), " (it dropped audio once: other apps may be busy with the audio)");
        assert!(take_note(3, 0).contains("dropped audio 3 times"));
        let both = take_note(2, 1);
        assert!(both.contains("2 times") && both.contains("went silent in places") && both.contains("Mic Mode"));
        let mut r = Record { note: take_note(0, 4), ..Default::default() };
        r.answered(true, "take1: a take of 4 bars".into());
        assert!(r.said().starts_with("take1: a take of 4 bars (the input went silent"), "{}", r.said());
        r.answered(true, "take2: a take of 2 bars".into());
        assert_eq!(r.said(), "take2: a take of 2 bars", "said once, of the take it was about");
    }

    #[test]
    fn a_measurement_says_how_late_through_what_and_what_it_set() {
        let route = "MacBook Pro Speakers and MacBook Pro Microphone";
        let late = measured(44.0, route);
        assert!(
            late.starts_with(&format!("the clicks came back 44 ms late through {route}, so the latency is now +44 ms"))
        );
        assert!(
            measured(-20.0, route).contains("20 ms early through") && measured(-20.0, route).contains("now -20 ms")
        );
        assert!(measured(-0.0, route).contains("back in time") && measured(-0.0, route).contains("now +0 ms"));
    }
}

/// played notes as notes in beats from the take's start, `beat` seconds each, on a grid of `grid` beats (0: as played)
fn to_beats(played: &[crate::audio::record::Played], beat: f64, grid: f64) -> Vec<claudius_api::NoteView> {
    let snap = |b: f64| if grid > 0.0 { (b / grid).round() * grid } else { b };
    let shortest = if grid > 0.0 { grid } else { 1.0 / 64.0 };
    played
        .iter()
        .map(|n| {
            let t = snap(n.start / beat);
            claudius_api::NoteView {
                t,
                d: (snap(n.end / beat) - t).max(shortest),
                p: i32::from(n.pitch),
                v: n.velocity,
            }
        })
        .collect()
}

/// a take on its way to claudius
pub struct Upload {
    pub wav: Vec<u8>,
    pub from: String,
    pub name: Option<String>,
    pub role: String,
}

#[derive(Default)]
pub struct Record {
    pub open: bool,
    pub settings: Settings,
    /// the section the take starts with
    from: Option<String>,
    name: String,
    pub upload: Option<Upload>,
    /// a take is to start now, as the Record button starts one (a scripted run's `rec:start`)
    pub start: bool,
    /// the take is to end now (Space, while it records)
    pub stop: bool,
    /// the latency is to be measured now, as the Measure button measures it (a scripted run's `rec:measure`)
    pub measure: bool,
    /// what to say, and whether it is good news
    status: Option<(bool, String)>,
    /// what was wrong with the take on its way, said again with claudius' answer
    note: String,
    /// why the meter cannot hear the input, while that is so
    deaf_why: Option<String>,
    sending: bool,
    level: f32,
    inputs: Vec<crate::audio::record::Input>,
    midi_inputs: Vec<String>,
    live: Option<Live>,
    /// the input the meter shows while no take records, and what it is listening to; or what it could not listen to
    listener: Option<(crate::audio::record::Listener, Listening)>,
    deaf: Option<Listening>,
}

/// an audio input and the channels of it, or a MIDI input
type Listening = (Option<String>, usize, bool, Option<String>);

/// a take being recorded, or the latency being measured
struct Live {
    recorder: crate::audio::record::Recorder,
    /// where in the song the take starts and the song ends, in seconds, and how long a beat is (for a MIDI take's
    /// notes)
    take_start: f64,
    song_end: f64,
    beat: f64,
    /// it records what a MIDI input plays
    midi: bool,
    /// it is a measurement of the latency, not a take
    measuring: bool,
    /// when it was asked to stop, while the recorder hands over the last of what it heard (in a browser)
    stopping: Option<f64>,
    /// since when the input has heard nothing at all, and whether for long enough to say so
    silent_since: Option<f64>,
    silent: bool,
}

impl Record {
    /// what the window last said of a take or a measurement, or why it cannot hear the input; or that it is waiting for
    /// the last of a take
    #[cfg(any(not(target_arch = "wasm32"), feature = "plan"))]
    pub fn said(&self) -> String {
        if self.live.as_ref().is_some_and(|l| l.stopping.is_some()) {
            return "finishing ...".into();
        }
        self.deaf_why.clone().or_else(|| self.status.as_ref().map(|(_, s)| s.clone())).unwrap_or_default()
    }

    /// what claudius answered to the take, with what the take's recording was told of
    pub fn answered(&mut self, ok: bool, message: String) {
        self.sending = false;
        if ok {
            self.name.clear();
        }
        self.status = Some((ok, format!("{message}{}", std::mem::take(&mut self.note))));
    }
}

impl Record {
    pub fn show(&mut self) {
        self.open = true;
        self.inputs = crate::audio::record::inputs();
        self.midi_inputs = crate::audio::record::midi_inputs();
    }

    pub fn recording(&self) -> bool {
        self.live.is_some()
    }
}

pub fn dialog(ctx: &egui::Context, cx: &mut Cx) {
    keep_up(ctx, cx);
    wake_while_stopping(ctx, cx);
    if !cx.view.record.open {
        return;
    }
    let mut open = true;
    egui::Window::new("Record a take")
        .id(egui::Id::unique("record"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(470.0)
        .pivot(egui::Align2::CENTER_TOP)
        .default_pos(ctx.content_rect().center_top() + vec2(0.0, 110.0))
        .show(ctx, |ui| body(ui, cx));
    // a take that is recording finishes first
    if !open && !cx.view.record.recording() {
        cx.view.record.open = false;
    }
    wake_while_stopping(ctx, cx);
}

/// Whatever asked a take to stop (the Stop buttons, Space, the song's end, a plan), nothing else wakes the window when
/// the browser's recorder has handed over the last of it.
fn wake_while_stopping(ctx: &egui::Context, cx: &Cx) {
    if cx.view.record.live.as_ref().is_some_and(|l| l.stopping.is_some()) {
        ctx.request_repaint();
    }
}

fn body(ui: &mut Ui, cx: &mut Cx) {
    let Some(song) = cx.state.song.clone() else {
        ui.label(RichText::new("Make a song first.").color(TEXT));
        return;
    };
    let selected = song.sections.get(cx.view.section).map(|s| s.name.clone());
    let unready = not_ready(cx, &song_audio(cx));
    let (instruments, synths) = super::melodic_sounds(cx.sounds);
    let r = &mut cx.view.record;
    if r.from.as_ref().is_none_or(|f| !song.sections.iter().any(|s| s.name == *f)) {
        r.from = selected.or_else(|| song.sections.first().map(|s| s.name.clone()));
    }
    let busy = r.recording() || r.sending;
    ui.label(small(
        "The song plays from a count-in, and the take records from the first downbeat of its section. Wear \
         headphones; to hear yourself without delay, monitor through your audio interface.",
    ));
    ui.add_space(8.0);
    let mut measure = false;
    egui::Grid::new("record-settings").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
        ui.label(small("Input"));
        ui.add_enabled_ui(!busy, |ui| input(ui, r));
        ui.end_row();
        ui.label(small("Starts with"));
        ui.add_enabled_ui(!busy, |ui| {
            ui.horizontal(|ui| {
                let from = r.from.clone().unwrap_or_default();
                egui::ComboBox::from_id_salt("record-from").selected_text(&from).width(110.0).show_ui(ui, |ui| {
                    for s in &song.sections {
                        ui.selectable_value(&mut r.from, Some(s.name.clone()), &s.name);
                    }
                });
                let count = |n: u32| match n {
                    0 => "no count-in".to_string(),
                    1 => "1 bar of count-in".to_string(),
                    n => format!("{n} bars of count-in"),
                };
                egui::ComboBox::from_id_salt("record-count-in").selected_text(count(r.settings.count_in)).show_ui(
                    ui,
                    |ui| {
                        for n in 0..=2 {
                            ui.selectable_value(&mut r.settings.count_in, n, count(n));
                        }
                    },
                );
                ui.checkbox(&mut r.settings.click, small("click while recording"));
            });
        });
        ui.end_row();
        ui.label(small("What it is"));
        let midi = r.settings.midi.is_some();
        ui.add_enabled_ui(!busy, |ui| {
            ui.horizontal(|ui| {
                if midi {
                    egui::ComboBox::from_id_salt("record-part-role")
                        .selected_text(&r.settings.part_role)
                        .width(70.0)
                        .show_ui(ui, |ui| {
                            for role in PART_ROLES {
                                ui.selectable_value(&mut r.settings.part_role, role.to_string(), *role);
                            }
                        })
                        .response
                        .on_hover_text("the part the notes make: keys, a lead, chords, a pad, an arp or a bass");
                    egui::ComboBox::from_id_salt("record-sound").selected_text(&r.settings.sound).width(100.0).show_ui(
                        ui,
                        |ui| {
                            for (heading, names) in [("instruments", &instruments), ("synths", &synths)] {
                                if !names.is_empty() {
                                    ui.label(small(heading).weak());
                                }
                                for s in names {
                                    ui.selectable_value(&mut r.settings.sound, s.clone(), s);
                                }
                            }
                        },
                    );
                    let grid = |g: f64| match g {
                        0.0 => "as played".to_string(),
                        0.25 => "on sixteenths".to_string(),
                        0.5 => "on eighths".to_string(),
                        _ => "on beats".to_string(),
                    };
                    egui::ComboBox::from_id_salt("record-grid").selected_text(grid(r.settings.grid)).show_ui(
                        ui,
                        |ui| {
                            for g in [0.25, 0.5, 1.0, 0.0] {
                                ui.selectable_value(&mut r.settings.grid, g, grid(g));
                            }
                        },
                    );
                    egui::ComboBox::from_id_salt("record-style")
                        .selected_text(format!("as {}", r.settings.style))
                        .show_ui(ui, |ui| {
                            for s in claudius_api::NOTE_STYLES {
                                ui.selectable_value(&mut r.settings.style, s.to_string(), *s);
                            }
                        })
                        .response
                        .on_hover_text(
                            "line plays the notes as played; harmony adds a third above, octaves doubles them \
                             below; chords puts a triad under each note, bass moves them down, arp breaks each into \
                             a rising arpeggio, pad holds a chord a bar",
                        );
                } else {
                    egui::ComboBox::from_id_salt("record-role")
                        .selected_text(&r.settings.role)
                        .width(80.0)
                        .show_ui(ui, |ui| {
                            for role in ROLES {
                                ui.selectable_value(&mut r.settings.role, role.to_string(), *role);
                            }
                        })
                        .response
                        .on_hover_text("what the take is, for the mix: it sets how loud claudius makes it");
                }
                let hint = if midi { "named played ..." } else { "named take1, take2 ..." };
                ui.add(egui::TextEdit::singleline(&mut r.name).hint_text(hint).desired_width(130.0))
                    .on_hover_text("the track's name: letters, digits, - and _");
            });
        });
        ui.end_row();
        ui.label(small("Latency"));
        ui.add_enabled_ui(!busy, |ui| {
            ui.horizontal(|ui| {
                let (offset, what) = if midi {
                    (
                        &mut r.settings.midi_offset_ms,
                        "what your headphones or speakers add that they do not report, such as Bluetooth ones: notes \
                         that land late want more",
                    )
                } else {
                    (
                        &mut r.settings.offset_ms,
                        "what your setup adds that the devices do not report, such as a laptop's speakers and \
                         microphone or Bluetooth headphones: a take that lands late wants more",
                    )
                };
                ui.add(
                    egui::DragValue::new(offset)
                        .range(-200.0..=600.0)
                        .speed(1.0)
                        .custom_formatter(|v, _| format!("{v:+.0} ms")),
                )
                .on_hover_text(what);
                if !midi {
                    measure = ui
                        .button(small("Measure"))
                        .on_hover_text(
                            "claudius plays eight clicks, listens for them, and sets the latency from how late they \
                             come back: turn the speakers up a little, or hold the headphones to the microphone",
                        )
                        .clicked();
                }
            });
        });
        ui.end_row();
    });
    ui.add_space(10.0);
    meter(ui, r.level);
    ui.add_space(10.0);
    let mut start = false;
    let mut stop = false;
    ui.horizontal(|ui| {
        if r.recording() {
            stop = widgets::tool_button(ui, Icon::Stop, "Stop", true, true).clicked();
        } else {
            let why = if r.sending { Some("the last take is still on its way".to_string()) } else { unready.clone() };
            start = widgets::tool_button(ui, Icon::Record, "Record", why.is_none(), true)
                .on_disabled_hover_text(why.unwrap_or_default())
                .clicked();
        }
        let (good, text) = what_now(r);
        if !text.is_empty() {
            let color = if good { TEXT_DIM } else { theme::BAD };
            ui.add(egui::Label::new(small(text).color(color)).wrap());
        }
    });
    if start {
        begin(cx, &song);
    }
    if measure {
        measure_latency(cx);
    }
    if stop {
        finish(cx);
    }
}

/// the input and which of its channels
fn input(ui: &mut Ui, r: &mut Record) {
    ui.horizontal(|ui| {
        if r.inputs.is_empty() && r.midi_inputs.is_empty() {
            ui.label(small("no microphone, audio input or MIDI keyboard found").color(theme::BAD));
            return;
        }
        if r.settings.midi.as_ref().is_some_and(|m| !r.midi_inputs.contains(m)) {
            r.settings.midi = None;
        }
        let name = r.settings.input.clone().filter(|n| r.inputs.iter().any(|i| i.name == *n));
        let shown = match &r.settings.midi {
            Some(m) => format!("MIDI: {m}"),
            None => name.clone().or_else(|| r.inputs.first().map(|i| i.name.clone())).unwrap_or_default(),
        };
        egui::ComboBox::from_id_salt("record-input").selected_text(&shown).width(190.0).show_ui(ui, |ui| {
            for i in &r.inputs {
                let on = r.settings.midi.is_none() && shown == i.name;
                if ui.selectable_label(on, &i.name).clicked() {
                    (r.settings.input, r.settings.midi) = (Some(i.name.clone()), None);
                }
            }
            let offer = crate::audio::record::midi_offer();
            if !r.midi_inputs.is_empty() || offer.is_some() {
                ui.label(small("MIDI").weak());
            }
            for m in &r.midi_inputs {
                if ui.selectable_label(r.settings.midi.as_ref() == Some(m), m).clicked() {
                    r.settings.midi = Some(m.clone());
                }
            }
            // a browser asks the person before the page may use MIDI keyboards
            match offer {
                Some((what, true)) => {
                    if ui.selectable_label(false, &what).clicked() {
                        crate::audio::record::ask_for_midi();
                    }
                }
                Some((why, false)) => {
                    ui.label(small(why).weak());
                }
                None => {}
            }
        });
        if r.settings.midi.is_some() {
            return;
        }
        let channels = r.inputs.iter().find(|i| i.name == shown).map_or(1, |i| i.channels as usize);
        let label = |first: usize, stereo: bool| {
            if stereo { format!("inputs {}+{}", first + 1, first + 2) } else { format!("input {}", first + 1) }
        };
        if r.settings.channel + usize::from(r.settings.stereo) >= channels {
            (r.settings.channel, r.settings.stereo) = (0, false);
        }
        egui::ComboBox::from_id_salt("record-channel")
            .selected_text(label(r.settings.channel, r.settings.stereo))
            .show_ui(ui, |ui| {
                for c in 0..channels {
                    let mono = (c, false);
                    if ui.selectable_label((r.settings.channel, r.settings.stereo) == mono, label(c, false)).clicked() {
                        (r.settings.channel, r.settings.stereo) = mono;
                    }
                }
                for c in (0..channels.saturating_sub(1)).step_by(2) {
                    let pair = (c, true);
                    if ui.selectable_label((r.settings.channel, r.settings.stereo) == pair, label(c, true)).clicked() {
                        (r.settings.channel, r.settings.stereo) = pair;
                    }
                }
            });
    });
}

/// how loud the input is: green, yellow from -12 dBFS, red from -1 (where it clips)
fn meter(ui: &mut Ui, level: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 8.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 2.0, WELL);
    let db = 20.0 * level.max(1e-6).log10();
    let full = ((db + 60.0) / 60.0).clamp(0.0, 1.0);
    let color = if db > -1.0 {
        theme::BAD
    } else if db > -12.0 {
        theme::WARN
    } else {
        theme::GOOD
    };
    let mut lit = rect;
    lit.set_width(rect.width() * full);
    p.rect_filled(lit, 2.0, color);
}

/// what the window says: where the take is, or what happened last
fn what_now(r: &Record) -> (bool, String) {
    if let Some(live) = &r.live {
        let t = live.recorder.time();
        let from = r.from.as_deref().unwrap_or("");
        if !live.recorder.playing() {
            return (true, "starting: allow the microphone if the browser asks".into());
        }
        if t < live.take_start {
            return (true, format!("count-in: {:.1} s to {from}", live.take_start - t));
        }
        if live.stopping.is_some() {
            return (true, "finishing ...".into());
        }
        if live.silent {
            return (false, NOTHING_IN.into());
        }
        if live.measuring {
            return (true, format!("listening for the clicks: {:.0} s", (live.song_end + 0.5 - t).max(0.0).ceil()));
        }
        return (true, format!("recording from {from}: {}", crate::mock::fmt_time(t - live.take_start)));
    }
    match &r.deaf_why {
        Some(why) => (false, why.clone()),
        None => r.status.clone().unwrap_or_default(),
    }
}

/// the render a take is recorded over: the player's, in the app; in the browser, fetched for the take (none while it
/// loads)
fn song_audio(cx: &Cx) -> Result<Option<std::sync::Arc<crate::audio::Pcm>>, String> {
    #[cfg(not(target_arch = "wasm32"))]
    return Ok(cx.player.pcm());
    #[cfg(target_arch = "wasm32")]
    {
        let Some(render) = &cx.state.render else { return Ok(None) };
        let path = render.url.strip_suffix(".mp3").map_or_else(|| render.url.clone(), |stem| format!("{stem}.wav"));
        let url = if path.contains("://") { path } else { format!("{}{path}", cx.server.unwrap_or_default()) };
        crate::audio::record::song(&url)
    }
}

/// why there is no take to record yet, when the render is not there to record over
fn not_ready(cx: &Cx, song: &Result<Option<std::sync::Arc<crate::audio::Pcm>>, String>) -> Option<String> {
    match song {
        Ok(Some(_)) => None,
        Ok(None) if cx.player.has_audio() => Some("the song is still loading for the take".into()),
        Ok(None) => Some("render the song first".into()),
        Err(e) => Some(e.clone()),
    }
}

fn begin(cx: &mut Cx, song: &SongView) {
    use crate::audio::record::{Plan, Recorder};
    let beat = 60.0 / cx.audio_bpm().max(1.0);
    let audio = song_audio(cx);
    if let Some(why) = not_ready(cx, &audio) {
        cx.view.record.status = Some((false, why));
        return;
    }
    let Ok(Some(pcm)) = audio else { return };
    let r = &mut cx.view.record;
    let Some(section) = r.from.as_ref().and_then(|f| song.sections.iter().find(|s| s.name == *f)) else {
        r.status = Some((false, "choose the section the take starts with".into()));
        return;
    };
    let (start, end) = (section.start_bar as f64 * 4.0 * beat, pcm.seconds());
    let plan = Plan {
        input: r.settings.input.clone(),
        midi: r.settings.midi.clone(),
        channel: r.settings.channel,
        stereo: r.settings.stereo,
        song: pcm,
        start,
        count_in: r.settings.count_in as f64 * 4.0 * beat,
        beat,
        bar: 4,
        click: r.settings.click,
        offset: if r.settings.midi.is_some() { r.settings.midi_offset_ms } else { r.settings.offset_ms } as f64
            / 1000.0,
    };
    cx.player.pause();
    // the recorder opens the input itself: some systems give a device to one stream at a time
    r.listener = None;
    match Recorder::start(plan) {
        Ok(recorder) => {
            let midi = r.settings.midi.is_some();
            r.live = Some(Live {
                recorder,
                take_start: start,
                song_end: end,
                beat,
                midi,
                measuring: false,
                stopping: None,
                silent_since: None,
                silent: false,
            });
            (r.status, r.level) = (None, 0.0);
        }
        Err(e) => r.status = Some((false, e)),
    }
}

/// starts measuring the latency: claudius plays its clicks and listens for them on the chosen input
fn measure_latency(cx: &mut Cx) {
    use crate::audio::record::{Plan, Recorder};
    cx.player.pause();
    let r = &mut cx.view.record;
    r.listener = None;
    let plan = Plan::measure(r.settings.input.clone(), r.settings.channel);
    let end = plan.song.seconds();
    match Recorder::start(plan) {
        Ok(recorder) => {
            r.live = Some(Live {
                recorder,
                take_start: 0.0,
                song_end: end,
                beat: 0.5,
                midi: false,
                measuring: true,
                stopping: None,
                silent_since: None,
                silent: false,
            });
            (r.status, r.level) = (None, 0.0);
        }
        Err(e) => r.status = Some((false, e)),
    }
}

/// what to add to what is said of a take that dropped audio, or fell silent in places (`take::hushed`)
fn take_note(dropouts: usize, hushed: usize) -> String {
    let mut note = String::new();
    if dropouts > 0 {
        let times = if dropouts == 1 { "once".to_string() } else { format!("{dropouts} times") };
        note += &format!(" (it dropped audio {times}: other apps may be busy with the audio)");
    }
    if hushed > 0 {
        note += &format!(" ({})", crate::audio::take::HUSHED);
    }
    note
}

/// what a measurement found, as the window says it: `ms` the clicks came back late by, through `route`
fn measured(ms: f32, route: &str) -> String {
    let when = match ms {
        m if m.abs() < 1.0 => "in time".to_string(),
        m if m > 0.0 => format!("{m:.0} ms late"),
        m => format!("{:.0} ms early", -m),
    };
    format!(
        "the clicks came back {when} through {route}, so the latency is now {:+.0} ms: measure again for other \
         speakers, headphones or inputs",
        if ms.abs() < 1.0 { 0.0 } else { ms }
    )
}

/// what to say when the input hears nothing at all while a take records
#[cfg(not(target_arch = "wasm32"))]
const NOTHING_IN: &str = "nothing is coming in: check the input and its channel; on a Mac, allow the microphone for \
                          Claudius (or your terminal) in System Settings, Privacy & Security, Microphone";
#[cfg(target_arch = "wasm32")]
const NOTHING_IN: &str = "nothing is coming in: check the input and its channel, and that the browser may use the \
                          microphone (the site settings in the address bar); on a Mac, the system has to allow the \
                          browser too, and Control Center's Mic Mode should be Standard";

/// Stops the take (or the measurement) and sends it: at once in the app; in a browser once the recorder has handed
/// over the last of what it heard, which `keep_up` waits for.
pub fn finish(cx: &mut Cx) {
    let now = cx.now;
    let Some(live) = cx.view.record.live.as_mut() else { return };
    live.recorder.stop_soon();
    if live.recorder.drained() {
        done(cx);
    } else {
        live.stopping.get_or_insert(now);
    }
}

/// the take is all in: a recording to keep with the song, or the notes played as a part, on its way to claudius
fn done(cx: &mut Cx) {
    use crate::audio::record::Take;
    let r = &mut cx.view.record;
    let Some(live) = r.live.take() else { return };
    if live.measuring {
        let route = live.recorder.route().to_string();
        r.status = Some(match live.recorder.lateness() {
            Ok(late) => {
                let ms = ((late * 1000.0).round() as f32).clamp(-200.0, 600.0);
                r.settings.offset_ms = ms;
                (true, measured(ms, &route))
            }
            Err(e) => (false, e),
        });
        return;
    }
    let name = Some(r.name.trim().to_string()).filter(|n| !n.is_empty());
    let from = r.from.clone().unwrap_or_default();
    match live.recorder.finish() {
        Ok(Take::Audio { peak: 0.0, .. }) => {
            r.status = Some((false, "the take is silent: nothing came in from the input".into()));
        }
        Ok(Take::Audio { wav, secs, dropouts, hushed, .. }) => {
            r.note = take_note(dropouts, hushed);
            r.status = Some((true, format!("sending {secs:.1} s to claudius ...{}", r.note)));
            r.sending = true;
            r.upload = Some(Upload { wav, from, name, role: r.settings.role.clone() });
        }
        Ok(Take::Notes(played)) => {
            let notes = to_beats(&played, live.beat, r.settings.grid);
            r.status = Some((true, format!("sending {} notes to claudius ...", notes.len())));
            r.sending = true;
            cx.out.push(claudius_api::Command::AddNotes {
                from,
                notes,
                sounds: vec![r.settings.sound.clone()],
                name,
                role: Some(r.settings.part_role.clone()),
                style: Some(r.settings.style.clone()),
            });
        }
        Err(e) => r.status = Some((false, e)),
    }
}

/// every frame, natively: takes in what the input heard, keeps the meter moving, and stops at the song's end
fn keep_up(ctx: &egui::Context, cx: &mut Cx) {
    let now = cx.now;
    let r = &mut cx.view.record;
    if std::mem::take(&mut r.stop) && r.live.is_some() {
        finish(cx);
        return;
    }
    // a plan's start waits while the song loads for the take, as a person waits for the Record button
    let loading = r.start && cx.player.has_audio() && matches!(song_audio(cx), Ok(None));
    let r = &mut cx.view.record;
    if !loading
        && std::mem::take(&mut r.start)
        && r.live.is_none()
        && let Some(song) = cx.state.song.clone()
    {
        begin(cx, &song);
        return;
    }
    let r = &mut cx.view.record;
    if std::mem::take(&mut r.measure) && r.live.is_none() {
        measure_latency(cx);
        return;
    }
    let r = &mut cx.view.record;
    if let Some(asked) = r.live.as_ref().and_then(|l| l.stopping) {
        // a second is far longer than the recorder takes to hand over what it held
        if r.live.as_ref().is_some_and(|l| l.recorder.drained()) || now - asked > 1.0 {
            done(cx);
        } else {
            ctx.request_repaint();
        }
        return;
    }
    let Some(live) = r.live.as_mut() else {
        listen(ctx, r);
        return;
    };
    r.listener = None;
    live.recorder.tick();
    let heard = live.recorder.level();
    r.level = (r.level * 0.85).max(heard);
    if let Some(e) = live.recorder.error() {
        r.live = None;
        r.status = Some((false, e));
        return;
    }
    let t = live.recorder.time();
    // a keyboard is quiet between notes: only an audio input that hears nothing at all is a problem
    if !live.midi && t >= live.take_start && heard == 0.0 {
        let since = *live.silent_since.get_or_insert(now);
        live.silent = now - since > 2.5;
    } else {
        (live.silent_since, live.silent) = (None, false);
    }
    ctx.request_repaint();
    if t >= live.song_end + 0.5 {
        finish(cx);
    }
}

/// while the window is open and no take records, the meter shows what the chosen input hears
fn listen(ctx: &egui::Context, r: &mut Record) {
    if !r.open {
        (r.listener, r.deaf, r.deaf_why, r.level) = (None, None, None, 0.0);
        return;
    }
    // a browser lists its inputs when it gets round to it, names them once the page may use the microphone, and
    // lists MIDI keyboards once it may use those
    #[cfg(target_arch = "wasm32")]
    {
        r.inputs = crate::audio::record::inputs();
        r.midi_inputs = crate::audio::record::midi_inputs();
    }
    // a channel remembered from another input may be one this input does not have
    let shown = r.settings.input.clone().filter(|n| r.inputs.iter().any(|i| i.name == *n));
    let channels = shown.as_ref().map_or_else(|| r.inputs.first(), |n| r.inputs.iter().find(|i| i.name == *n));
    if let Some(i) = channels
        && r.settings.channel + usize::from(r.settings.stereo) >= i.channels as usize
    {
        (r.settings.channel, r.settings.stereo) = (0, false);
    }
    let want: Listening = (r.settings.input.clone(), r.settings.channel, r.settings.stereo, r.settings.midi.clone());
    if r.listener.as_ref().is_none_or(|(_, on)| *on != want) && r.deaf.as_ref() != Some(&want) {
        r.listener = None;
        let started = match &want.3 {
            Some(m) => crate::audio::record::Listener::midi(m),
            None => crate::audio::record::Listener::start(want.0.as_deref(), want.1, want.2),
        };
        match started {
            Ok(l) => (r.listener, r.deaf, r.deaf_why) = (Some((l, want)), None, None),
            Err(e) => (r.deaf, r.deaf_why) = (Some(want), Some(e)),
        }
    }
    let Some((l, _)) = &r.listener else {
        r.level = 0.0;
        return;
    };
    if let Some(e) = l.error() {
        r.deaf = r.listener.take().map(|(_, on)| on);
        r.deaf_why = Some(e);
        return;
    }
    r.level = (r.level * 0.85).max(l.level());
    ctx.request_repaint_after(std::time::Duration::from_millis(40));
}

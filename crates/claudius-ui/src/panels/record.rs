//! "Record": a take from a microphone or an audio interface. The song plays from a count-in before the section the
//! take starts with, and what the input hears from that section's first downbeat on waits to be played back in the
//! song, recorded again or kept: kept, it goes to claudius, which plays it once from there (`POST /api/takes`). A take
//! an assistant asks for (`request_take`) opens the window set up for it, and goes to the assistant once it is sent.

use std::sync::Arc;

use claudius_api::{PitchCheck, SongView, TakeRequest, TakeStatus};
use eframe::egui::{self, RichText, Sense, Ui, vec2};
use serde::{Deserialize, Serialize};

use super::{Cx, small};
use crate::theme::{self, TEXT, TEXT_DIM, WELL};
use crate::widgets::{self, Icon};

/// what a take can be in the mix (`claudius::take::ROLES`)
pub(crate) const ROLES: &[&str] = &["vocal", "lead", "chords", "keys", "bass", "drums", "texture"];

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

/// what a MIDI take's part can be: a part's role, or the song's drums
const PART_ROLES: &[&str] = &["keys", "lead", "chords", "pad", "arp", "bass", DRUMS];
/// what a MIDI take plays as the song's drum lanes: a drum pad's notes, by the General MIDI drum map
const DRUMS: &str = "drums";

/// the drum a General MIDI note plays (channel 10's map, which drum pads and drum apps send), as a lane's role
fn gm_drum(pitch: u8) -> Option<&'static str> {
    Some(match pitch {
        35 | 36 => "kick",
        38 | 40 => "snare",
        39 => "clap",
        42 | 44 => "hat",
        46 => "ohat",
        41 | 43 | 45 | 47 | 48 | 50 => "tom",
        49 | 52 | 55 | 57 => "crash",
        51 | 53 | 59 => "ride",
        54 | 69 | 70 | 82 => "shaker",
        37 | 56 | 58 | 60..=68 | 71..=81 => "perc",
        _ => return None,
    })
}

/// the roles of lanes that play a drum when the song has no lane of its own role
fn stand_ins(role: &str) -> &'static [&'static str] {
    match role {
        "snare" => &["clap"],
        "clap" => &["snare"],
        "ohat" => &["hat"],
        "hat" => &["shaker", "ohat"],
        "ride" => &["hat", "ohat"],
        "crash" => &["ride", "ohat"],
        "tom" => &["perc"],
        "shaker" => &["hat", "perc"],
        "perc" => &["shaker"],
        _ => &[],
    }
}

/// Notes played on drum pads as hits on the song's lanes (`(name, role)`): each note's drum (`gm_drum`) on the lane
/// of its role, or of one that stands in for it, in beats from the take's start; with the drums the song has no lane
/// for, and the notes the map has no drum for. A pad struck before the take began is no hit of it, unless it was struck
/// early by less than half a step of the `grid`, for the downbeat.
fn to_hits(
    played: &[crate::audio::record::Played],
    beat: f64,
    grid: f64,
    lanes: &[(String, String)],
) -> (Vec<claudius_api::HitView>, Vec<&'static str>, usize) {
    let (mut missing, mut strange) = (Vec::new(), 0);
    let early = if grid > 0.0 { grid / 2.0 } else { 1.0 / 32.0 } * beat;
    let hits = played
        .iter()
        .filter(|n| n.start >= -early)
        .filter_map(|n| {
            let Some(drum) = gm_drum(n.pitch) else {
                strange += 1;
                return None;
            };
            let roles = std::iter::once(drum).chain(stand_ins(drum).iter().copied());
            let lane = roles.into_iter().find_map(|role| lanes.iter().find(|(_, r)| r == role));
            if lane.is_none() && !missing.contains(&drum) {
                missing.push(drum);
            }
            lane.map(|(name, _)| claudius_api::HitView {
                lane: name.clone(),
                t: n.start.max(0.0) / beat,
                v: n.velocity,
            })
        })
        .collect();
    (hits, missing, strange)
}

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
        let held = to_beats(&[Played { start: -1.0, end: 0.5, pitch: 60, velocity: 0.5 }], 0.5, 0.25);
        assert_eq!((held[0].t, held[0].d), (0.0, 1.0), "a key held across the take's start starts with it");
    }

    #[test]
    fn drum_pads_play_the_songs_lanes_by_the_general_midi_map() {
        let pad = |start, pitch| Played { start, end: start + 0.1, pitch, velocity: 0.8 };
        // a kick a little early for the downbeat, a snare (the song has a clap), an open hat (it has a closed one), a
        // tom (no lane), a note no drum, and a kick struck a sixteenth before the take began
        let played = [pad(-0.02, 36), pad(0.5, 38), pad(1.0, 46), pad(1.25, 45), pad(1.5, 20), pad(-0.125, 36)];
        let lanes: Vec<(String, String)> =
            [("kick", "kick"), ("clap", "clap"), ("hat", "hat")].map(|(n, r)| (n.into(), r.into())).to_vec();
        let (hits, missing, strange) = to_hits(&played, 0.5, 0.25, &lanes);
        let on = hits.iter().map(|h| (h.lane.as_str(), h.t)).collect::<Vec<_>>();
        assert_eq!(on, [("kick", 0.0), ("clap", 1.0), ("hat", 2.0)]);
        assert_eq!((missing, strange), (vec!["tom"], 1));
        assert!((hits[0].v - 0.8).abs() < 1e-6, "as hard as it was struck");
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
    fn drums_asked_for_are_a_beat_to_hear_not_drum_pads() {
        let ask = TakeRequest {
            id: 1,
            from: "verse".into(),
            role: Some("drums".into()),
            name: None,
            note: "beatbox a bar".into(),
            status: TakeStatus::Waiting,
            answer: None,
        };
        let mut r = Record::default();
        assert!(r.set_up_for(Some(&ask)));
        assert_eq!((r.settings.role.as_str(), r.settings.part_role.as_str()), ("drums", "keys"), "no pads");
    }

    #[test]
    fn an_input_that_hears_nothing_at_all_says_how_to_let_claudius_use_the_microphone() {
        let mut r = Record::default();
        assert_eq!(what_now(&r).1, "");
        r.quiet = true;
        let (good, said) = what_now(&r);
        assert!(!good && said.starts_with("nothing is coming in") && hears_nothing(&r), "{said}");
        #[cfg(target_os = "macos")]
        assert!(said.contains("Privacy & Security, Microphone") && crate::audio::record::privacy_page().is_some());
        #[cfg(windows)]
        assert!(
            said.contains("Let desktop apps access your microphone") && crate::audio::record::privacy_page().is_some()
        );
        r.deaf_why = Some("no microphone or audio input".into());
        assert_eq!(what_now(&r).1, "no microphone or audio input", "why it cannot listen at all comes first");
    }

    #[test]
    fn a_take_asked_for_sets_the_window_up_once_and_says_when_the_assistant_has_it() {
        let ask = |id, status, role: &str| TakeRequest {
            id,
            from: "chorus".into(),
            role: Some(role.into()),
            name: Some("hook".into()),
            note: "softly".into(),
            status,
            answer: None,
        };
        let mut r = Record::default();
        assert!(r.set_up_for(Some(&ask(1, TakeStatus::Waiting, "pad"))));
        let set = |r: &Record| (r.from.clone(), r.name.clone(), r.settings.role.clone(), r.settings.part_role.clone());
        assert_eq!(set(&r), (Some("chorus".into()), "hook".into(), "vocal".into(), "pad".into()), "a pad is a part");
        assert!(!r.set_up_for(Some(&ask(1, TakeStatus::Waiting, "pad"))), "once");
        assert!(!r.set_up_for(Some(&ask(2, TakeStatus::Cancelled, "bass"))), "not once it waits no more");
        assert!(!r.set_up_for(None));
        // a take that waits to be kept keeps the window as it was recorded, until it is kept or thrown away
        let song = Arc::new(crate::audio::Pcm { rate: 48_000, channels: 2, samples: Vec::new() });
        let from = Some("intro".to_string());
        (r.from, r.held) = (
            from.clone(),
            Some(Held { wav: Vec::new(), secs: 1.0, from: "intro".into(), song, start: 0.0, bar: 2.0, request: None }),
        );
        assert!(!r.set_up_for(Some(&ask(3, TakeStatus::Waiting, "bass"))));
        assert_eq!((&r.from, r.settings.role.as_str()), (&from, "vocal"), "not set up for the request yet");
        r.held = None;
        assert!(r.set_up_for(Some(&ask(3, TakeStatus::Waiting, "bass"))));
        assert_eq!((r.settings.role.as_str(), r.settings.part_role.as_str()), ("bass", "bass"));
        r.answers = Some(3);
        r.answered(true, "hook: a take of 4 bars".into());
        assert_eq!(r.said(), "hook: a take of 4 bars (the assistant has it now)");
        r.answers = Some(3);
        r.answered(false, "no section 'x'".into());
        assert_eq!(r.said(), "no section 'x'", "not when claudius turned it down");
    }

    #[test]
    fn a_pitch_check_is_taken_up_once_and_its_guide_plays_its_notes_on_their_beats() {
        let note = claudius_api::NoteView { t: 1.0, d: 0.9, p: 69, v: 0.8 };
        let c = PitchCheck {
            id: 4,
            bpm: 90.0,
            notes: vec![note],
            answers: vec![(5.0, 10.0)],
            beats: 10.0,
            ..Default::default()
        };
        let mut r = Record::default();
        assert!(r.set_up_check(Some(&c)));
        assert!(!r.set_up_check(Some(&c)), "once");
        let sung = PitchCheck { id: 5, status: TakeStatus::Taken, ..c.clone() };
        assert!(!r.set_up_check(Some(&sung)), "a check already sung opens nothing");
        // the window is the check's, unless a take is in hand: that is finished first
        assert!(r.checking(true) && !r.checking(false));
        let song = Arc::new(crate::audio::Pcm { rate: 48_000, channels: 2, samples: Vec::new() });
        r.held =
            Some(Held { wav: Vec::new(), secs: 1.0, from: "drop".into(), song, start: 0.0, bar: 2.0, request: None });
        assert!(r.take_in_hand() && !r.checking(true), "a take waiting to be kept keeps the window");
        (r.held, r.sending) = (None, true);
        assert!(!r.checking(true), "nor one on its way");
        r.check_sending = true;
        assert!(r.checking(false), "a check's answers on their way are the check's");
        let g = guide(&c);
        let beat = (60.0 / 90.0 * 48_000.0) as usize;
        assert_eq!((g.rate, g.channels, g.frames()), (48_000, 2, 10 * beat));
        assert!(g.samples[..2 * beat].iter().all(|&v| v == 0.0), "nothing before its beat");
        let a4: Vec<f32> = g.samples[2 * (beat + 4_800)..2 * (beat + 14_400)].iter().step_by(2).copied().collect();
        let crossings = a4.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
        assert!((crossings as i32 - 88).abs() <= 2, "A4 for 0.2 s: {crossings} rising crossings");
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
            // a key held across the take's start starts with it
            let t = snap(n.start.max(0.0) / beat);
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
    /// the take request (`request_take`) it answers
    pub request: Option<u64>,
    /// the name of the file it is, which says its kind when its bytes do not (an AAC file's)
    pub file: Option<String>,
    /// the library's record of it, when a free library offered it: what the demo's engine credits it by
    pub found: Option<serde_json::Value>,
}

/// a take recorded and not kept yet
struct Held {
    wav: Vec<u8>,
    secs: f64,
    from: String,
    /// the song it was recorded over, where in it the take starts, and how long a bar is, for playing it back
    song: Arc<crate::audio::Pcm>,
    start: f64,
    bar: f64,
    /// the take request it was recorded for
    request: Option<u64>,
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
    /// the take is to be played back, kept, or the take asked for turned down now, as their buttons do (a scripted
    /// run's `rec:play`, `rec:keep`, `rec:not-now`)
    pub play: bool,
    pub keep: bool,
    pub not_now: bool,
    /// the take request (`request_take`) the window last opened for, and the one a take on its way answers
    asked: u64,
    answers: Option<u64>,
    /// a pitch check is to start now, as its Start button starts one (a scripted run's `rec:check`)
    pub check: bool,
    /// the pitch check the window last opened for; one's answers on their way to claudius (its id, the recording);
    /// and whether they are still on their way
    checked: u64,
    pub check_upload: Option<(u64, Vec<u8>)>,
    check_sending: bool,
    /// a take that has stopped, waiting to be kept, recorded again or thrown away; and what plays it back in the song
    held: Option<Held>,
    preview: Option<crate::audio::Player>,
    /// what to say, and whether it is good news
    status: Option<(bool, String)>,
    /// what was wrong with the take on its way, said again with claudius' answer
    note: String,
    /// why the meter cannot hear the input, while that is so
    deaf_why: Option<String>,
    /// since when (the window's clock) the audio input the meter listens to has heard nothing at all, to the bit; and
    /// whether that has gone on long enough to be the system keeping the microphone from claudius
    quiet_since: Option<f64>,
    quiet: bool,
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
/// (and whether that MIDI input is listed yet: a browser lists them late, and the meter tries again once it does)
type Listening = (Option<String>, usize, bool, Option<String>, bool);

/// a take being recorded, or the latency being measured
struct Live {
    recorder: crate::audio::record::Recorder,
    /// where in the song the take starts and the song ends, in seconds, and how long a beat is (for a MIDI take's
    /// notes)
    take_start: f64,
    song_end: f64,
    beat: f64,
    /// the song it records over, and the take request it was started for
    song: Arc<crate::audio::Pcm>,
    request: Option<u64>,
    /// it records what a MIDI input plays
    midi: bool,
    /// it is a measurement of the latency, not a take
    measuring: bool,
    /// it is the answers to a pitch check, by its id
    check: Option<u64>,
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
        if let Some(playing) = self.playing_back() {
            return playing;
        }
        self.deaf_why.clone().or_else(|| self.status.as_ref().map(|(_, s)| s.clone())).unwrap_or_default()
    }

    /// what claudius answered to the take, with what the take's recording was told of, and whether the assistant
    /// that asked for it has it now
    pub fn answered(&mut self, ok: bool, message: String) {
        self.sending = false;
        if ok {
            self.name.clear();
        }
        let sent = if ok && self.answers.is_some() { " (the assistant has it now)" } else { "" };
        self.answers = None;
        self.status = Some((ok, format!("{message}{}{sent}", std::mem::take(&mut self.note))));
    }

    /// a pitch check asked for opens the window for it, once
    pub fn check_asked(&mut self, check: Option<&PitchCheck>) {
        if self.set_up_check(check) {
            self.show();
        }
    }

    /// Takes up a pitch check asked for, once; whether it was one it had not taken up yet. A take that records, waits
    /// to be kept or is on its way keeps the window as it is until it is done with.
    fn set_up_check(&mut self, check: Option<&PitchCheck>) -> bool {
        if self.live.is_some() || self.held.is_some() || self.sending {
            return false;
        }
        let Some(c) = check.filter(|c| c.status == TakeStatus::Waiting && c.id != self.checked) else { return false };
        (self.checked, self.status) = (c.id, None);
        true
    }

    /// a take in hand: one recording, waiting to be kept, or on its way (a pitch check waits for it)
    fn take_in_hand(&self) -> bool {
        self.live.as_ref().is_some_and(|l| l.check.is_none())
            || self.held.is_some()
            || (self.sending && !self.check_sending)
    }

    /// the window is the pitch check's: one waits, is being sung, or is on its way, and no take is in hand
    fn checking(&self, waiting: bool) -> bool {
        !self.take_in_hand() && (waiting || self.live.as_ref().is_some_and(|l| l.check.is_some()) || self.check_sending)
    }

    /// what claudius heard in a pitch check's answers (the How it is sung window shows all of it)
    pub fn check_answered(&mut self, ok: bool, message: String) {
        (self.sending, self.check_sending) = (false, false);
        self.status = Some(if ok {
            (true, "the pitch check is in: your voice is in the How it is sung window".into())
        } else {
            (false, message)
        });
    }

    /// a take an assistant asks for opens the window, set up for it
    pub fn asked(&mut self, request: Option<&TakeRequest>) {
        if self.set_up_for(request) {
            self.show();
        }
    }

    /// Sets the window up for a take an assistant asks for, once: the section it starts with, what it is in the mix
    /// (or the part a keyboard's notes make), and its name. Whether it was a request it had not set up for yet. A take
    /// that records, waits to be kept or is on its way keeps the window as it is until it is done with.
    fn set_up_for(&mut self, request: Option<&TakeRequest>) -> bool {
        if self.live.is_some() || self.held.is_some() || self.sending {
            return false;
        }
        let Some(t) = request.filter(|t| t.status == TakeStatus::Waiting && t.id != self.asked) else { return false };
        self.asked = t.id;
        self.from = Some(t.from.clone());
        if let Some(role) = t.role.as_deref() {
            if ROLES.contains(&role) {
                self.settings.role = role.into();
            }
            // an assistant's drums are a take to hear (a beat played or beatboxed), not drum pads on MIDI, which answer
            // no request
            if PART_ROLES.contains(&role) && role != DRUMS {
                self.settings.part_role = role.into();
            }
        }
        if let Some(name) = &t.name {
            self.name = name.clone();
        }
        self.status = None;
        true
    }

    /// where the take being played back is, while it plays
    fn playing_back(&self) -> Option<String> {
        let p = self.preview.as_ref().filter(|p| p.is_playing())?;
        let at = |t: f64| crate::mock::fmt_time(t);
        Some(format!("playing it back: {} of {}", at(p.position()), at(p.duration())))
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
    let checking = cx.view.record.checking(waiting_check(cx.state).is_some());
    egui::Window::new(if checking { "Check your pitch" } else { "Record a take" })
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
    if check_body(ui, cx) {
        return;
    }
    let Some(song) = cx.state.song.clone() else {
        ui.label(RichText::new("Make a song first.").color(TEXT));
        ui.add_space(8.0);
        offer_check(ui, cx);
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
    let asked = waiting(cx.state);
    if let Some(t) = &asked {
        let note = if t.note.is_empty() { String::new() } else { format!(": {}", t.note) };
        let mut not_now = false;
        egui::Frame::new().fill(theme::mix(WELL, theme::ACCENT, 0.16)).corner_radius(6).inner_margin(8).show(
            ui,
            |ui| {
                ui.horizontal(|ui| {
                    not_now = ui
                        .button(small("Not now"))
                        .on_hover_text("tell the assistant that you will not record it")
                        .clicked();
                    let asks = format!("An assistant asks for a take from {}{note}", t.from);
                    ui.add(egui::Label::new(RichText::new(asks).color(TEXT)).wrap());
                });
            },
        );
        r.not_now |= not_now;
        ui.add_space(8.0);
    }
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
                        .on_hover_text(
                            "the part the notes make: keys, a lead, chords, a pad, an arp or a bass; or the song's \
                             drums, struck on drum pads (by the General MIDI drum map, which pads and drum apps send)",
                        );
                }
                let drums = midi && r.settings.part_role == DRUMS;
                if midi && !drums {
                    let tip = |s: &str| super::named(super::sound_about(s, cx.sounds).as_deref(), s);
                    egui::ComboBox::from_id_salt("record-sound")
                        .selected_text(claudius_api::words::name_in_words(&r.settings.sound))
                        .width(100.0)
                        .truncate()
                        .show_ui(ui, |ui| {
                            for (heading, names) in [("instruments", &instruments), ("synths", &synths)] {
                                if !names.is_empty() {
                                    ui.label(small(heading).weak());
                                }
                                for s in names {
                                    ui.selectable_value(
                                        &mut r.settings.sound,
                                        s.clone(),
                                        claudius_api::words::name_in_words(s),
                                    )
                                    .on_hover_text(tip(s));
                                }
                            }
                        })
                        .response
                        .on_hover_text(tip(&r.settings.sound));
                }
                if midi {
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
                }
                if midi && !drums {
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
                }
                if !midi {
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
                // drums go into the song's lanes, which have their names
                if !drums {
                    let hint = if midi { "named played ..." } else { "named take1, take2 ..." };
                    ui.add(egui::TextEdit::singleline(&mut r.name).hint_text(hint).desired_width(130.0))
                        .on_hover_text("the track's name: letters, digits, - and _");
                }
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
    let mut throw = false;
    ui.horizontal(|ui| {
        if r.recording() {
            stop = widgets::tool_button(ui, Icon::Stop, "Stop", true, true).clicked();
        } else {
            let why = if r.sending { Some("the last take is still on its way".to_string()) } else { unready.clone() };
            let again = r.held.is_some();
            start = widgets::tool_button(
                ui,
                Icon::Record,
                if again { "Record again" } else { "Record" },
                why.is_none(),
                !again,
            )
            .on_disabled_hover_text(why.unwrap_or_default())
            .clicked();
        }
        if r.held.is_some() && !r.recording() {
            let playing = r.preview.as_ref().is_some_and(|p| p.is_playing());
            let (icon, label) = if playing { (Icon::Stop, "Stop playing") } else { (Icon::Play, "Play it back") };
            r.play |= widgets::tool_button(ui, icon, label, true, false)
                .on_hover_text("the song from a bar before the take, with the take in it")
                .clicked();
            let answers =
                r.held.as_ref().and_then(|h| h.request).is_some_and(|id| asked.as_ref().is_some_and(|t| t.id == id));
            let (label, what) = if answers {
                ("Send it", "the take goes into the song, and to the assistant that asked for it")
            } else {
                ("Keep it", "the take goes into the song")
            };
            r.keep |= widgets::tool_button(ui, Icon::Save, label, true, true).on_hover_text(what).clicked();
            throw = ui.button(small("Throw it away")).clicked();
        }
    });
    say_now(ui, r);
    if throw {
        (r.held, r.preview, r.status) = (None, None, Some((true, "thrown away".into())));
    }
    ui.add_space(10.0);
    offer_check(ui, cx);
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

/// a pitch check to sing, being sung, or on its way to claudius: the window is for it alone until it is done
fn check_body(ui: &mut Ui, cx: &mut Cx) -> bool {
    let waiting = waiting_check(cx.state);
    let r = &mut cx.view.record;
    if !r.checking(waiting.is_some()) {
        return false;
    }
    let singing = r.live.as_ref().and_then(|l| l.check.map(|id| (id, l.recorder.time(), l.beat)));
    let note = waiting.as_ref().map(|c| c.note.clone()).unwrap_or_default();
    let asks = if note.is_empty() {
        "A pitch check".to_string()
    } else {
        format!("An assistant asks for a pitch check: {note}")
    };
    egui::Frame::new().fill(theme::mix(WELL, theme::ACCENT, 0.16)).corner_radius(6).inner_margin(8).show(ui, |ui| {
        ui.add(egui::Label::new(RichText::new(asks).color(TEXT)).wrap());
    });
    ui.add_space(6.0);
    ui.label(small(
        "Six short phrases play, one at a time. Sing each back in the gap after it, in whatever octave suits you: about \
         a minute. Wear headphones, so that the microphone hears you and not the phrases.",
    ));
    ui.add_space(8.0);
    let busy = singing.is_some() || r.check_sending;
    ui.horizontal(|ui| {
        ui.label(small("Input"));
        ui.add_enabled_ui(!busy, |ui| input(ui, r));
    });
    ui.add_space(10.0);
    meter(ui, r.level);
    ui.add_space(10.0);
    let (mut start, mut stop, mut not_now) = (false, false, false);
    ui.horizontal(|ui| {
        if singing.is_some() {
            stop = widgets::tool_button(ui, Icon::Stop, "Stop", true, true).clicked();
        } else if !r.check_sending {
            start = widgets::tool_button(ui, Icon::Record, "Start the pitch check", true, true).clicked();
            not_now = ui.button(small("Not now")).on_hover_text("leave the pitch check for now").clicked();
        }
    });
    if let (Some((_, t, beat)), Some(c)) = (singing, waiting.as_ref()) {
        let b = t / beat.max(1e-6);
        let phrases = c.answers.len().max(1);
        let say = if b < 0.0 {
            "the count-in: the first phrase is next".to_string()
        } else {
            let k = ((b / 10.0) as usize).min(phrases - 1);
            let doing = if b - k as f64 * 10.0 < 4.5 { "listen" } else { "now sing it back" };
            format!("phrase {} of {phrases}: {doing}", k + 1)
        };
        ui.add_space(6.0);
        ui.label(RichText::new(say).color(TEXT));
    }
    say_now(ui, r);
    r.check |= start;
    if stop {
        finish(cx);
    }
    if not_now && let Some(c) = waiting {
        cx.out.push(claudius_api::Command::CancelPitchCheck { id: c.id });
        cx.view.record.status = Some((true, "the pitch check can wait".into()));
    }
    true
}

/// a pitch check to ask for, and the voice the last one heard
fn offer_check(ui: &mut Ui, cx: &mut Cx) {
    if waiting_check(cx.state).is_some() {
        ui.label(small("A pitch check waits: it comes up once this take is kept or thrown away.").color(TEXT_DIM));
        return;
    }
    ui.horizontal(|ui| {
        let asked = ui
            .button(small("Check my pitch"))
            .on_hover_text("six short phrases to sing back: claudius learns how you sing, and tunes your takes so")
            .clicked();
        if asked {
            cx.out.push(claudius_api::Command::RequestPitchCheck { low: None, high: None, note: String::new() });
        }
        if let Some(v) = &cx.state.voice {
            ui.add(egui::Label::new(small(format!("your voice: {v}")).color(TEXT_DIM)).wrap());
        }
    });
}

/// the pitch check asked for, while it waits to be sung
fn waiting_check(state: &claudius_api::State) -> Option<PitchCheck> {
    state.pitch_check.clone().filter(|c| c.status == TakeStatus::Waiting)
}

/// what a pitch check plays: each note a clear tone with its first harmonics, a 10 ms attack and an 80 ms release
fn guide(c: &PitchCheck) -> crate::audio::Pcm {
    let rate = 48_000u32;
    let beat = 60.0 / c.bpm.max(1.0);
    let n = (c.beats * beat * rate as f64) as usize;
    let mut mono = vec![0f32; n];
    for note in &c.notes {
        let (at, len) = ((note.t * beat * rate as f64) as usize, (note.d * beat * rate as f64) as usize);
        let hz = 440.0 * 2f64.powf((note.p as f64 - 69.0) / 12.0);
        for i in 0..len.min(n.saturating_sub(at)) {
            let s = i as f64 / rate as f64;
            let env = (s / 0.01).min(1.0).min((len - i) as f64 / rate as f64 / 0.08);
            let ph = std::f64::consts::TAU * hz * s;
            mono[at + i] += ((ph.sin() + 0.35 * (2.0 * ph).sin() + 0.15 * (3.0 * ph).sin()) * 0.25 * env) as f32;
        }
    }
    crate::audio::Pcm { rate, channels: 2, samples: mono.iter().flat_map(|&v| [v, v]).collect() }
}

/// starts a pitch check: its phrases play from a bar of count-in, and the input records from the first
fn begin_check(cx: &mut Cx, c: &PitchCheck) {
    use crate::audio::record::{Plan, Recorder};
    let beat = 60.0 / c.bpm.max(1.0);
    let song = Arc::new(guide(c));
    let r = &mut cx.view.record;
    (r.held, r.preview) = (None, None);
    let plan = Plan {
        input: r.settings.input.clone(),
        midi: None,
        channel: r.settings.channel,
        stereo: false,
        song: song.clone(),
        start: 0.0,
        count_in: 4.0 * beat,
        beat,
        bar: 4,
        click: false,
        offset: r.settings.offset_ms as f64 / 1000.0,
    };
    cx.player.pause();
    r.listener = None;
    match Recorder::start(plan) {
        Ok(recorder) => {
            r.live = Some(Live {
                recorder,
                take_start: 0.0,
                song_end: song.seconds(),
                beat,
                song,
                request: None,
                midi: false,
                measuring: false,
                check: Some(c.id),
                stopping: None,
                silent_since: None,
                silent: false,
            });
            (r.status, r.level) = (None, 0.0);
        }
        Err(e) => r.status = Some((false, e)),
    }
}

/// the input and which of its channels
fn input(ui: &mut Ui, r: &mut Record) {
    ui.horizontal(|ui| {
        if r.inputs.is_empty() && r.midi_inputs.is_empty() {
            ui.label(small("no microphone, audio input or MIDI keyboard found").color(theme::BAD));
            return;
        }
        // a keyboard chosen before goes only once the list is all there is: a browser lists its keyboards late, and
        // only once the page may use them
        if crate::audio::record::midi_settled() && r.settings.midi.as_ref().is_some_and(|m| !r.midi_inputs.contains(m))
        {
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
    if let Some(playing) = r.playing_back() {
        return (true, playing);
    }
    match &r.deaf_why {
        Some(why) => (false, why.clone()),
        None if r.quiet => (false, NOTHING_IN.into()),
        None => r.status.clone().unwrap_or_default(),
    }
}

/// whether the input heard nothing at all for a while, in a take or on the meter: the system may be keeping the
/// microphone from claudius
fn hears_nothing(r: &Record) -> bool {
    r.quiet || r.live.as_ref().is_some_and(|l| l.silent)
}

/// What the window says (`what_now`), and, while the input hears nothing at all, a way to the system's settings that
/// let claudius use the microphone, where the system has them.
fn say_now(ui: &mut Ui, r: &mut Record) {
    let (good, text) = what_now(r);
    if text.is_empty() {
        return;
    }
    ui.add_space(6.0);
    let color = if good { TEXT_DIM } else { theme::BAD };
    ui.add(egui::Label::new(small(text).color(color)).wrap());
    if hears_nothing(r)
        && crate::audio::record::privacy_page().is_some()
        && ui.small_button("Open the system's microphone settings").clicked()
        && let Err(e) = crate::audio::record::open_privacy_page()
    {
        r.status = Some((false, e));
    }
}

/// the render a take is recorded over: the player's, in the app and the demo; in a browser on a claudius, fetched for
/// the take (none while it loads)
fn song_audio(cx: &Cx) -> Result<Option<std::sync::Arc<crate::audio::Pcm>>, String> {
    #[cfg(not(target_arch = "wasm32"))]
    return Ok(cx.player.pcm());
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(pcm) = cx.player.pcm() {
            return Ok(Some(pcm));
        }
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
    let request = waiting(cx.state).map(|t| t.id);
    let audio = song_audio(cx);
    if let Some(why) = not_ready(cx, &audio) {
        cx.view.record.status = Some((false, why));
        return;
    }
    let Ok(Some(pcm)) = audio else { return };
    let r = &mut cx.view.record;
    (r.held, r.preview) = (None, None);
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
        song: pcm.clone(),
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
                song: pcm,
                request,
                midi,
                measuring: false,
                check: None,
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
    let (end, song) = (plan.song.seconds(), plan.song.clone());
    match Recorder::start(plan) {
        Ok(recorder) => {
            r.live = Some(Live {
                recorder,
                take_start: 0.0,
                song_end: end,
                beat: 0.5,
                song,
                request: None,
                midi: false,
                measuring: true,
                check: None,
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

/// what to say when the input hears nothing at all, in a take or on the meter: where the system keeps the microphone
/// from apps, how to let claudius use it
#[cfg(all(not(target_arch = "wasm32"), target_os = "macos"))]
const NOTHING_IN: &str = "nothing is coming in: check the input and its channel, and allow the microphone for Claudius \
                          (or for your terminal, when you started claudius from one) in System Settings, Privacy & \
                          Security, Microphone";
#[cfg(all(not(target_arch = "wasm32"), windows))]
const NOTHING_IN: &str = "nothing is coming in: check the input and its channel, and turn on Microphone access and Let \
                          desktop apps access your microphone in Settings, Privacy & security, Microphone";
#[cfg(all(not(target_arch = "wasm32"), not(target_os = "macos"), not(windows)))]
const NOTHING_IN: &str = "nothing is coming in: check the input and its channel, and that the input is not muted in \
                          the system's sound settings";
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

/// the take is all in: a recording that waits to be kept, or the notes played as a part, on their way to claudius
fn done(cx: &mut Cx) {
    use crate::audio::record::Take;
    let still = waiting(cx.state).map(|t| t.id);
    let r = &mut cx.view.record;
    let Some(live) = r.live.take() else { return };
    if let Some(id) = live.check {
        match live.recorder.finish() {
            Ok(Take::Audio { peak: 0.0, .. }) => {
                r.status = Some((false, "nothing came in from the input: check it, and sing the phrases again".into()));
            }
            Ok(Take::Audio { wav, .. }) => {
                r.status = Some((true, "sending your answers to claudius ...".into()));
                (r.sending, r.check_sending, r.check_upload) = (true, true, Some((id, wav)));
            }
            Ok(Take::Notes(_)) => r.status = Some((false, "a pitch check is sung, not played".into())),
            Err(e) => r.status = Some((false, e)),
        }
        return;
    }
    // the request it was started for, while that is still the one waiting
    let asked = live.request.filter(|id| Some(*id) == still);
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
    // how long the take went on, in beats from its section's start
    let length = ((live.recorder.time() - live.take_start) / live.beat).max(0.0);
    match live.recorder.finish() {
        Ok(Take::Audio { peak: 0.0, .. }) => {
            r.status = Some((false, "the take is silent: nothing came in from the input".into()));
        }
        Ok(Take::Audio { wav, secs, dropouts, hushed, .. }) => {
            r.note = take_note(dropouts, hushed);
            let keep = if asked.is_some() { "send it" } else { "keep it" };
            r.status =
                Some((true, format!("{secs:.1} s recorded: play it back, record it again, or {keep}{}", r.note)));
            let (song, start, bar) = (live.song, live.take_start, 4.0 * live.beat);
            r.held = Some(Held { wav, secs, from, song, start, bar, request: asked });
        }
        Ok(Take::Notes(played)) if r.settings.part_role == DRUMS => {
            let lanes: Vec<(String, String)> = cx.state.song.as_ref().map_or_else(Vec::new, |s| {
                let role =
                    |lane: &str| s.tracks.iter().find(|t| t.name == lane).map_or(lane.into(), |t| t.role.clone());
                s.drums.iter().map(|d| (d.lane.clone(), role(&d.lane))).collect()
            });
            let (hits, missing, strange) = to_hits(&played, live.beat, r.settings.grid, &lanes);
            let mut left = Vec::new();
            if !missing.is_empty() {
                left.push(format!("the song has no lane for {}", missing.join(" or ")));
            }
            if strange > 0 {
                left.push(format!("{strange} notes play no drum of the General MIDI map"));
            }
            let left = if left.is_empty() { String::new() } else { format!(" ({})", left.join("; ")) };
            if hits.is_empty() {
                r.status = Some((false, format!("no drum of the song was struck{left}")));
                return;
            }
            r.status = Some((true, format!("sending {} hits to claudius ...{left}", hits.len())));
            (r.sending, r.answers) = (true, None);
            cx.out.push(claudius_api::Command::AddHits {
                from,
                hits,
                bars: None,
                variant: None,
                grid: Some(r.settings.grid),
                start: Some(0.0),
                beats: Some(length),
            });
        }
        Ok(Take::Notes(played)) => {
            let notes = to_beats(&played, live.beat, r.settings.grid);
            r.status = Some((true, format!("sending {} notes to claudius ...", notes.len())));
            (r.sending, r.answers) = (true, asked);
            cx.out.push(claudius_api::Command::AddNotes {
                from,
                notes,
                sounds: vec![r.settings.sound.clone()],
                name,
                role: Some(r.settings.part_role.clone()),
                style: Some(r.settings.style.clone()),
                request: asked,
            });
        }
        Err(e) => r.status = Some((false, e)),
    }
}

/// the take an assistant asked for, while it waits for one
fn waiting(state: &claudius_api::State) -> Option<TakeRequest> {
    state.take_request.clone().filter(|t| t.status == TakeStatus::Waiting)
}

/// plays the take that waits to be kept in the song, from a bar before it; or stops playing it
fn play_back(ctx: &egui::Context, cx: &mut Cx) {
    let r = &mut cx.view.record;
    let Some(held) = &r.held else { return };
    if let Some(p) = r.preview.as_mut().filter(|p| p.is_playing()) {
        p.stop();
        return;
    }
    cx.player.pause();
    if r.preview.is_none() {
        let take = match crate::audio::Pcm::decode_wav(&held.wav) {
            Ok(take) => take,
            Err(e) => return r.status = Some((false, format!("cannot play the take back: {e}"))),
        };
        let mix = crate::audio::take::preview(&held.song, held.start - held.bar, &take, held.start);
        let mut player = crate::audio::Player::new(ctx);
        if let Some(e) = player.error() {
            return r.status = Some((false, format!("cannot play the take back: {e}")));
        }
        player.load(crate::audio::Source::Pcm(Arc::new(mix)), false);
        r.preview = Some(player);
    }
    if let Some(p) = r.preview.as_mut() {
        p.seek(0.0);
        p.play();
    }
}

/// sends the take that waits to be kept to claudius, for the assistant waiting for one if there is one
fn keep(cx: &mut Cx) {
    let still = waiting(cx.state).map(|t| t.id);
    let r = &mut cx.view.record;
    let Some(held) = r.held.take() else { return };
    let asked = held.request.filter(|id| Some(*id) == still);
    let name = Some(r.name.trim().to_string()).filter(|n| !n.is_empty());
    r.preview = None;
    r.status = Some((true, format!("sending {:.1} s to claudius ...{}", held.secs, r.note)));
    (r.sending, r.answers) = (true, asked);
    r.upload = Some(Upload {
        wav: held.wav,
        from: held.from,
        name,
        role: r.settings.role.clone(),
        request: asked,
        file: None,
        found: None,
    });
}

/// tells the assistant waiting for a take that the person will not record it now
fn not_now(cx: &mut Cx) {
    let Some(t) = waiting(cx.state) else { return };
    cx.out.push(claudius_api::Command::CancelTakeRequest { id: t.id });
    cx.view.record.status = Some((true, "the assistant hears that you will not record it now".into()));
}

/// every frame, natively: takes in what the input heard, keeps the meter moving, and stops at the song's end
fn keep_up(ctx: &egui::Context, cx: &mut Cx) {
    let now = cx.now;
    cx.view.record.asked(cx.state.take_request.as_ref());
    cx.view.record.check_asked(cx.state.pitch_check.as_ref());
    if std::mem::take(&mut cx.view.record.check)
        && cx.view.record.live.is_none()
        && !cx.view.record.take_in_hand()
        && let Some(c) = waiting_check(cx.state)
    {
        begin_check(cx, &c);
        return;
    }
    let r = &mut cx.view.record;
    if std::mem::take(&mut r.not_now) {
        not_now(cx);
    }
    let r = &mut cx.view.record;
    if std::mem::take(&mut r.play) && r.live.is_none() {
        play_back(ctx, cx);
    }
    let r = &mut cx.view.record;
    if std::mem::take(&mut r.keep) && r.live.is_none() && !r.sending {
        keep(cx);
    }
    let r = &mut cx.view.record;
    if let Some(p) = r.preview.as_mut() {
        p.tick();
        if !r.open {
            p.pause();
        } else if p.is_playing() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
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
        (r.quiet_since, r.quiet) = (None, false);
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
    let listed = r.settings.midi.as_ref().is_some_and(|m| r.midi_inputs.contains(m));
    let want: Listening =
        (r.settings.input.clone(), r.settings.channel, r.settings.stereo, r.settings.midi.clone(), listed);
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
    let Some((l, on)) = &r.listener else {
        r.level = 0.0;
        return;
    };
    if let Some(e) = l.error() {
        r.deaf = r.listener.take().map(|(_, on)| on);
        r.deaf_why = Some(e);
        return;
    }
    let heard = l.level();
    r.level = (r.level * 0.85).max(heard);
    // a keyboard is quiet between notes; an audio input that hears nothing at all is one the system keeps from claudius
    // (or one muted or unplugged), as a quiet room still hisses
    let now = ctx.input(|i| i.time);
    if on.3.is_none() && heard == 0.0 {
        let since = *r.quiet_since.get_or_insert(now);
        r.quiet = now - since > 2.5;
    } else {
        (r.quiet_since, r.quiet) = (None, false);
    }
    ctx.request_repaint_after(std::time::Duration::from_millis(40));
}

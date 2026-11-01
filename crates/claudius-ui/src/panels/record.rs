//! "Record": a take from a microphone or an audio interface. The song plays from a count-in before the section the
//! take starts with, and what the input hears from that section's first downbeat on goes to claudius, which plays it
//! once from there (`POST /api/takes`).

#[cfg(not(target_arch = "wasm32"))]
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
    /// milliseconds the setup adds that the devices do not report
    pub offset_ms: f32,
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
        }
    }
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
    /// the take is to end now (Space, while it records)
    pub stop: bool,
    /// what to say, and whether it is good news
    status: Option<(bool, String)>,
    /// why the meter cannot hear the input, while that is so
    deaf_why: Option<String>,
    sending: bool,
    level: f32,
    #[cfg(not(target_arch = "wasm32"))]
    inputs: Vec<crate::audio::record::Input>,
    #[cfg(not(target_arch = "wasm32"))]
    live: Option<Live>,
    /// the input the meter shows while no take records, and what it is listening to; or what it could not listen to
    #[cfg(not(target_arch = "wasm32"))]
    listener: Option<(crate::audio::record::Listener, Listening)>,
    #[cfg(not(target_arch = "wasm32"))]
    deaf: Option<Listening>,
}

/// an input and the channels of it
#[cfg(not(target_arch = "wasm32"))]
type Listening = (Option<String>, usize, bool);

/// a take being recorded
#[cfg(not(target_arch = "wasm32"))]
struct Live {
    recorder: crate::audio::record::Recorder,
    /// where in the song the take starts and the song ends, in seconds
    take_start: f64,
    song_end: f64,
    /// since when the input has heard nothing at all, and whether for long enough to say so
    silent_since: Option<f64>,
    silent: bool,
}

impl Record {
    /// what claudius answered to the take
    pub fn answered(&mut self, ok: bool, message: String) {
        self.sending = false;
        if ok {
            self.name.clear();
        }
        self.status = Some((ok, message));
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Record {
    pub fn show(&mut self) {
        self.open = true;
        self.inputs = crate::audio::record::inputs();
    }

    pub fn recording(&self) -> bool {
        self.live.is_some()
    }
}

#[cfg(target_arch = "wasm32")]
impl Record {
    pub fn recording(&self) -> bool {
        false
    }
}

pub fn dialog(ctx: &egui::Context, cx: &mut Cx) {
    #[cfg(not(target_arch = "wasm32"))]
    keep_up(ctx, cx);
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
}

fn body(ui: &mut Ui, cx: &mut Cx) {
    let Some(song) = cx.state.song.clone() else {
        ui.label(RichText::new("Make a song first.").color(TEXT));
        return;
    };
    let selected = song.sections.get(cx.view.section).map(|s| s.name.clone());
    let has_audio = cx.player.has_audio();
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
        ui.add_enabled_ui(!busy, |ui| {
            ui.horizontal(|ui| {
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
                ui.add(
                    egui::TextEdit::singleline(&mut r.name).hint_text("named take1, take2 ...").desired_width(150.0),
                )
                .on_hover_text("the track's name: letters, digits, - and _");
            });
        });
        ui.end_row();
        ui.label(small("Latency"));
        ui.add_enabled_ui(!busy, |ui| {
            ui.add(
                egui::DragValue::new(&mut r.settings.offset_ms)
                    .range(-200.0..=500.0)
                    .speed(1.0)
                    .custom_formatter(|v, _| format!("{v:+.0} ms")),
            )
            .on_hover_text(
                "what your setup adds that the devices do not report, such as Bluetooth headphones: a take that \
                 lands late wants more",
            );
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
            let why = if r.sending { "the last take is still on its way" } else { "render the song first" };
            start = widgets::tool_button(ui, Icon::Record, "Record", has_audio && !r.sending, true)
                .on_disabled_hover_text(why)
                .clicked();
        }
        let (good, text) = what_now(r);
        if !text.is_empty() {
            let color = if good { TEXT_DIM } else { theme::BAD };
            ui.add(egui::Label::new(small(text).color(color)).wrap());
        }
    });
    #[cfg(not(target_arch = "wasm32"))]
    {
        if start {
            begin(cx, &song);
        }
        if stop {
            finish(cx);
        }
    }
    #[cfg(target_arch = "wasm32")]
    let _ = (start, stop);
}

/// the input and which of its channels
#[cfg(not(target_arch = "wasm32"))]
fn input(ui: &mut Ui, r: &mut Record) {
    ui.horizontal(|ui| {
        if r.inputs.is_empty() {
            ui.label(small("no microphone or audio input found").color(theme::BAD));
            return;
        }
        let name = r.settings.input.clone().filter(|n| r.inputs.iter().any(|i| i.name == *n));
        let shown = name.clone().unwrap_or_else(|| r.inputs[0].name.clone());
        egui::ComboBox::from_id_salt("record-input").selected_text(&shown).width(190.0).show_ui(ui, |ui| {
            for i in &r.inputs {
                ui.selectable_value(&mut r.settings.input, Some(i.name.clone()), &i.name);
            }
        });
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

#[cfg(target_arch = "wasm32")]
fn input(ui: &mut Ui, _r: &mut Record) {
    ui.label(small("recording needs the studio app"));
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
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(live) = &r.live {
        let t = live.recorder.time();
        let from = r.from.as_deref().unwrap_or("");
        if t < live.take_start {
            return (true, format!("count-in: {:.1} s to {from}", live.take_start - t));
        }
        if live.silent {
            return (
                false,
                "nothing is coming in: check the input and its channel; on a Mac, allow the microphone for \
                 Claudius (or your terminal) in System Settings, Privacy & Security, Microphone"
                    .into(),
            );
        }
        return (true, format!("recording from {from}: {}", crate::mock::fmt_time(t - live.take_start)));
    }
    match &r.deaf_why {
        Some(why) => (false, why.clone()),
        None => r.status.clone().unwrap_or_default(),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn begin(cx: &mut Cx, song: &SongView) {
    use crate::audio::record::{Plan, Recorder};
    let beat = 60.0 / cx.audio_bpm().max(1.0);
    let Some(pcm) = cx.player.pcm() else {
        cx.view.record.status = Some((false, "render the song first".into()));
        return;
    };
    let r = &mut cx.view.record;
    let Some(section) = r.from.as_ref().and_then(|f| song.sections.iter().find(|s| s.name == *f)) else {
        r.status = Some((false, "choose the section the take starts with".into()));
        return;
    };
    let (start, end) = (section.start_bar as f64 * 4.0 * beat, pcm.seconds());
    let plan = Plan {
        input: r.settings.input.clone(),
        channel: r.settings.channel,
        stereo: r.settings.stereo,
        song: pcm,
        start,
        count_in: r.settings.count_in as f64 * 4.0 * beat,
        beat,
        bar: 4,
        click: r.settings.click,
        offset: r.settings.offset_ms as f64 / 1000.0,
    };
    cx.player.pause();
    // the recorder opens the input itself: some systems give a device to one stream at a time
    r.listener = None;
    match Recorder::start(plan) {
        Ok(recorder) => {
            r.live = Some(Live { recorder, take_start: start, song_end: end, silent_since: None, silent: false });
            (r.status, r.level) = (None, 0.0);
        }
        Err(e) => r.status = Some((false, e)),
    }
}

/// stops the take and sends it
#[cfg(not(target_arch = "wasm32"))]
pub fn finish(cx: &mut Cx) {
    let r = &mut cx.view.record;
    let Some(live) = r.live.take() else { return };
    match live.recorder.finish() {
        Ok(take) if take.peak == 0.0 => {
            r.status = Some((false, "the take is silent: nothing came in from the input".into()));
        }
        Ok(take) => {
            let name = Some(r.name.trim().to_string()).filter(|n| !n.is_empty());
            let dropped = match take.dropouts {
                0 => String::new(),
                n => format!(
                    " (it dropped audio {n} time{}: other apps may be busy with the audio)",
                    if n == 1 { "" } else { "s" }
                ),
            };
            r.status = Some((true, format!("sending {:.1} s to claudius ...{dropped}", take.secs)));
            r.sending = true;
            r.upload = Some(Upload {
                wav: take.wav,
                from: r.from.clone().unwrap_or_default(),
                name,
                role: r.settings.role.clone(),
            });
        }
        Err(e) => r.status = Some((false, e)),
    }
}

/// every frame, natively: takes in what the input heard, keeps the meter moving, and stops at the song's end
#[cfg(not(target_arch = "wasm32"))]
fn keep_up(ctx: &egui::Context, cx: &mut Cx) {
    let now = cx.now;
    let r = &mut cx.view.record;
    if std::mem::take(&mut r.stop) && r.live.is_some() {
        finish(cx);
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
    if t >= live.take_start && heard == 0.0 {
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
#[cfg(not(target_arch = "wasm32"))]
fn listen(ctx: &egui::Context, r: &mut Record) {
    if !r.open {
        (r.listener, r.deaf, r.deaf_why, r.level) = (None, None, None, 0.0);
        return;
    }
    // a channel remembered from another input may be one this input does not have
    let shown = r.settings.input.clone().filter(|n| r.inputs.iter().any(|i| i.name == *n));
    let channels = shown.as_ref().map_or_else(|| r.inputs.first(), |n| r.inputs.iter().find(|i| i.name == *n));
    if let Some(i) = channels
        && r.settings.channel + usize::from(r.settings.stereo) >= i.channels as usize
    {
        (r.settings.channel, r.settings.stereo) = (0, false);
    }
    let want: Listening = (r.settings.input.clone(), r.settings.channel, r.settings.stereo);
    if r.listener.as_ref().is_none_or(|(_, on)| *on != want) && r.deaf.as_ref() != Some(&want) {
        r.listener = None;
        match crate::audio::record::Listener::start(want.0.as_deref(), want.1, want.2) {
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

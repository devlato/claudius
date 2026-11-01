//! A scripted run for checking the studio without a person at the screen: env `CLAUDIUS_UI_PLAN` is a `;`-separated
//! list of steps, e.g. `wait:4;shot:/tmp/a.png;tab:mixer;play;wait:2;shot:/tmp/b.png;exit`. A web build with the
//! `plan` feature (for tests only) takes the page's `?plan=` instead, and leaves out screenshots and pointer input.
//! What the Record window says goes to stderr (the browser's console), as `record: ...`, what it offers for MIDI
//! keyboards as `midi: ...`, the last analysis as `analysis: ...`, each toast as `toast: ...`, and the graph's node
//! chosen as `graph: ...`.
//! Steps: `wait:<s>`, `tab:<sequencer|mixer|parts|harmony|script|sounds|graph|log>`, `section:<i>`, `play`, `pause`, `seek:<s>`,
//! `newsong`, `close`, `tour[:<step>]`, `assistants`, `publish`, `record` (the Record window),
//! `rec:<start|stop|measure>` (a take, or the latency), `rec:<play|keep|not-now>` (the take back, kept or sent; the
//! take an assistant asked for turned down), `rec:allow-midi`, `rec:midi:<input>` (a MIDI input chosen),
//! `rec:grid:<beats>` (its notes' grid, 0 as played), `pick:<song|audio|cover>` (a file dialog, as its button opens
//! one), `listen:<analyze|remake>` (as the New song dialog's buttons), `audiopath:<file>` (a recording read as if
//! chosen for Analyze, in the app), `cmd:<command json>`, `size:<w>,<h>`, `script:<code; \n for a newline, %3B for ;>`
//! (into the editor), `run`, `reference`, `shot:<file.png>` (the window as rendered), `exit`, and synthetic pointer
//! input in points: `move:x,y`, `click:x,y`, `rclick:x,y`, `drag:x1,y1,x2,y2`, `sdrag:x1,y1,x2,y2` (with shift).

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use claudius_api::Command;
use eframe::egui;

pub enum Step {
    Tab(String),
    Section(usize),
    Play,
    Pause,
    Seek(f64),
    NewSong,
    Close,
    Send(Command),
    Size(f32, f32),
    Script(String),
    AudioPath(String),
    /// a file dialog, as the button for what it is for opens one
    Pick(crate::pick::Want),
    /// the chosen recording sent to be listened to (false) or remade (true), as the New song dialog's buttons do
    Listen(bool),
    Run,
    Reference,
    /// the tour, at this step
    Tour(usize),
    /// the Connect an assistant dialog
    Assistants,
    /// the Publish dialog
    Publish,
    /// the Record window
    Record,
    /// what the Record window's buttons do
    Rec(Rec),
    Exit,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Rec {
    /// a take starts, as the Record button starts one
    Start,
    /// it ends, as Space ends one
    Stop,
    /// the latency is measured, as the Measure button measures it
    Measure,
    /// the MIDI input of that name is chosen
    Midi(String),
    /// a MIDI take's notes go to a grid of this many beats (0: as played)
    Grid(f64),
    /// the page asks the browser for MIDI keyboards, as choosing "let the page use them" does
    AllowMidi,
    /// the take that waits to be kept plays back, as Play it back plays it
    Play,
    /// it is kept (or sent to the assistant that asked for it), as Keep it keeps it
    Keep,
    /// the take an assistant asked for is turned down, as Not now turns it down
    NotNow,
}

pub struct Plan {
    steps: VecDeque<String>,
    until: f64,
    /// what was last told of each thing it tells
    told: std::collections::HashMap<&'static str, String>,
    /// the screenshot being taken: done flag, file, when it was asked for
    shot: Option<(Arc<AtomicBool>, String, f64)>,
    /// pointer events to feed in, one batch per frame
    input: VecDeque<(Vec<egui::Event>, egui::Modifiers)>,
}

/// a line for whoever runs the plan: stderr, or the browser's console
pub fn say(line: &str) {
    #[cfg(not(target_arch = "wasm32"))]
    eprintln!("{line}");
    #[cfg(target_arch = "wasm32")]
    web_sys::console::log_1(&line.into());
}

impl Plan {
    pub fn from_env() -> Option<Plan> {
        #[cfg(not(target_arch = "wasm32"))]
        let plan = std::env::var("CLAUDIUS_UI_PLAN").ok()?;
        #[cfg(target_arch = "wasm32")]
        let plan =
            web_sys::UrlSearchParams::new_with_str(&web_sys::window()?.location().search().ok()?).ok()?.get("plan")?;
        Some(Plan {
            steps: plan.split(';').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
            until: 0.0,
            told: Default::default(),
            shot: None,
            input: VecDeque::new(),
        })
    }

    /// says `what` of `thing` when it is not what was said of it last
    pub fn tell(&mut self, thing: &'static str, what: &str) {
        if self.told.get(thing).is_none_or(|t| t != what) {
            if !what.is_empty() {
                say(&format!("{thing}: {what}"));
            }
            self.told.insert(thing, what.to_string());
        }
    }

    /// feeds the next batch of synthetic input (call from `App::raw_input_hook`)
    #[cfg(not(target_arch = "wasm32"))]
    pub fn feed(&mut self, raw: &mut egui::RawInput) {
        if let Some((events, modifiers)) = self.input.pop_front() {
            raw.events.push(egui::Event::ModifiersChanged(modifiers));
            raw.events.extend(events);
        }
    }

    fn pointer(&mut self, button: egui::PointerButton, from: egui::Pos2, to: egui::Pos2, modifiers: egui::Modifiers) {
        use egui::Event::{PointerButton, PointerMoved};
        let press = |pos, pressed| PointerButton { pos, button, pressed, modifiers };
        self.input.push_back((vec![PointerMoved(from)], modifiers));
        self.input.push_back((vec![press(from, true)], modifiers));
        for i in 1..=8 {
            self.input.push_back((vec![PointerMoved(from + (to - from) * (i as f32 / 8.0))], modifiers));
        }
        self.input.push_back((vec![press(to, false)], modifiers));
        self.input.push_back((Vec::new(), egui::Modifiers::NONE));
    }

    #[cfg(target_arch = "wasm32")]
    fn shoot(&mut self, _ctx: &egui::Context, path: String, _now: f64) {
        say(&format!("no screenshot {path}: screenshots are the app's"));
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn shoot(&mut self, ctx: &egui::Context, path: String, now: f64) {
        let done = Arc::new(AtomicBool::new(false));
        self.shot = Some((done.clone(), path.clone(), now));
        ctx.request_screenshot(move |image| {
            if done.swap(true, Ordering::Relaxed) {
                return;
            }
            match std::fs::write(&path, png(&image)) {
                Ok(()) => eprintln!("screenshot {path} ({}x{})", image.size[0], image.size[1]),
                Err(e) => eprintln!("could not write {path}: {e}"),
            }
        });
        ctx.request_repaint();
    }

    /// the next step to apply now, if any (screenshots and waits are handled here)
    pub fn next(&mut self, ctx: &egui::Context, now: f64) -> Option<Step> {
        ctx.request_repaint_after(std::time::Duration::from_millis(50));
        if let Some((done, path, at)) = &self.shot {
            if !done.load(Ordering::Relaxed) {
                // a request can get lost with a discarded frame: ask again
                if now - at > 3.0 {
                    let path = path.clone();
                    self.shoot(ctx, path, now);
                }
                return None;
            }
            self.shot = None;
        }
        if now < self.until || !self.input.is_empty() {
            return None;
        }
        let step = self.steps.pop_front()?;
        let (verb, arg) = step.split_once(':').unwrap_or((step.as_str(), ""));
        match verb {
            "wait" => {
                self.until = now + arg.parse::<f64>().unwrap_or(1.0);
                None
            }
            "shot" => {
                self.shoot(ctx, arg.to_string(), now);
                None
            }
            "tab" => Some(Step::Tab(arg.to_string())),
            "section" => Some(Step::Section(arg.parse().unwrap_or(0))),
            "play" => Some(Step::Play),
            "pause" => Some(Step::Pause),
            "seek" => Some(Step::Seek(arg.parse().unwrap_or(0.0))),
            "newsong" => Some(Step::NewSong),
            "tour" => Some(Step::Tour(arg.parse().unwrap_or(0))),
            "assistants" => Some(Step::Assistants),
            "publish" => Some(Step::Publish),
            "record" => Some(Step::Record),
            "rec" => match arg {
                "start" => Some(Step::Rec(Rec::Start)),
                "stop" => Some(Step::Rec(Rec::Stop)),
                "measure" => Some(Step::Rec(Rec::Measure)),
                "allow-midi" => Some(Step::Rec(Rec::AllowMidi)),
                "play" => Some(Step::Rec(Rec::Play)),
                "keep" => Some(Step::Rec(Rec::Keep)),
                "not-now" => Some(Step::Rec(Rec::NotNow)),
                other => match (other.strip_prefix("midi:"), other.strip_prefix("grid:").map(str::parse)) {
                    (Some(name), _) => Some(Step::Rec(Rec::Midi(name.to_string()))),
                    (_, Some(Ok(beats))) => Some(Step::Rec(Rec::Grid(beats))),
                    _ => {
                        say(&format!(
                            "a plan's rec is rec:start, rec:stop, rec:measure, rec:play, rec:keep, rec:not-now, \
                             rec:allow-midi, rec:midi:<input> or rec:grid:<beats>, not '{step}'"
                        ));
                        None
                    }
                },
            },
            "close" => Some(Step::Close),
            "cmd" => match serde_json::from_str(arg) {
                Ok(c) => Some(Step::Send(c)),
                Err(e) => {
                    say(&format!("bad command in plan: {e}"));
                    None
                }
            },
            "click" | "rclick" | "drag" | "sdrag" | "move" if cfg!(target_arch = "wasm32") => {
                say(&format!("'{step}' skipped: pointer input is the app's"));
                None
            }
            "click" | "rclick" | "drag" | "sdrag" => {
                let n: Vec<f32> = arg.split(',').filter_map(|v| v.trim().parse().ok()).collect();
                let (from, to) = match n.as_slice() {
                    [x, y] => (egui::pos2(*x, *y), egui::pos2(*x, *y)),
                    [x1, y1, x2, y2] => (egui::pos2(*x1, *y1), egui::pos2(*x2, *y2)),
                    _ => {
                        say(&format!("bad coordinates in '{step}'"));
                        return None;
                    }
                };
                let button =
                    if verb == "rclick" { egui::PointerButton::Secondary } else { egui::PointerButton::Primary };
                let modifiers = if verb == "sdrag" { egui::Modifiers::SHIFT } else { egui::Modifiers::NONE };
                self.pointer(button, from, to, modifiers);
                None
            }
            "script" => Some(Step::Script(arg.replace("\\n", "\n").replace("%3B", ";"))),
            "run" => Some(Step::Run),
            "audiopath" => Some(Step::AudioPath(arg.to_string())),
            "pick" => match arg {
                "song" => Some(Step::Pick(crate::pick::Want::Song)),
                "audio" => Some(Step::Pick(crate::pick::Want::Audio)),
                "cover" => Some(Step::Pick(crate::pick::Want::Cover)),
                _ => {
                    say(&format!("a plan's pick is pick:song, pick:audio or pick:cover, not '{step}'"));
                    None
                }
            },
            "listen" => Some(Step::Listen(arg == "remake")),
            "reference" => Some(Step::Reference),
            "move" => {
                let n: Vec<f32> = arg.split(',').filter_map(|v| v.trim().parse().ok()).collect();
                if let [x, y] = n.as_slice() {
                    self.input.push_back((vec![egui::Event::PointerMoved(egui::pos2(*x, *y))], egui::Modifiers::NONE));
                }
                None
            }
            "size" => {
                let (w, h) = arg.split_once(',')?;
                Some(Step::Size(w.trim().parse().ok()?, h.trim().parse().ok()?))
            }
            "exit" => Some(Step::Exit),
            other => {
                say(&format!("unknown plan step '{other}'"));
                None
            }
        }
    }
}

/// an uncompressed RGBA PNG
#[cfg(not(target_arch = "wasm32"))]
fn png(image: &egui::ColorImage) -> Vec<u8> {
    fn crc32(data: &[u8]) -> u32 {
        let mut crc = !0u32;
        for &b in data {
            crc ^= b as u32;
            for _ in 0..8 {
                crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
            }
        }
        !crc
    }
    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut c = kind.to_vec();
        c.extend_from_slice(data);
        out.extend_from_slice(&c);
        out.extend_from_slice(&crc32(&c).to_be_bytes());
    }
    let [w, h] = image.size;
    let mut raw = Vec::with_capacity((w * 4 + 1) * h);
    for row in image.pixels.chunks(w) {
        raw.push(0);
        for p in row {
            raw.extend_from_slice(&[p.r(), p.g(), p.b(), 255]);
        }
    }
    let mut z = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = raw.chunks(65_535).collect();
    for (i, block) in blocks.iter().enumerate() {
        z.push(u8::from(i + 1 == blocks.len()));
        let len = block.len() as u16;
        z.extend_from_slice(&len.to_le_bytes());
        z.extend_from_slice(&(!len).to_le_bytes());
        z.extend_from_slice(block);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &x in &raw {
        a = (a + x as u32) % 65_521;
        b = (b + a) % 65_521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());
    let mut out = vec![0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1A, b'\n'];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(w as u32).to_be_bytes());
    ihdr.extend_from_slice(&(h as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}

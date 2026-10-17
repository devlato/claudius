//! A scripted run for checking the studio without a person at the screen (native only): env `CLAUDIUS_UI_PLAN` is a
//! `;`-separated list of steps, e.g. `wait:4;shot:/tmp/a.png;tab:mixer;play;wait:2;shot:/tmp/b.png;exit`.
//! Steps: `wait:<s>`, `tab:<sequencer|mixer|parts|harmony|script|log>`, `section:<i>`, `play`, `pause`, `seek:<s>`,
//! `newsong`, `close`, `cmd:<command json>`, `size:<w>,<h>`, `script:<code; \n for a newline, %3B for ;>` (into the editor), `run`, `audiopath:<file>`
//! (the editor's script), `reference`, `shot:<file.png>` (the window as rendered), `exit`, and synthetic pointer input in
//! points: `move:x,y`, `click:x,y`, `rclick:x,y`, `drag:x1,y1,x2,y2`, `sdrag:x1,y1,x2,y2` (with shift).

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
    Run,
    Reference,
    Exit,
}

pub struct Plan {
    steps: VecDeque<String>,
    until: f64,
    /// the screenshot being taken: done flag, file, when it was asked for
    shot: Option<(Arc<AtomicBool>, String, f64)>,
    /// pointer events to feed in, one batch per frame
    input: VecDeque<(Vec<egui::Event>, egui::Modifiers)>,
}

impl Plan {
    pub fn from_env() -> Option<Plan> {
        let plan = std::env::var("CLAUDIUS_UI_PLAN").ok()?;
        Some(Plan {
            steps: plan.split(';').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
            until: 0.0,
            shot: None,
            input: VecDeque::new(),
        })
    }

    /// feeds the next batch of synthetic input (call from `App::raw_input_hook`)
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
            "close" => Some(Step::Close),
            "cmd" => match serde_json::from_str(arg) {
                Ok(c) => Some(Step::Send(c)),
                Err(e) => {
                    eprintln!("bad command in plan: {e}");
                    None
                }
            },
            "click" | "rclick" | "drag" | "sdrag" => {
                let n: Vec<f32> = arg.split(',').filter_map(|v| v.trim().parse().ok()).collect();
                let (from, to) = match n.as_slice() {
                    [x, y] => (egui::pos2(*x, *y), egui::pos2(*x, *y)),
                    [x1, y1, x2, y2] => (egui::pos2(*x1, *y1), egui::pos2(*x2, *y2)),
                    _ => {
                        eprintln!("bad coordinates in '{step}'");
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
                eprintln!("unknown plan step '{other}'");
                None
            }
        }
    }
}

/// an uncompressed RGBA PNG
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

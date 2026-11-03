//! The demo's sounds: claudius' synth presets and two drum machines, rendered by claudius itself
//! (`crates/claudius/examples/demo_sounds.rs`) and served beside the web studio as `sounds/kit.json` and Ogg Vorbis
//! files. They arrive while the demo runs: the manifest first, then the files a song plays, each decoded here.
//! Natively, `CLAUDIUS_DEMO_SOUNDS` names their folder or address; without them, the demo plays its own synth.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use eframe::egui;
use serde::Deserialize;

#[derive(Deserialize)]
struct Manifest {
    rate: u32,
    #[serde(default)]
    credits: Vec<String>,
    sounds: HashMap<String, Spec>,
}

#[derive(Clone, Deserialize)]
struct Spec {
    pitched: bool,
    release: f32,
    samples: Vec<FileSpec>,
}

#[derive(Clone, Deserialize)]
struct FileSpec {
    note: i32,
    file: String,
    secs: f32,
    energy: Vec<f32>,
}

/// the times a clip's energy is known at (the generator's `TIMES`)
pub const TIMES: [f32; 9] = [0.0625, 0.125, 0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0];

/// a decoded file, at the demo's rate; `r` is empty for a mono one
pub struct Clip {
    pub l: Vec<f32>,
    pub r: Vec<f32>,
    pub secs: f32,
    /// K-weighted energy from its start to each of `TIMES`
    pub energy: Vec<f32>,
    /// a sampled loop's K-weighted power while it plays (0 for the kit's own files)
    pub power: f32,
    /// a sampled loop's length in beats (0 for a texture and the kit's own files)
    pub beats: f32,
}

impl Clip {
    /// its K-weighted energy over its first `t` seconds (all of it past its end)
    pub fn energy_until(&self, t: f32) -> f32 {
        // the generator takes each time's energy up to the clip's end at most: the last is all of it
        if t >= self.secs {
            return self.energy.last().copied().unwrap_or(0.0);
        }
        let mut before = (0.0, 0.0);
        for (&at, &e) in TIMES.iter().zip(&self.energy) {
            if t <= at {
                return before.1 + (e - before.1) * (t - before.0) / (at - before.0);
            }
            before = (at, e);
        }
        before.1
    }

    pub fn len(&self) -> usize {
        self.l.len()
    }

    /// the clip between its samples, `pos` in samples: linear, which at most three semitones from a sample's note
    /// keeps its highs, and costs the browser little
    pub fn linear(&self, pos: f32) -> (f32, f32) {
        let i = pos as usize;
        let t = pos - i as f32;
        let read = |x: &[f32]| {
            let a = x.get(i).copied().unwrap_or(0.0);
            a + (x.get(i + 1).copied().unwrap_or(0.0) - a) * t
        };
        let l = read(&self.l);
        (l, if self.r.is_empty() { l } else { read(&self.r) })
    }
}

/// a sound as a render plays it: the files of it that have arrived, by the note each plays
#[derive(Clone)]
pub struct Sound {
    pub pitched: bool,
    pub release: f32,
    pub clips: Vec<(i32, Arc<Clip>)>,
}

impl Sound {
    /// the clip nearest `note`, and how fast to read it to play that note
    pub fn pick(&self, note: f32) -> Option<(&Clip, f32)> {
        if !self.pitched {
            return self.clips.first().map(|(_, c)| (c.as_ref(), 1.0));
        }
        let (n, c) =
            self.clips.iter().min_by(|a, b| (a.0 as f32 - note).abs().total_cmp(&(b.0 as f32 - note).abs()))?;
        Some((c.as_ref(), 2f32.powf((note - *n as f32) / 12.0)))
    }
}

/// what a song plays of a sound: its name, and for a pitched one the notes it reaches
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Need {
    pub sound: String,
    pub low: i32,
    pub high: i32,
}

enum Arrival {
    Manifest(Result<Manifest, String>),
    File(String, Result<Clip, String>),
}

pub struct Kit {
    /// a folder, or an address (relative to the page on the web)
    base: Option<String>,
    /// the samples songs play (`sampled:` and their tracks' sounds), as the sampler cut them
    sampled: HashMap<String, Sound>,
    manifest: Option<Manifest>,
    /// there is no kit, or it could not be read: the demo plays its synth
    gone: bool,
    clips: HashMap<String, Arc<Clip>>,
    asked: HashSet<String>,
    failed: HashSet<String>,
    tx: Sender<Arrival>,
    rx: Receiver<Arrival>,
    ctx: Option<egui::Context>,
    /// whom the sounds are by, once the manifest says (taken once, for the log)
    pub credits: Option<Vec<String>>,
}

/// a sound for `Kit::with`: its name, whether it is pitched, its release, and a clip for each of its notes
#[cfg(test)]
pub type TestSound<'a> = (&'a str, bool, f32, Vec<(i32, Clip)>);

/// at this rate the demo renders, and the kit is made
const RATE: u32 = super::synth::RATE;

fn default_base() -> Option<String> {
    if cfg!(target_arch = "wasm32") {
        Some("sounds".into())
    } else {
        std::env::var("CLAUDIUS_DEMO_SOUNDS").ok().filter(|s| !s.trim().is_empty())
    }
}

impl Kit {
    pub fn new(ctx: Option<egui::Context>) -> Kit {
        Kit::at(default_base(), ctx)
    }

    /// the kit in the folder or at the address `base` (none: the demo plays its synth)
    pub fn at(base: Option<String>, ctx: Option<egui::Context>) -> Kit {
        let (tx, rx) = channel();
        let kit = Kit {
            gone: base.is_none(),
            base,
            sampled: HashMap::new(),
            manifest: None,
            clips: HashMap::new(),
            asked: HashSet::new(),
            failed: HashSet::new(),
            tx,
            rx,
            ctx,
            credits: None,
        };
        kit.fetch("kit.json".into(), |bytes| {
            Arrival::Manifest(
                bytes.and_then(|b| serde_json::from_slice::<Manifest>(&b).map_err(|e| format!("kit.json: {e}"))),
            )
        });
        kit
    }

    /// a kit of these sounds, every file of them arrived
    #[cfg(test)]
    pub fn with(sounds: Vec<TestSound>) -> Kit {
        let mut kit = Kit::at(None, None);
        let mut specs = HashMap::new();
        for (name, pitched, release, clips) in sounds {
            let mut samples = Vec::new();
            for (k, (note, clip)) in clips.into_iter().enumerate() {
                let file = format!("{name}-{k}.ogg");
                samples.push(FileSpec { note, file: file.clone(), secs: clip.secs, energy: clip.energy.clone() });
                kit.clips.insert(file, Arc::new(clip));
            }
            specs.insert(name.to_string(), Spec { pitched, release, samples });
        }
        kit.manifest = Some(Manifest { rate: RATE, credits: Vec::new(), sounds: specs });
        kit.gone = false;
        kit
    }

    /// a kit of these sounds whose files are all still on their way; `arrive` delivers one
    #[cfg(test)]
    pub fn pending(sounds: Vec<TestSound>) -> Kit {
        let mut kit = Kit::with(sounds);
        kit.clips.clear();
        kit
    }

    /// a file of `Kit::with` or `Kit::pending` (`{sound}-{n}.ogg`, its n-th note) arriving, or failing to
    #[cfg(test)]
    pub fn arrive(&self, file: &str, clip: Result<Clip, String>) {
        let _ = self.tx.send(Arrival::File(file.to_string(), clip));
    }

    fn fetch(&self, name: String, done: impl FnOnce(Result<Vec<u8>, String>) -> Arrival + Send + 'static) {
        let Some(base) = &self.base else { return };
        let path = format!("{}/{name}", base.trim_end_matches('/'));
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        let deliver = move |a: Arrival| {
            let _ = tx.send(a);
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        };
        #[cfg(not(target_arch = "wasm32"))]
        if !base.contains("://") {
            deliver(done(std::fs::read(&path).map_err(|e| format!("{path}: {e}"))));
            return;
        }
        ehttp::fetch(ehttp::Request::get(&path), move |res| {
            let bytes = match res {
                Ok(r) if r.ok => Ok(r.bytes),
                Ok(r) => Err(format!("{path}: {} {}", r.status, r.status_text)),
                Err(e) => Err(format!("{path}: {e}")),
            };
            deliver(done(bytes));
        });
    }

    /// takes what has arrived; true when something new can be played
    pub fn poll(&mut self) -> bool {
        let mut new = false;
        while let Ok(a) = self.rx.try_recv() {
            match a {
                Arrival::Manifest(Ok(m)) if m.rate == RATE => {
                    self.credits = Some(m.credits.clone());
                    self.manifest = Some(m);
                    new = true;
                }
                Arrival::Manifest(Ok(m)) => {
                    log::warn!("the demo's sounds are at {} Hz, not {RATE}: the demo plays its synth", m.rate);
                    self.gone = true;
                }
                Arrival::Manifest(Err(e)) => {
                    log::info!("no demo sounds ({e}): the demo plays its synth");
                    self.gone = true;
                }
                Arrival::File(name, Ok(clip)) => {
                    self.clips.insert(name, Arc::new(clip));
                    new = true;
                }
                Arrival::File(name, Err(e)) => {
                    log::warn!("demo sound {name}: {e}");
                    self.failed.insert(name);
                }
            }
        }
        new
    }

    pub fn gone(&self) -> bool {
        self.gone
    }

    /// the files of a sound a need plays: every file of a drum or effect, and a pitched sound's within half an
    /// octave of the notes it reaches
    fn files<'a>(&'a self, need: &Need) -> Vec<&'a FileSpec> {
        let Some(spec) = self.manifest.as_ref().and_then(|m| m.sounds.get(&need.sound)) else { return Vec::new() };
        if !spec.pitched {
            return spec.samples.iter().collect();
        }
        let near: Vec<_> = spec.samples.iter().filter(|s| s.note >= need.low - 6 && s.note <= need.high + 6).collect();
        if !near.is_empty() {
            return near;
        }
        let middle = (need.low + need.high) / 2;
        spec.samples.iter().min_by_key(|s| (s.note - middle).abs()).into_iter().collect()
    }

    /// starts fetching what `needs` plays that has not been asked for
    pub fn want(&mut self, needs: &[Need]) {
        let wanted: Vec<FileSpec> =
            needs.iter().flat_map(|n| self.files(n)).filter(|s| !self.asked.contains(&s.file)).cloned().collect();
        for f in wanted {
            if !self.asked.insert(f.file.clone()) {
                continue;
            }
            let FileSpec { file, secs, energy, .. } = f;
            let name = file.clone();
            self.fetch(file, move |bytes| {
                Arrival::File(
                    name,
                    bytes.and_then(decode).map(|(l, r)| Clip { l, r, secs, energy, power: 0.0, beats: 0.0 }),
                )
            });
        }
    }

    /// files of `needs` still on their way (or the manifest)
    pub fn waiting(&self, needs: &[Need]) -> bool {
        if self.gone {
            return false;
        }
        if self.manifest.is_none() {
            return true;
        }
        needs
            .iter()
            .flat_map(|n| self.files(n))
            .any(|s| !self.clips.contains_key(&s.file) && !self.failed.contains(&s.file))
    }

    /// the sound as far as it has arrived, for the notes a need reaches
    pub fn sound(&self, need: &Need) -> Option<Sound> {
        if let Some(s) = self.sampled.get(&need.sound) {
            return Some(s.clone());
        }
        let spec = self.manifest.as_ref()?.sounds.get(&need.sound)?;
        let clips: Vec<(i32, Arc<Clip>)> =
            self.files(need).iter().filter_map(|s| Some((s.note, self.clips.get(&s.file)?.clone()))).collect();
        (!clips.is_empty()).then_some(Sound { pitched: spec.pitched, release: spec.release, clips })
    }

    /// whether the kit has `sound` at all
    pub fn has(&self, sound: &str) -> bool {
        self.sampled.contains_key(sound) || self.manifest.as_ref().is_some_and(|m| m.sounds.contains_key(sound))
    }

    /// a sample a loop track plays, by the track's sound
    pub fn sample(&mut self, track_sound: &str, clip: Arc<Clip>) {
        let sound = Sound { pitched: false, release: 0.005, clips: vec![(60, clip)] };
        self.sampled.insert(sampled(track_sound), sound);
    }

    /// lets go of the samples no track of these sounds plays
    pub fn keep_samples(&mut self, track_sounds: &HashSet<String>) {
        self.sampled.retain(|name, _| track_sounds.iter().any(|t| sampled(t) == *name));
    }
}

/// the name a sampled loop's sound has in the kit: its track's sound
pub fn sampled(track_sound: &str) -> String {
    format!("sampled:{track_sound}")
}

/// an Ogg Vorbis file as samples, the right channel empty for a mono one (the kit's files are at its rate)
fn decode(bytes: Vec<u8>) -> Result<(Vec<f32>, Vec<f32>), String> {
    decode_at(bytes).map(|(l, r, _)| (l, r))
}

/// an Ogg Vorbis file as samples, and the rate they are at
pub fn decode_at(bytes: Vec<u8>) -> Result<(Vec<f32>, Vec<f32>, u32), String> {
    use symphonia::core::codecs::CodecParameters;
    use symphonia::core::codecs::audio::AudioDecoderOptions;
    use symphonia::core::errors::Error as Sym;
    use symphonia::core::formats::probe::Hint;
    use symphonia::core::formats::{FormatOptions, TrackType};
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;

    let mss = MediaSourceStream::new(Box::new(std::io::Cursor::new(bytes)), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("ogg");
    let mut format = symphonia::default::get_probe()
        .probe(&hint, mss, FormatOptions::default(), MetadataOptions::default())
        .map_err(|e| e.to_string())?;
    let track = format.default_track(TrackType::Audio).ok_or("no audio in it")?;
    let id = track.id;
    let Some(CodecParameters::Audio(params)) = track.codec_params.clone() else { return Err("no codec in it".into()) };
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(&params, &AudioDecoderOptions::default())
        .map_err(|e| e.to_string())?;
    let channels = params.channels.as_ref().map_or(1, |c| c.count()).max(1);
    let rate = params.sample_rate.unwrap_or(RATE);
    let (mut l, mut r, mut buf) = (Vec::new(), Vec::new(), Vec::new());
    loop {
        let packet = match format.next_packet() {
            Ok(Some(p)) => p,
            Ok(None) | Err(Sym::ResetRequired) => break,
            Err(Sym::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e.to_string()),
        };
        if packet.track_id != id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(audio) => {
                audio.copy_to_vec_interleaved(&mut buf);
                for frame in buf.chunks_exact(channels) {
                    l.push(frame[0]);
                    if channels > 1 {
                        r.push(frame[1]);
                    }
                }
            }
            Err(Sym::DecodeError(_)) => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok((l, r, rate))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// one of claudius' own sounds, its pluck preset at F#4
    const PLUCK: &[u8] = include_bytes!("testdata/pluck-66.ogg");

    #[test]
    fn an_ogg_file_decodes() {
        let (l, r) = decode(PLUCK.to_vec()).unwrap();
        assert!(l.len() > 20_000 && l.len() == r.len(), "{} {}", l.len(), r.len());
        assert!(l.iter().fold(0f32, |m, x| m.max(x.abs())) > 0.3);
    }

    #[test]
    fn a_kit_in_a_folder_arrives_and_plays() {
        let dir = std::env::temp_dir().join(format!("claudius-demo-kit-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("pluck-66.ogg"), PLUCK).unwrap();
        std::fs::write(
            dir.join("kit.json"),
            r#"{"version": 1, "rate": 48000, "credits": ["claudius"], "sounds": {"pluck": {"pitched": true,
            "release": 0.2, "samples": [
                {"note": 66, "file": "pluck-66.ogg", "secs": 0.64, "energy": [0.001, 0.002, 0.003, 0.004, 0.005, 0.005, 0.005, 0.005, 0.005]},
                {"note": 90, "file": "missing.ogg", "secs": 0.6, "energy": [0, 0, 0, 0, 0, 0, 0, 0, 0]}]}}}"#,
        )
        .unwrap();
        let mut kit = Kit::at(Some(dir.display().to_string()), None);
        assert!(kit.poll() && kit.has("pluck") && !kit.has("bell"));
        assert_eq!(kit.credits.take(), Some(vec!["claudius".to_string()]));
        let need = Need { sound: "pluck".into(), low: 60, high: 72 };
        assert!(kit.waiting(std::slice::from_ref(&need)), "nothing has been asked for yet");
        kit.want(std::slice::from_ref(&need));
        assert!(kit.poll() && !kit.waiting(std::slice::from_ref(&need)));
        let sound = kit.sound(&need).unwrap();
        assert_eq!(sound.clips.len(), 1, "only the files within half an octave of the notes it reaches");
        let (clip, ratio) = sound.pick(69.0).unwrap();
        assert!((ratio - 2f32.powf(3.0 / 12.0)).abs() < 1e-4, "{ratio}");
        assert!((clip.energy_until(0.09375) - 0.0015).abs() < 1e-6, "between the times its energy is known at");
        assert_eq!(clip.energy_until(99.0), 0.005);
        let high = Need { sound: "pluck".into(), low: 88, high: 90 };
        kit.want(std::slice::from_ref(&high));
        assert!(!kit.poll(), "a file that cannot be read brings nothing to play");
        assert!(!kit.waiting(std::slice::from_ref(&high)), "and is not waited for");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn without_a_kit_nothing_waits() {
        let kit = Kit::at(None, None);
        assert!(kit.gone() && !kit.waiting(&[Need { sound: "pluck".into(), low: 60, high: 60 }]));
    }
}

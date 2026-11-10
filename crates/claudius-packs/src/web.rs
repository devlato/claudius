//! Web editions of installed instruments: what a browser can fetch and hold of each for a song, kept as the pack keeps
//! it but smaller. Of each instrument, the first variants; of each variant, one velocity layer (two for a keyboard,
//! whose tone changes most with how hard it is struck), its first round robin, a root every few semitones for a pitched
//! one, its key ranges widened to meet halfway, and a few seconds of each note (a note that loops keeps its loop), as
//! Ogg Vorbis. The index it writes is a pack's like any other, which a browser hands to `provide`, and then each file as
//! it fetches it to `provide_file`.

use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::decode::{self, Remote, Sample};
use crate::model::{FileEntry, InstrumentDef, Kind, LoopMode, PackIndex, Variant, Zone};

/// what of an instrument its web edition keeps
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limits {
    /// velocity layers: one (the one struck at 96), or two (at 48 and 100)
    pub layers: usize,
    /// the fewest semitones between two roots kept
    pub spacing: f32,
    /// the most seconds of a note kept, unless it loops
    pub secs: f32,
    /// variants kept (a drum machine's sound has many)
    pub variants: usize,
    /// Ogg Vorbis quality, 0 to 10
    pub quality: u8,
}

impl Limits {
    /// what a browser can hold of an instrument of `category`: two layers and five seconds of a piano, one layer and
    /// six seconds of what bows or blows (whose notes are held), four of what is plucked or struck, and two layers and
    /// two seconds of a drum (a ghost note is another hit than an accent) in four of its sounds
    pub fn of(category: &str, kind: Kind) -> Limits {
        let base = Limits { layers: 1, spacing: 3.0, secs: 4.0, variants: 1, quality: 4 };
        if kind == Kind::Drum {
            return Limits { layers: 2, secs: 2.0, variants: 4, ..base };
        }
        match category {
            "piano" | "electric piano" => Limits { layers: 2, secs: 5.0, ..base },
            "strings" | "brass" | "woodwinds" | "organ" | "voice" | "choir" | "pad" => Limits { secs: 6.0, ..base },
            _ => base,
        }
    }
}

/// what to make: an instrument of the pack by its name, and the names it also answers to in its web edition (a web
/// studio that has a Wurlitzer and no Rhodes plays the one for the other)
#[derive(Clone, Debug)]
pub struct Pick {
    pub instrument: String,
    pub aliases: Vec<String>,
}

/// The web edition of the instruments `picks` of the installed pack `ix` (its files under `dir`, fetched first where
/// the pack is installed lazily), written under `out`: each note a file `<instrument>/<n>.ogg`, its URL `base` and
/// that, and the index of them, which is returned (and written as `out/index.json`).
pub fn edition(ix: &PackIndex, dir: &Path, picks: &[Pick], base: &str, out: &Path) -> Result<PackIndex> {
    let spec: claudius_encode::Spec = "ogg".parse()?;
    if let Some(why) = spec.unavailable() {
        bail!("a web edition is Ogg Vorbis, which this build cannot write: {why}");
    }
    let mut files: Vec<FileEntry> = Vec::new();
    let mut instruments: Vec<InstrumentDef> = Vec::new();
    for pick in picks {
        let def = ix
            .instruments
            .iter()
            .find(|d| d.name == pick.instrument)
            .with_context(|| format!("no instrument {} in {}", pick.instrument, ix.name))?;
        let limits = Limits::of(&def.category, def.kind);
        let mut variants = Vec::new();
        // what the files will tell once they come, which a song asks before: whether it was recorded in mono
        let mut recorded_mono = None;
        for (n, v) in def.variants.iter().take(limits.variants).enumerate() {
            let mut zones = Vec::new();
            let mut mono = Vec::new();
            for z in kept(&v.zones, def.kind, &limits) {
                let entry =
                    ix.files.get(z.file as usize).with_context(|| format!("{}: no file {}", def.name, z.file))?;
                let remote = (!entry.url.is_empty()).then(|| Remote {
                    url: entry.url.clone(),
                    size: entry.size,
                    flac: entry.flac,
                });
                let sample = decode::load(&dir.join(&entry.path), remote.as_ref())
                    .with_context(|| format!("{}: {}", def.name, entry.path))?;
                let (pcm, channels, zone) = cut(&sample, &z, &limits);
                let quality = format!("ogg:{}", limits.quality).parse::<claudius_encode::Spec>()?;
                let bytes = claudius_encode::encode(
                    &quality,
                    claudius_encode::Pcm { samples: &pcm, channels, rate: sample.sr },
                    &Default::default(),
                )?;
                let path = format!("{}/{}.ogg", def.name, files.len());
                let at = out.join(&path);
                if let Some(parent) = at.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&at, &bytes).with_context(|| format!("writing {}", at.display()))?;
                files.push(FileEntry {
                    url: format!("{}/{path}", base.trim_end_matches('/')),
                    path,
                    size: bytes.len() as u64,
                    flac: false,
                });
                zones.push(Zone { file: files.len() as u32 - 1, ..zone });
                mono.push(sample.r.is_none());
            }
            if n == 0 {
                recorded_mono = crate::player::reference(&zones).map(|i| mono[i]);
            }
            variants.push(Variant { name: v.name.clone(), zones });
        }
        let mut aliases = def.aliases.clone();
        aliases.extend(pick.aliases.iter().filter(|a| !def.aliases.contains(a)).cloned());
        instruments.push(InstrumentDef { aliases, variants, recorded_mono, ..def.clone() });
    }
    let index = PackIndex {
        files,
        instruments,
        lazy: false,
        installed_at: 0,
        version: format!("{}-web", ix.version),
        ..ix.clone()
    };
    std::fs::create_dir_all(out)?;
    std::fs::write(out.join("index.json"), serde_json::to_vec(&index)?)?;
    Ok(index)
}

/// The zones of a variant its web edition keeps, as they then play: the velocity layers `limits` says, each over a
/// share of all velocities with no crossfade; the first round robin and the first random alternative; and of a
/// pitched instrument a root every `limits.spacing` semitones at least, the lowest and the highest among them, each
/// zone's keys reaching halfway to the next one's.
fn kept(zones: &[Zone], kind: Kind, limits: &Limits) -> Vec<Zone> {
    let first = |z: &&Zone| z.seq_pos <= 1 && z.lorand <= 0.0;
    let layers: Vec<(u8, u8)> = {
        let mut l: Vec<(u8, u8)> = zones.iter().filter(first).map(|z| (z.lovel, z.hivel)).collect();
        l.sort_unstable();
        l.dedup();
        l
    };
    let at = |v: u8| layers.iter().copied().filter(|(lo, hi)| (*lo..=*hi).contains(&v)).max_by_key(|l| l.1);
    let wanted: Vec<(u8, u8)> = if limits.layers >= 2 && layers.len() >= 2 {
        let (soft, loud) = (at(48).unwrap_or(layers[0]), at(100).unwrap_or(layers[layers.len() - 1]));
        if soft == loud { vec![loud] } else { vec![soft, loud] }
    } else {
        at(96).or_else(|| layers.last().copied()).into_iter().collect()
    };
    let mut out = Vec::new();
    for (i, layer) in wanted.iter().enumerate() {
        // the soft layer plays up to the loud one's lowest velocity, the loud one from there
        let (lovel, hivel) = match (wanted.len(), i) {
            (1, _) => (0, 127),
            (_, 0) => (0, wanted[1].0.saturating_sub(1).max(1)),
            _ => (wanted[1].0.max(2), 127),
        };
        let mut these: Vec<Zone> = zones
            .iter()
            .filter(first)
            .filter(|z| (z.lovel, z.hivel) == *layer)
            .map(|z| Zone {
                lovel,
                hivel,
                xfin: None,
                xfout: None,
                seq_len: 0,
                seq_pos: 0,
                lorand: 0.0,
                hirand: 1.0,
                ..z.clone()
            })
            .collect();
        if kind == Kind::Pitched && these.iter().all(|z| z.keytrack > 0.0) {
            these = spaced(these, limits.spacing);
        }
        out.extend(these);
    }
    out
}

/// a root every `spacing` semitones at least of `zones`, the lowest and the highest among them, each zone's keys
/// reaching halfway to the next one's and the outer ones as far as the zones reached
fn spaced(mut zones: Vec<Zone>, spacing: f32) -> Vec<Zone> {
    zones.sort_by(|a, b| a.root.total_cmp(&b.root));
    let (lowest, highest) =
        (zones.iter().map(|z| z.lokey).min().unwrap_or(0), zones.iter().map(|z| z.hikey).max().unwrap_or(127));
    let mut keep: Vec<Zone> = Vec::new();
    for (i, z) in zones.iter().enumerate() {
        let last = i + 1 == zones.len();
        if keep.last().is_none_or(|k| z.root - k.root >= spacing) || last {
            if last && keep.last().is_some_and(|k| z.root - k.root < spacing / 2.0) {
                keep.pop();
            }
            keep.push(z.clone());
        }
    }
    let roots: Vec<f32> = keep.iter().map(|z| z.root).collect();
    for (i, z) in keep.iter_mut().enumerate() {
        z.lokey = if i == 0 { lowest } else { ((roots[i - 1] + roots[i]) / 2.0).floor() as u8 + 1 };
        z.hikey = if i + 1 == roots.len() { highest } else { ((roots[i] + roots[i + 1]) / 2.0).floor() as u8 };
    }
    keep
}

/// The part of `sample` a zone plays, as interleaved samples, its channels, and the zone as it plays the file cut from
/// it: from its offset on, at most `limits.secs` long unless it loops (then through its loop), faded out over its last
/// 30 ms where it is cut short.
fn cut(sample: &Sample, z: &Zone, limits: &Limits) -> (Vec<f32>, usize, Zone) {
    let len = sample.len();
    let from = (z.offset as usize).min(len);
    let end = if z.end > 0 { (z.end as usize + 1).min(len) } else { len };
    let file_loop = sample.file_loop.filter(|_| z.loop_end == 0);
    let (loop_start, loop_end) = file_loop.map_or((z.loop_start, z.loop_end), |(s, e)| (s, e));
    let loops = matches!(z.loop_mode, LoopMode::Continuous | LoopMode::Sustain) && loop_end > loop_start;
    let most = from + (limits.secs * sample.sr as f32) as usize;
    let to = if loops { end.min((loop_end as usize + sample.sr as usize / 10).max(most)) } else { end.min(most) };
    let n = to.saturating_sub(from);
    let fade = if to < end && !loops { (0.03 * sample.sr as f32) as usize } else { 0 }.min(n);
    let gain = |i: usize| if i + fade >= n && fade > 0 { (n - i) as f32 / fade as f32 } else { 1.0 };
    let right = sample.r.as_deref();
    let pcm: Vec<f32> = match right {
        Some(r) => (0..n).flat_map(|i| [sample.l[from + i] * gain(i), r[from + i] * gain(i)]).collect(),
        None => (0..n).map(|i| sample.l[from + i] * gain(i)).collect(),
    };
    let shift = |p: u32| p.saturating_sub(from as u32);
    let zone = Zone {
        offset: 0,
        end: 0,
        loop_start: if loops { shift(loop_start) } else { 0 },
        loop_end: if loops { shift(loop_end) } else { 0 },
        loop_mode: if loops {
            z.loop_mode
        } else if z.loop_mode == LoopMode::OneShot {
            LoopMode::OneShot
        } else {
            LoopMode::NoLoop
        },
        ..z.clone()
    };
    (pcm, if right.is_some() { 2 } else { 1 }, zone)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// a pack in `dir` of a tone sampled at every root from 48 to 60 in two velocity layers, each 6 s of a sine at its
    /// root, and a drum of two hits
    fn source(dir: &Path) -> PackIndex {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut files = Vec::new();
        let mut write = |name: &str, hz: f64, secs: f64| {
            let path = format!("{name}.wav");
            let mut w = hound::WavWriter::create(dir.join(&path), spec).unwrap();
            for i in 0..(secs * 48_000.0) as usize {
                let v = 0.5 * (std::f64::consts::TAU * hz * i as f64 / 48_000.0).sin();
                w.write_sample((v * 32767.0) as i16).unwrap();
            }
            w.finalize().unwrap();
            files.push(FileEntry { path, url: String::new(), size: 0, flac: false });
            files.len() as u32 - 1
        };
        let mut tone = Vec::new();
        for (lovel, hivel) in [(0, 63), (64, 127)] {
            for root in 48..=60 {
                let file = write(&format!("tone{root}_{lovel}"), 440.0 * 2f64.powf((root as f64 - 69.0) / 12.0), 6.0);
                tone.push(Zone {
                    file,
                    root: root as f32,
                    lokey: root,
                    hikey: root,
                    lovel,
                    hivel,
                    ..Default::default()
                });
            }
        }
        let hits = [write("hit0", 200.0, 0.5), write("hit1", 300.0, 3.0)];
        let def = |name: &str, category: &str, kind: Kind, zones: Vec<Zone>| InstrumentDef {
            name: name.into(),
            category: category.into(),
            kind,
            description: String::new(),
            one_shot: kind == Kind::Drum,
            release: 0.2,
            attack: 0.0,
            gain_db: 0.0,
            veltrack: 1.0,
            vel_curve: Vec::new(),
            mono: false,
            retrigger: false,
            recorded_mono: None,
            unison: None,
            aliases: vec![format!("{name}_too")],
            variants: vec![Variant { name: String::new(), zones }],
        };
        let drum = hits
            .iter()
            .enumerate()
            .map(|(k, &file)| Zone { file, lokey: 36 + k as u8, hikey: 36 + k as u8, ..Default::default() })
            .collect();
        PackIndex {
            format: crate::model::INDEX_FORMAT,
            name: "webtest".into(),
            title: "a test".into(),
            description: String::new(),
            categories: Vec::new(),
            licence: "CC0-1.0".into(),
            licence_url: String::new(),
            attribution: String::new(),
            attribution_required: false,
            licence_note: String::new(),
            source: String::new(),
            version: "1".into(),
            default: false,
            opt_in: false,
            warning: None,
            user: false,
            lazy: false,
            installed_at: 0,
            aliases: Vec::new(),
            files,
            instruments: vec![
                def("webtest_tone", "piano", Kind::Pitched, tone),
                def("webtest_kit", "drums", Kind::Drum, drum),
            ],
        }
    }

    #[test]
    fn a_web_edition_keeps_a_root_every_three_semitones_in_two_layers_cut_short_and_plays_in_a_browser_as_handed_over()
    {
        let (src, out) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let ix = source(src.path());
        let picks = [
            Pick { instrument: "webtest_tone".into(), aliases: vec!["webtest_other".into()] },
            Pick { instrument: "webtest_kit".into(), aliases: Vec::new() },
        ];
        let web = edition(&ix, src.path(), &picks, "/studio/packs/webtest", out.path()).unwrap();
        let tone = &web.instruments[0];
        assert_eq!(tone.aliases, ["webtest_tone_too", "webtest_other"]);
        assert_eq!(tone.recorded_mono, Some(true), "said before its files come");
        let zones = &tone.variants[0].zones;
        // roots 48, 51, 54, 57, 60 in each layer: the keys from 48 to 60 each played by one zone of each layer
        let roots: Vec<f32> = zones.iter().filter(|z| z.hivel == 127).map(|z| z.root).collect();
        assert_eq!(roots, [48.0, 51.0, 54.0, 57.0, 60.0]);
        for key in 48..=60u8 {
            for vel in [10u8, 63, 64, 127] {
                let playing = zones
                    .iter()
                    .filter(|z| (z.lokey..=z.hikey).contains(&key) && (z.lovel..=z.hivel).contains(&vel))
                    .count();
                assert_eq!(playing, 1, "key {key} at {vel}");
            }
        }
        // a piano's note, five seconds of six; the drum's hits whole, the longer one cut to two
        let secs = |file: u32| {
            let f = &web.files[file as usize];
            let d = claudius_decode::decode_bytes(std::fs::read(out.path().join(&f.path)).unwrap(), "ogg").unwrap();
            assert!(f.url.ends_with(&f.path) && f.url.starts_with("/studio/packs/webtest/"));
            d.samples.len() as f64 / d.channels as f64 / d.rate as f64
        };
        assert!(zones.iter().all(|z| (secs(z.file) - 5.0).abs() < 0.05));
        let kit = &web.instruments[1].variants[0].zones;
        assert_eq!(kit.len(), 2);
        assert!((secs(kit[0].file) - 0.5).abs() < 0.05 && (secs(kit[1].file) - 2.0).abs() < 0.05);
        // handed over as a browser hands it: an instrument once its index is, playing once its files are
        crate::provide(web.clone());
        let r = crate::resolve("webtest_other", None).expect("resolves by the alias its edition gave it");
        assert!(crate::instrument_ref(&r).unwrap().recorded_mono(), "known with none of its files");
        let wanted = crate::wanted(std::slice::from_ref(&r));
        assert_eq!(wanted.len(), zones.len());
        for (pack, path, _) in &wanted {
            crate::provide_file(pack, path, std::fs::read(out.path().join(path)).unwrap()).unwrap();
        }
        assert!(crate::wanted(std::slice::from_ref(&r)).is_empty());
        let note = [crate::NoteOn { start: 0, gate: 24_000, pitch: 55.0, velocity: 0.8 }];
        let (l, _) = crate::instrument_ref(&r).unwrap().render(&note, 48_000);
        let crossings = l.windows(2).take(24_000).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count();
        // G3 is 196 Hz: about 98 rising crossings in half a second
        assert!((90..=106).contains(&crossings), "{crossings}");
    }
}

//! claudius-packs: install, list, audition and render the sample packs.
//!
//! ```text
//! claudius-packs list
//! claudius-packs install --defaults | <pack>... [--lazy] [--accept-licence] [--threads N]
//! claudius-packs remove <pack>
//! claudius-packs instruments
//! claudius-packs resolve <name> [bank]
//! claudius-packs aliases
//! claudius-packs add-map <url | github:user/repo[/branch[/path]]>
//! claudius-packs credits [name...]
//! claudius-packs audition [out_dir] [--pack P]
//! claudius-packs render <name> <out.wav> [--bank B] [--notes 60,64,67] [--secs S]
//! claudius-packs bench [name] [seconds] [--bank B]
//! ```

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use claudius_packs::{InstallOptions, InstrumentInfo, NoteOn, SR};

fn progress(frac: f32, msg: &str) {
    eprint!("\r\x1b[2K[{:>3.0}%] {msg}", frac * 100.0);
    if frac >= 1.0 {
        eprintln!();
    }
}

fn flag(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

fn opt<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(|s| s.as_str())
}

fn positional(args: &[String]) -> Vec<&str> {
    let mut out = Vec::new();
    let mut skip = false;
    for a in args {
        if skip {
            skip = false;
            continue;
        }
        if matches!(a.as_str(), "--threads" | "--pack" | "--bank" | "--notes" | "--secs") {
            skip = true;
            continue;
        }
        if a.starts_with("--") {
            continue;
        }
        out.push(a.as_str());
    }
    out
}

/// 24-bit WAV; anything louder than -1 dBFS is scaled down rather than clipped
fn write_wav(path: &Path, l: &[f32], r: &[f32]) -> Result<()> {
    let spec =
        hound::WavSpec { channels: 2, sample_rate: SR, bits_per_sample: 24, sample_format: hound::SampleFormat::Int };
    let mut w = hound::WavWriter::create(path, spec).with_context(|| format!("cannot write {}", path.display()))?;
    let peak = l.iter().chain(r).fold(0.0f32, |m, v| m.max(v.abs()));
    let g = if peak > 0.891 { 0.891 / peak } else { 1.0 };
    for i in 0..l.len() {
        for v in [l[i], r[i]] {
            w.write_sample(((v * g).clamp(-1.0, 1.0) * 8_388_607.0) as i32)?;
        }
    }
    w.finalize()?;
    Ok(())
}

fn secs(s: f64) -> usize {
    (s * SR as f64) as usize
}

/// a C to play around: the C nearest the middle of the recorded range
fn base_c(info: &InstrumentInfo) -> f64 {
    let mid = (info.lowest as f64 + info.highest as f64) / 2.0;
    let c = (mid / 12.0).round() * 12.0 - 6.0;
    (c / 12.0).round().max(2.0) * 12.0
}

/// C major arpeggio, then the chord held
fn arpeggio(info: &InstrumentInfo) -> (Vec<NoteOn>, usize) {
    let c = base_c(info).min(84.0);
    let step = secs(0.3);
    let mut notes = Vec::new();
    for (i, iv) in [0.0, 4.0, 7.0, 12.0].iter().enumerate() {
        notes.push(NoteOn { start: i * step, gate: secs(0.26), pitch: c + iv, velocity: 0.55 + 0.1 * i as f32 });
    }
    let t = 4 * step + secs(0.1);
    for iv in [0.0, 4.0, 7.0] {
        notes.push(NoteOn { start: t, gate: secs(1.4), pitch: c + iv, velocity: 0.75 });
    }
    (notes, t + secs(1.4) + secs(1.2))
}

fn stats(l: &[f32], r: &[f32]) -> (f32, f32, bool) {
    let mut peak = 0.0f32;
    let mut sum = 0.0f64;
    let mut bad = false;
    for (a, b) in l.iter().zip(r) {
        if !a.is_finite() || !b.is_finite() {
            bad = true;
        }
        peak = peak.max(a.abs()).max(b.abs());
        sum += (*a as f64).powi(2) + (*b as f64).powi(2);
    }
    let rms = (sum / (2 * l.len().max(1)) as f64).sqrt() as f32;
    (peak, rms, bad)
}

fn db(x: f32) -> f32 {
    20.0 * x.max(1e-9).log10()
}

/// windowed Goertzel power (dB) of `x` at frequency `f`
fn power_db(x: &[f32], f: f64) -> f64 {
    let n = x.len();
    let k = 2.0 * (2.0 * std::f64::consts::PI * f / SR as f64).cos();
    let (mut s1, mut s2) = (0.0f64, 0.0f64);
    for (i, v) in x.iter().enumerate() {
        let w = 0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / (n - 1) as f64).cos();
        let s0 = *v as f64 * w + k * s1 - s2;
        s2 = s1;
        s1 = s0;
    }
    10.0 * (s1 * s1 + s2 * s2 - k * s1 * s2).max(1e-20).log10()
}

/// chime spectra are inharmonic with no partial at the nominal pitch, so the check below does not apply
const OCTAVE_CHECK_SKIP: &[&str] = &["vcsl:tubular_bells"];

/// Render one note and look for octave mistakes in the mapping: a correctly pitched harmonic sound has nothing
/// at half its fundamental, and its fundamental is not tens of dB below the octave above.
fn octave_check(inst: &claudius_packs::Instrument, info: &InstrumentInfo) -> Option<String> {
    if OCTAVE_CHECK_SKIP.contains(&format!("{}:{}", info.pack, info.instrument).as_str()) {
        return None;
    }
    let p = base_c(info).clamp(info.lowest as f64, info.highest.max(info.lowest) as f64).round();
    let note = NoteOn { start: 0, gate: secs(1.0), pitch: p, velocity: 0.8 };
    let (l, r) = inst.render(&[note], secs(1.2));
    let mono: Vec<f32> =
        l[secs(0.1)..secs(0.7)].iter().zip(&r[secs(0.1)..secs(0.7)]).map(|(a, b)| 0.5 * (a + b)).collect();
    if mono.iter().all(|v| v.abs() < 1e-5) {
        return Some(format!("silent at {}", claudius_packs::parse::note_name(p as i32)));
    }
    let f = 440.0 * 2f64.powf((p - 69.0) / 12.0);
    let (half, fund, fifth, oct) =
        (power_db(&mono, f / 2.0), power_db(&mono, f), power_db(&mono, 1.5 * f), power_db(&mono, 2.0 * f));
    let name = claudius_packs::parse::note_name(p as i32);
    if half > fund + 3.0 && half > oct - 10.0 {
        return Some(format!("{name}: energy at f/2 ({half:.0} dB) above f ({fund:.0} dB): sounds an octave low?"));
    }
    if fund < oct - 30.0 && fifth < oct - 30.0 {
        return Some(format!("{name}: no fundamental ({fund:.0} dB vs {oct:.0} dB at 2f): sounds an octave high?"));
    }
    None
}

fn audition(out: &Path, only: Option<&str>) -> Result<()> {
    std::fs::create_dir_all(out)?;
    let mut infos = claudius_packs::instruments();
    if let Some(p) = only {
        infos.retain(|i| i.pack == p);
    }
    let mut by_cat: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut problems = Vec::new();
    let t0 = Instant::now();
    // drum kits: one beat per pack that has bd + sd + hh
    let mut kits: BTreeMap<String, Vec<InstrumentInfo>> = BTreeMap::new();
    for i in &infos {
        if i.kind == "drum" {
            kits.entry(i.pack.clone()).or_default().push(i.clone());
        }
    }
    for (pack, members) in &kits {
        let has = |n: &str| members.iter().any(|m| m.instrument == n);
        if !(has("bd") && has("sd") && has("hh")) {
            continue;
        }
        let beat = secs(0.125); // 16ths at 120 bpm
        let len = 32 * beat + secs(1.5);
        let (mut l, mut r) = (vec![0.0f32; len], vec![0.0f32; len]);
        let lanes: [(&str, &[usize], f32); 6] = [
            ("bd", &[0, 6, 8, 16, 22, 24, 26], 0.95),
            ("sd", &[4, 12, 20, 28], 0.9),
            ("hh", &[0, 2, 4, 6, 8, 10, 12, 16, 18, 20, 22, 24, 26, 28], 0.6),
            ("oh", &[14, 30], 0.7),
            ("cp", &[12, 28], 0.6),
            ("cr", &[0], 0.6),
        ];
        let mut hh_times = Vec::new();
        for (lane, steps, vel) in lanes {
            let Some(inst) =
                claudius_packs::resolve(lane, Some(pack)).and_then(|r| claudius_packs::instrument_ref(&r).ok())
            else {
                continue;
            };
            let notes: Vec<NoteOn> = steps
                .iter()
                .map(|&s| NoteOn {
                    start: s * beat,
                    gate: beat,
                    pitch: 60.0,
                    velocity: vel * if s % 4 == 0 { 1.0 } else { 0.8 },
                })
                .collect();
            if lane == "hh" {
                hh_times = notes.iter().map(|n| n.start).collect();
            }
            let (a, b) =
                if lane == "oh" { inst.render_choked(&notes, &hh_times, len) } else { inst.render(&notes, len) };
            for k in 0..len {
                l[k] += a[k] * 0.6;
                r[k] += b[k] * 0.6;
            }
        }
        let (peak, rms, bad) = stats(&l, &r);
        let path = out.join(format!("{pack}__beat.wav"));
        write_wav(&path, &l, &r)?;
        println!(
            "{:<44} beat      peak {:>6.1} dB  rms {:>6.1} dB",
            path.file_name().unwrap().to_string_lossy(),
            db(peak),
            db(rms)
        );
        by_cat.entry("drum kit".into()).or_default().push(format!("{pack}__beat.wav"));
        if bad || peak < 1e-4 {
            problems.push(format!("{pack} beat: {}", if bad { "non-finite samples" } else { "silent" }));
        }
    }
    for info in &infos {
        let Ok(inst) = claudius_packs::instrument_ref(&claudius_packs::InstrumentRef {
            pack: info.pack.clone(),
            instrument: info.instrument.clone(),
            variant: 0,
        }) else {
            continue;
        };
        let (notes, len) = if info.kind == "drum" {
            // a few variants (or the same hit at rising velocity)
            let n = info.variants.clamp(1, 4);
            let mut notes = Vec::new();
            let mut len = 0;
            let mut parts: Vec<(usize, std::sync::Arc<claudius_packs::Instrument>)> = Vec::new();
            for v in 0..n {
                let r = claudius_packs::InstrumentRef {
                    pack: info.pack.clone(),
                    instrument: info.instrument.clone(),
                    variant: v,
                };
                if let Ok(i) = claudius_packs::instrument_ref(&r) {
                    parts.push((v, i));
                }
            }
            let hits = if n == 1 { 4 } else { 1 };
            let total = parts.len() * hits;
            let spacing = secs(0.6);
            let full = total * spacing + secs(2.0);
            let (mut l, mut r) = (vec![0.0f32; full], vec![0.0f32; full]);
            let mut k = 0;
            for (_, inst) in &parts {
                let mut ns = Vec::new();
                for h in 0..hits {
                    ns.push(NoteOn {
                        start: k * spacing,
                        gate: secs(0.2),
                        pitch: 60.0,
                        velocity: if hits > 1 { 0.4 + 0.2 * h as f32 } else { 0.9 },
                    });
                    k += 1;
                }
                let (a, b) = inst.render(&ns, full);
                for j in 0..full {
                    l[j] += a[j];
                    r[j] += b[j];
                }
                notes.extend(ns);
            }
            len = len.max(full);
            let (peak, rms, bad) = stats(&l, &r);
            let path = out.join(format!("{}__{}.wav", info.pack, info.instrument));
            write_wav(&path, &l, &r)?;
            println!(
                "{:<44} {:<9} peak {:>6.1} dB  rms {:>6.1} dB  {} variants",
                path.file_name().unwrap().to_string_lossy(),
                info.category.chars().take(9).collect::<String>(),
                db(peak),
                db(rms),
                info.variants
            );
            by_cat.entry(info.category.clone()).or_default().push(format!("{}__{}.wav", info.pack, info.instrument));
            if bad || peak < 1e-4 {
                problems.push(format!(
                    "{}:{} {}",
                    info.pack,
                    info.instrument,
                    if bad { "non-finite samples" } else { "silent" }
                ));
            }
            let _ = (notes, len);
            continue;
        } else {
            arpeggio(info)
        };
        if let Some(w) = octave_check(&inst, info) {
            problems.push(format!("{}:{} {w}", info.pack, info.instrument));
        }
        let (l, r) = inst.render(&notes, len);
        let (peak, rms, bad) = stats(&l, &r);
        let path = out.join(format!("{}__{}.wav", info.pack, info.instrument));
        write_wav(&path, &l, &r)?;
        println!(
            "{:<44} {:<9} peak {:>6.1} dB  rms {:>6.1} dB  range {}..{}  {} layer(s)",
            path.file_name().unwrap().to_string_lossy(),
            info.category.chars().take(9).collect::<String>(),
            db(peak),
            db(rms),
            claudius_packs::parse::note_name(info.lowest as i32),
            claudius_packs::parse::note_name(info.highest as i32),
            info.velocity_layers
        );
        by_cat.entry(info.category.clone()).or_default().push(format!("{}__{}.wav", info.pack, info.instrument));
        if bad || peak < 1e-4 {
            problems.push(format!(
                "{}:{} {}",
                info.pack,
                info.instrument,
                if bad { "non-finite samples" } else { "silent" }
            ));
        }
    }
    println!(
        "\n{} files in {} ({:.1} s, {:.0} MB decoded audio cached)",
        by_cat.values().map(|v| v.len()).sum::<usize>(),
        out.display(),
        t0.elapsed().as_secs_f64(),
        claudius_packs::cache_bytes() as f64 / 1e6
    );
    for (cat, files) in &by_cat {
        println!("  {cat:<22} {}", files.len());
    }
    if !problems.is_empty() {
        bail!("problems:\n  {}", problems.join("\n  "));
    }
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(|s| s.as_str()).unwrap_or("help");
    let rest = &args[1.min(args.len())..];
    match cmd {
        "list" => {
            println!("packs in {}", claudius_packs::packs_dir().display());
            for p in claudius_packs::packs() {
                let state = if p.installed { if p.lazy { "lazy" } else { "installed" } } else { "-" };
                let tag = if p.opt_in {
                    "opt-in"
                } else if p.default {
                    "default"
                } else if p.user {
                    "user"
                } else {
                    "optional"
                };
                println!(
                    "{:<20} {:<9} {:<8} {:>7.1} MB  {:<3} instr  {:<14}  {}",
                    p.name,
                    state,
                    tag,
                    p.size_mb,
                    p.instruments,
                    p.licence.chars().take(14).collect::<String>(),
                    p.title
                );
            }
        }
        "install" => {
            let opts = InstallOptions {
                lazy: flag(rest, "--lazy"),
                threads: opt(rest, "--threads").and_then(|t| t.parse().ok()).unwrap_or(8),
                accept_opt_in: flag(rest, "--accept-licence") || flag(rest, "--accept-license"),
            };
            let t0 = Instant::now();
            let before: f32 = claudius_packs::packs().iter().filter(|p| p.installed).map(|p| p.size_mb).sum();
            if flag(rest, "--defaults") {
                claudius_packs::install_defaults(&progress)?;
            } else {
                let names = positional(rest);
                if names.is_empty() {
                    bail!("install what? (--defaults or pack names)");
                }
                for n in names {
                    claudius_packs::install_with(n, &opts, &progress)?;
                }
            }
            let after: f32 = claudius_packs::packs().iter().filter(|p| p.installed).map(|p| p.size_mb).sum();
            println!(
                "done in {:.1} s; packs on disk {:.1} MB (was {:.1} MB)",
                t0.elapsed().as_secs_f64(),
                after,
                before
            );
        }
        "remove" => {
            for n in positional(rest) {
                claudius_packs::remove(n)?;
                println!("removed {n}");
            }
        }
        "instruments" => {
            for i in claudius_packs::instruments() {
                println!(
                    "{:<36} {:<20} {:<7} {:>4}..{:<4} layers {:<2} rr {:<2} zones {:<4} variants {:<3} {}",
                    i.name,
                    i.category,
                    i.kind,
                    claudius_packs::parse::note_name(i.lowest as i32),
                    claudius_packs::parse::note_name(i.highest as i32),
                    i.velocity_layers,
                    i.round_robin,
                    i.zones,
                    i.variants,
                    if i.one_shot { "one-shot" } else { "" }
                );
            }
        }
        "resolve" => {
            let p = positional(rest);
            let name = p.first().copied().unwrap_or("piano");
            match claudius_packs::resolve(name, p.get(1).copied()) {
                Some(r) => println!("{r}"),
                None => bail!("nothing installed matches {name}"),
            }
        }
        "aliases" => {
            for (n, r) in claudius_packs::aliases() {
                println!("{n:<22} -> {r}");
            }
        }
        "add-map" => {
            let p = positional(rest);
            let url = p.first().copied().context("add-map <url | github:user/repo>")?;
            let info = claudius_packs::add_sample_map(url)?;
            println!("added {} ({} sounds, licence {}, lazy)", info.name, info.instruments, info.licence);
            if let Some(w) = info.warning {
                println!("warning: {w}");
            }
        }
        "credits" => {
            let names = positional(rest);
            let s = if names.is_empty() { claudius_packs::credits() } else { claudius_packs::credits_for(&names) };
            print!("{s}");
        }
        "audition" => {
            let p = positional(rest);
            let out = PathBuf::from(p.first().copied().unwrap_or("/tmp/claudius-packs-audition"));
            audition(&out, opt(rest, "--pack"))?;
        }
        "bench" => {
            // a dense part: four-note chords every half second plus a running eighth-note line
            let p = positional(rest);
            let name = p.first().copied().unwrap_or("piano");
            let seconds: f64 = p.get(1).and_then(|s| s.parse().ok()).unwrap_or(180.0);
            let inst = claudius_packs::instrument(name, opt(rest, "--bank"))?;
            let len = secs(seconds);
            let mut notes = Vec::new();
            let mut t = 0;
            let mut k = 0usize;
            while t < len {
                for iv in [0.0, 4.0, 7.0, 11.0] {
                    notes.push(NoteOn { start: t, gate: secs(0.45), pitch: 52.0 + iv + (k % 5) as f64, velocity: 0.7 });
                }
                notes.push(NoteOn { start: t, gate: secs(0.2), pitch: 72.0 + (k % 7) as f64, velocity: 0.8 });
                notes.push(NoteOn {
                    start: t + secs(0.25),
                    gate: secs(0.2),
                    pitch: 74.0 + (k % 6) as f64,
                    velocity: 0.75,
                });
                t += secs(0.5);
                k += 1;
            }
            let t0 = Instant::now();
            inst.preload()?;
            let load = t0.elapsed().as_secs_f64();
            let t1 = Instant::now();
            let (l, _) = inst.render(&notes, len);
            let dt = t1.elapsed().as_secs_f64();
            println!(
                "{}: {} notes over {seconds:.0} s rendered in {dt:.2} s ({:.0}x real time); preload {load:.2} s; peak {:.1} dB",
                inst.info().name,
                notes.len(),
                seconds / dt,
                db(l.iter().fold(0.0f32, |m, v| m.max(v.abs())))
            );
        }
        "render" => {
            let p = positional(rest);
            let (name, out) = match (p.first(), p.get(1)) {
                (Some(n), Some(o)) => (*n, PathBuf::from(o)),
                _ => bail!("render <name> <out.wav> [--bank B] [--notes 60,64,67] [--secs S]"),
            };
            let inst = claudius_packs::instrument(name, opt(rest, "--bank"))?;
            let len = secs(opt(rest, "--secs").and_then(|s| s.parse().ok()).unwrap_or(3.0));
            let pitches: Vec<f64> =
                opt(rest, "--notes").unwrap_or("60").split(',').filter_map(|s| s.trim().parse().ok()).collect();
            let notes: Vec<NoteOn> =
                pitches.iter().map(|&p| NoteOn { start: 0, gate: len / 2, pitch: p, velocity: 0.8 }).collect();
            let (l, r) = inst.render(&notes, len);
            write_wav(&out, &l, &r)?;
            println!("{} -> {}", inst.info().name, out.display());
        }
        "help" | "--help" | "-h" => println!("{}", usage()),
        other => {
            eprintln!("unknown command '{other}'\n\n{}", usage());
            std::process::exit(2);
        }
    }
    Ok(())
}

/// the commands, from the comment at the top of this file
fn usage() -> String {
    include_str!("claudius-packs.rs")
        .lines()
        .take_while(|l| l.starts_with("//!"))
        .filter(|l| !l.contains("```"))
        .map(|l| l.trim_start_matches("//!").trim_start())
        .collect::<Vec<_>>()
        .join("\n")
}

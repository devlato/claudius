//! Reading a whole song the way a producer would before remaking it: tempo, key and swing; sections (by novelty
//! every 4 bars) with their energy and which layers play (kick, bass, hats, harmony); the chords of every bar folded
//! into looping progressions; drum habits per 16th step (kick / snare / hats); the bass style. `to_blueprint` turns
//! that skeleton into a blueprint for a genre, so a new song can be written on it and then mutated.

use serde::Serialize;

use super::beats;
use super::chroma;
use super::onsets::{self, HOP};
use crate::audio::Stereo;
use crate::compose::{self, Library, Options};
use crate::config::SRF;
use crate::dsp::filter;
use crate::genres::GenreProfile;
use crate::model::{Blueprint, SectionKind, SectionSpec};
use crate::theory::chord::{Chord, roman_of};
use crate::theory::rhythm::{self, LaneTemplate};
use crate::theory::scale::Key;

#[derive(Clone, Debug, Serialize)]
pub struct SectionGuess {
    pub start_bar: usize,
    pub bars: usize,
    pub energy: f32,
    pub kind: SectionKind,
    pub kick: bool,
    pub bass: bool,
    pub hats: bool,
    pub harmony: bool,
}

/// a row of per-step strengths as `analyze` prints it: `X` strong, `x` clear, `.` faint, `-` nothing
pub fn grid(steps: &[f32]) -> String {
    steps
        .iter()
        .map(|&x| match x {
            x if x >= 0.85 => 'X',
            x if x >= 0.4 => 'x',
            x if x > 0.0 => '.',
            _ => '-',
        })
        .collect()
}

#[derive(Clone, Debug, Serialize)]
pub struct Analysis {
    pub bpm: f64,
    pub key: Key,
    pub key_confidence: f64,
    pub swing: f32,
    pub pulse_clarity: f64,
    pub first_downbeat: f64,
    pub bar_secs: f64,
    pub bars: usize,
    pub chords: Vec<String>,
    pub progression_a: Vec<String>,
    pub progression_b: Vec<String>,
    pub bars_per_chord: u32,
    pub sections: Vec<SectionGuess>,
    /// per 16th step of a bar, 0..1: how often each lane hits there in the busiest sections
    pub kick_steps: Vec<f32>,
    pub snare_steps: Vec<f32>,
    pub hat_steps: Vec<f32>,
    pub bass_style: String,
}

fn band(mono: &[f32], lo: f64, hi: f64) -> Vec<f32> {
    let mut x = mono.to_vec();
    if lo > 0.0 {
        filter::butter(&mut x, true, lo, SRF, 4);
    }
    if hi > 0.0 {
        filter::butter(&mut x, false, hi, SRF, 4);
    }
    x
}

fn rms_db(x: &[f32], a: usize, b: usize) -> f64 {
    let b = b.min(x.len());
    if b <= a {
        return -120.0;
    }
    10.0 * (x[a..b].iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / (b - a) as f64).max(1e-14).log10()
}

fn steps_of(env: &[f32], bar_start: f64, bar_len: f64) -> [f32; 16] {
    let mut out = [0.0f32; 16];
    for (k, o) in out.iter_mut().enumerate() {
        let t = bar_start + k as f64 * bar_len / 16.0;
        let c = (t * SRF / HOP as f64) as i64;
        let w = ((0.015 * SRF) / HOP as f64).ceil() as i64;
        *o = ((c - w)..=(c + w))
            .filter(|i| *i >= 0 && (*i as usize) < env.len())
            .map(|i| env[i as usize])
            .fold(0.0, f32::max);
    }
    out
}

fn template(avg: &[f32; 16]) -> Vec<f32> {
    let mx = avg.iter().cloned().fold(0.0f32, f32::max).max(1e-9);
    avg.iter()
        .map(|v| {
            let s = v / mx;
            if s >= 0.85 { 1.0 } else { ((s - 0.25) / 0.6).clamp(0.0, 0.95) }
        })
        .collect()
}

pub fn analyze(x: &Stereo, bpm_range: Option<(f64, f64)>) -> Analysis {
    let mono = x.mono();
    let grid = beats::track(&mono, bpm_range);
    let beat = 60.0 / grid.bpm.max(1.0);
    let bar_len = 4.0 * beat;
    let first = grid.beats.get(grid.downbeat).copied().unwrap_or(0.0);
    let mut start = first - (first / bar_len).floor() * bar_len;
    if start > bar_len - 0.06 {
        start = (start - bar_len).max(0.0);
    }
    let bars = (((mono.len() as f64 / SRF) - start) / bar_len).floor().max(1.0) as usize;
    let at = |b: usize| ((start + b as f64 * bar_len) * SRF) as usize;
    let t_bar = |b: usize| start + b as f64 * bar_len;
    let (low, high) = (band(&mono, 30.0, 150.0), band(&mono, 5000.0, 0.0));
    let kick_env = onsets::flux(&low, None);
    let snare_env = onsets::flux(&mono, Some((1000.0, 5000.0)));
    let hat_env = onsets::flux(&mono, Some((6000.0, 16000.0)));
    let chroma_hop = 4096;
    let ch = chroma::chroma(&mono, chroma_hop);
    let bass_ch = chroma::chroma_band(&mono, chroma_hop, 40.0, 220.0);
    let mut bar_bass: Vec<[f32; 12]> = Vec::with_capacity(bars);
    let mut rms = Vec::with_capacity(bars);
    let mut lo_db = Vec::with_capacity(bars);
    let mut hi_db = Vec::with_capacity(bars);
    let mut bar_chroma: Vec<[f32; 12]> = Vec::with_capacity(bars);
    let mut kick = Vec::with_capacity(bars);
    let mut snare = Vec::with_capacity(bars);
    let mut hat = Vec::with_capacity(bars);
    for b in 0..bars {
        let (a, e) = (at(b), at(b + 1));
        rms.push(rms_db(&mono, a, e));
        lo_db.push(rms_db(&low, a, e));
        hi_db.push(rms_db(&high, a, e));
        let (fa, fe) = (a / chroma_hop, (e / chroma_hop).min(ch.len()));
        let mut c = [0.0f32; 12];
        for f in ch.iter().take(fe).skip(fa) {
            for i in 0..12 {
                c[i] += f[i];
            }
        }
        let s: f32 = c.iter().sum::<f32>().max(1e-9);
        c.iter_mut().for_each(|v| *v /= s);
        bar_chroma.push(c);
        let mut cb = [0.0f32; 12];
        // the bass under the chord: second halves of the first three beats, where the kick has decayed (and a
        // sidechained bass swells back), before any walk-up to the next chord
        for (fi, f) in bass_ch.iter().enumerate().take(fe.min(bass_ch.len())).skip(fa) {
            let t = (fi * chroma_hop + chroma::NFFT / 2) as f64 / SRF - t_bar(b);
            let in_beat = (t / beat).fract();
            if (0.45..0.95).contains(&in_beat) && t < 3.0 * beat {
                for i in 0..12 {
                    cb[i] += f[i];
                }
            }
        }
        let sb: f32 = cb.iter().sum::<f32>().max(1e-9);
        cb.iter_mut().for_each(|v| *v /= sb);
        bar_bass.push(cb);
        let t0 = start + b as f64 * bar_len;
        kick.push(steps_of(&kick_env, t0, bar_len));
        snare.push(steps_of(&snare_env, t0, bar_len));
        hat.push(steps_of(&hat_env, t0, bar_len));
    }
    // key from the loudness-weighted average chroma
    let mut avg = [0.0f32; 12];
    for (b, c) in bar_chroma.iter().enumerate() {
        let w = 10f64.powf(rms[b] / 20.0) as f32;
        for i in 0..12 {
            avg[i] += c[i] * w;
        }
    }
    // key: of the closest profile fits, the one whose tonic chord opens the bars most and that explains the most
    // chords diatonically
    let cands = chroma::key_candidates(&avg, 4);
    let mut best_key = (cands[0].0, f64::MIN, cands[0].1);
    for (k, c) in &cands {
        let chords_k: Vec<Chord> =
            (0..bars).map(|b| chroma::chord_with_bass(&bar_chroma[b], &bar_bass[b], Some(k)).0).collect();
        let diatonic = chords_k.iter().filter(|ch| ch.pcs().iter().all(|p| k.pcs().contains(p))).count() as f64
            / bars.max(1) as f64;
        let tonic =
            chords_k.iter().filter(|ch| ch.root == k.tonic && ch.quality.is_minorish() == k.mode.is_minor()).count()
                as f64
                / bars.max(1) as f64;
        let bass_tonic = (0..bars).map(|b| bar_bass[b][k.tonic as usize] as f64).sum::<f64>() / bars.max(1) as f64;
        let s = c + 0.5 * diatonic + 1.5 * tonic + 0.5 * bass_tonic;
        if s > best_key.1 {
            best_key = (*k, s, *c);
        }
    }
    let (key, key_conf) = (best_key.0, best_key.2);
    let chords: Vec<Chord> =
        (0..bars).map(|b| chroma::chord_with_bass(&bar_chroma[b], &bar_bass[b], Some(&key)).0).collect();

    // sections: boundaries where 4-bar blocks differ most
    let block = 4usize;
    let nb = bars.div_ceil(block);
    let feat = |i: usize| -> Vec<f64> {
        let r = (i * block)..((i + 1) * block).min(bars);
        let m = |v: &[f64]| r.clone().map(|b| v[b]).sum::<f64>() / r.len().max(1) as f64;
        let mut f = vec![m(&rms) / 6.0, m(&lo_db) / 6.0, m(&hi_db) / 6.0];
        for k in 0..12 {
            f.push(r.clone().map(|b| bar_chroma[b][k] as f64).sum::<f64>() / r.len().max(1) as f64 * 8.0);
        }
        f
    };
    let feats: Vec<Vec<f64>> = (0..nb).map(feat).collect();
    let d: Vec<f64> =
        feats.windows(2).map(|w| w[0].iter().zip(&w[1]).map(|(a, b)| (a - b).powi(2)).sum::<f64>().sqrt()).collect();
    let mean = d.iter().sum::<f64>() / d.len().max(1) as f64;
    let sd = (d.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / d.len().max(1) as f64).sqrt();
    let mut bounds = vec![0usize];
    for (i, v) in d.iter().enumerate() {
        let b = (i + 1) * block;
        if *v > mean + 0.5 * sd && b - bounds.last().unwrap() >= 8 && bars - b >= 4 {
            bounds.push(b);
        }
    }
    bounds.push(bars);
    let sec_rms: Vec<f64> =
        bounds.windows(2).map(|w| (w[0]..w[1]).map(|b| rms[b]).sum::<f64>() / (w[1] - w[0]).max(1) as f64).collect();
    let (rmin, rmax) = sec_rms.iter().fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
    let mean_steps = |v: &Vec<[f32; 16]>, a: usize, b: usize| -> f32 {
        (a..b).map(|i| v[i].iter().sum::<f32>()).sum::<f32>() / ((b - a).max(1) * 16) as f32
    };
    let kick_ref = mean_steps(&kick, 0, bars).max(1e-9);
    let hat_ref = mean_steps(&hat, 0, bars).max(1e-9);
    let lo_ref = lo_db.iter().cloned().fold(f64::MIN, f64::max);
    let mut sections: Vec<SectionGuess> = Vec::new();
    for (i, w) in bounds.windows(2).enumerate() {
        let (a, b) = (w[0], w[1]);
        let e = if rmax - rmin > 1e-6 { ((sec_rms[i] - rmin) / (rmax - rmin)) as f32 } else { 0.8 };
        let lo_m = (a..b).map(|x| lo_db[x]).sum::<f64>() / (b - a).max(1) as f64;
        let kick_on = mean_steps(&kick, a, b) > 0.6 * kick_ref;
        sections.push(SectionGuess {
            start_bar: a,
            bars: b - a,
            energy: 0.3 + 0.7 * e,
            kind: SectionKind::Groove,
            kick: kick_on,
            bass: lo_m > lo_ref - 9.0,
            hats: mean_steps(&hat, a, b) > 0.5 * hat_ref,
            harmony: true,
        });
    }
    let n = sections.len();
    for i in 0..n {
        let e = sections[i].energy;
        let next_high = sections.get(i + 1).is_some_and(|s| s.energy >= 0.85);
        sections[i].kind = if i == 0 && e < 0.85 {
            SectionKind::Intro
        } else if i + 1 == n && n > 2 && e < 0.85 {
            SectionKind::Outro
        } else if e >= 0.85 {
            SectionKind::Drop
        } else if next_high && e >= 0.45 {
            SectionKind::Build
        } else if !sections[i].kick || e < 0.5 {
            SectionKind::Break
        } else {
            SectionKind::Groove
        };
    }
    // progressions: the repeating cycle of chords in the loudest sections, and in the quiet ones
    let cycle = |a: usize, b: usize| -> (Vec<Chord>, u32) {
        let cs: Vec<Chord> = chords[a..b].to_vec();
        let mut best = (cs.len().clamp(1, 4), -1.0f64);
        for l in [2usize, 4, 8] {
            if cs.len() < 2 * l {
                continue;
            }
            let agree = (0..cs.len() - l).filter(|&i| cs[i] == cs[i + l]).count() as f64 / (cs.len() - l) as f64;
            if agree > best.1 + 0.05 {
                best = (l, agree);
            }
        }
        let mut c: Vec<Chord> = cs.into_iter().take(best.0).collect();
        let bpc = if c.len() >= 2 && c.len().is_multiple_of(2) && c.chunks(2).all(|p| p[0] == p[1]) {
            c = c.chunks(2).map(|p| p[0]).collect();
            2
        } else {
            1
        };
        (c, bpc)
    };
    let loud = sections
        .iter()
        .filter(|s| s.kind == SectionKind::Drop)
        .max_by_key(|s| s.bars)
        .or_else(|| sections.iter().max_by(|a, b| a.energy.partial_cmp(&b.energy).unwrap()));
    let (ca, bpc) = loud.map_or((Vec::new(), 1), |s| cycle(s.start_bar, s.start_bar + s.bars));
    let quiet =
        sections.iter().filter(|s| matches!(s.kind, SectionKind::Break | SectionKind::Intro)).max_by_key(|s| s.bars);
    let (cb, _) = quiet.map_or((ca.clone(), bpc), |s| cycle(s.start_bar, s.start_bar + s.bars));
    let to_rn = |cs: &[Chord]| cs.iter().map(|c| roman_of(c, &key)).collect::<Vec<_>>();

    // drums and swing from the busiest sections
    let busy: Vec<usize> =
        sections.iter().filter(|s| s.kick && s.energy >= 0.6).flat_map(|s| s.start_bar..s.start_bar + s.bars).collect();
    let busy = if busy.is_empty() { (0..bars).collect() } else { busy };
    let avg_steps = |v: &Vec<[f32; 16]>| -> [f32; 16] {
        let mut a = [0.0f32; 16];
        for b in &busy {
            for k in 0..16 {
                a[k] += v[*b][k];
            }
        }
        a
    };
    let (ks, ss, hs) = (avg_steps(&kick), avg_steps(&snare), avg_steps(&hat));
    let mut swing_votes = Vec::new();
    for b in busy.iter().take(64) {
        let t0 = start + *b as f64 * bar_len;
        for pair in 0..8 {
            let p0 = t0 + pair as f64 * beat / 2.0;
            let (fa, fe) =
                (((p0 + beat * 0.2) * SRF / HOP as f64) as usize, ((p0 + beat * 0.4) * SRF / HOP as f64) as usize);
            if fe >= hat_env.len() || fe <= fa {
                continue;
            }
            let (i, v) = (fa..fe).map(|i| (i, hat_env[i])).fold((fa, 0.0f32), |m, x| if x.1 > m.1 { x } else { m });
            if v > 0.0 {
                let off = (i * HOP) as f64 / SRF - p0;
                swing_votes.push((2.0 * off / beat).clamp(0.5, 0.7));
            }
        }
    }
    swing_votes.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let swing = swing_votes.get(swing_votes.len() / 2).copied().unwrap_or(0.5) as f32;
    let swing = if swing < 0.53 { 0.5 } else { swing };
    // bass: offbeat eighths, sixteenth rolls, or long notes
    let on = (ks[0] + ks[4] + ks[8] + ks[12]) / 4.0;
    let off = (ks[2] + ks[6] + ks[10] + ks[14]) / 4.0;
    let busy16 = ks.iter().filter(|v| **v > 0.6 * ks.iter().cloned().fold(0.0, f32::max)).count();
    let bass_style = if busy16 >= 10 {
        "rolling"
    } else if off > 0.55 * on {
        "offbeat"
    } else if busy16 <= 2 {
        "sustain"
    } else {
        "syncopated"
    };

    Analysis {
        bpm: grid.bpm,
        key,
        key_confidence: (key_conf * 100.0).round() / 100.0,
        swing,
        pulse_clarity: (grid.clarity * 100.0).round() / 100.0,
        first_downbeat: (start * 1000.0).round() / 1000.0,
        bar_secs: bar_len,
        bars,
        chords: chords.iter().map(|c| c.symbol(key.flats())).collect(),
        progression_a: to_rn(&ca),
        progression_b: to_rn(&cb),
        bars_per_chord: bpc,
        sections,
        kick_steps: template(&ks),
        snare_steps: template(&ss),
        hat_steps: template(&hs),
        bass_style: bass_style.to_string(),
    }
}

/// a blueprint for genre `g` on the analysed song's skeleton (its tempo, key, swing, form, progressions, drum
/// habits and bass style); sounds and mix come from the genre
pub fn to_blueprint(
    a: &Analysis,
    g: &GenreProfile,
    seed: u64,
    opts: &Options,
    lib: Option<&dyn Library>,
    source: &str,
) -> Blueprint {
    let o = Options { bpm: Some(a.bpm.round()), key: Some(a.key), ..opts.clone() };
    let mut bp = compose::generate(g, seed, &o, lib);
    if !a.progression_a.is_empty() {
        bp.harmony.progressions.insert("A".into(), a.progression_a.clone());
        bp.harmony.progressions.insert(
            "B".into(),
            if a.progression_b.is_empty() { a.progression_a.clone() } else { a.progression_b.clone() },
        );
        bp.harmony.bars_per_chord = a.bars_per_chord;
    }
    bp.swing = a.swing;
    let drums: Vec<String> = bp.drums.iter().map(|d| d.name.clone()).collect();
    let parts: Vec<String> = bp.parts.iter().map(|p| p.name.clone()).collect();
    let loops: Vec<String> = bp.loops.iter().map(|l| l.name.clone()).collect();
    let has = |v: &Vec<String>, n: &str| v.iter().any(|x| x == n);
    let mut form = Vec::new();
    for (i, s) in a.sections.iter().enumerate() {
        let mut layers: Vec<String> = Vec::new();
        if s.kick && has(&drums, "kick") {
            layers.push("kick".into());
        }
        for d in &drums {
            let hat_like = matches!(d.as_str(), "hat" | "ohat" | "shaker" | "ride");
            let back = matches!(d.as_str(), "clap" | "snare");
            if (hat_like && s.hats) || (back && s.kick && s.energy >= 0.5) || (d == "perc" && s.energy >= 0.6) {
                layers.push(d.clone());
            }
        }
        if s.bass && has(&parts, "bass") && s.energy >= 0.45 {
            layers.push("bass".into());
        }
        for p in &parts {
            let take = match p.as_str() {
                "chords" | "keys" => s.energy >= 0.4,
                "pad" => s.energy < 0.75 || s.kind == SectionKind::Break,
                "lead" => s.energy >= 0.8 || s.kind == SectionKind::Break,
                "arp" => s.energy >= 0.9,
                "acid" => s.energy >= 0.9,
                _ => false,
            };
            if take && !layers.contains(p) {
                layers.push(p.clone());
            }
        }
        for l in &loops {
            if s.energy >= 0.6 {
                layers.push(l.clone());
            }
        }
        if layers.is_empty() {
            layers.push(parts.first().cloned().unwrap_or_else(|| "kick".into()));
        }
        let bars = ((s.bars as f64 / 4.0).round().max(1.0) * 4.0) as u32;
        let next_drop = a.sections.get(i + 1).is_some_and(|n| n.kind == SectionKind::Drop);
        form.push(SectionSpec {
            name: format!("{}{}", s.kind.name(), i + 1),
            kind: s.kind,
            bars,
            energy: s.energy,
            prog: if matches!(s.kind, SectionKind::Break | SectionKind::Intro | SectionKind::Outro) {
                "B".into()
            } else {
                "A".into()
            },
            layers,
            variant: if s.energy < 0.5 { "sparse".into() } else { "main".into() },
            filter_in: s.kind == SectionKind::Intro,
            build: s.kind == SectionKind::Build || (next_drop && s.kind != SectionKind::Drop),
            gap: next_drop && s.kind != SectionKind::Drop,
            impact: s.kind == SectionKind::Drop && i > 0 && a.sections[i - 1].kind != SectionKind::Drop,
            fills: matches!(s.kind, SectionKind::Drop | SectionKind::Groove),
        });
    }
    if !form.is_empty() {
        bp.form = form;
    }
    let mut r = crate::rng::Rng::new(seed ^ 0xa11a);
    for (lane, steps) in
        [("kick", &a.kick_steps), ("clap", &a.snare_steps), ("snare", &a.snare_steps), ("hat", &a.hat_steps)]
    {
        if let Some(d) = bp.drums.iter_mut().find(|d| d.name == lane)
            && steps.iter().any(|v| *v > 0.0)
        {
            let t = LaneTemplate { steps: steps.clone(), vel: (0.7, 1.0), ghost: 0.0, rolls: 0.0, vary: 0.2 };
            d.patterns.insert("main".into(), rhythm::generate(&t, d.pattern_bars, &mut r));
        }
    }
    if let Some(bass) = bp.parts.iter_mut().find(|p| p.name == "bass")
        && g.parts.iter().any(|p| p.name == "bass" && p.styles.contains(&a.bass_style))
    {
        bass.style = a.bass_style.clone();
    }
    bp.lineage = vec![format!(
        "remade from {source}: {:.0} BPM, {}, {} sections, progression {}",
        a.bpm,
        a.key.name(),
        a.sections.len(),
        a.progression_a.join(" ")
    )];
    bp
}

//! The critic: is this song any good? Three kinds of evidence, each scored 0..1 with the problems it found in
//! words and a hint for which part of the blueprint to change:
//!   mix      meters on the master: loudness vs target, true peak, tonal balance per band, stereo width, mono low end
//!   arc      the energy arc: section loudness follows the arrangement's energy, drops clearly louder than breaks
//!   theory   the score itself: melody in key (or in its chord), chord tones on strong beats, singable range and
//!            leaps, bass on the roots, smooth voice leading, form
//!   ears     (when earshot's models are available) genre fit against named confusions, production quality
//!   time     (when recordings play on its tracks: loops, takes, a remake's pieces) each keeps the song's tempo and
//!            beat, as its own beats say
//! The total is a weighted mean (0..100), cut to as little as `OUT_OF_TIME` of it when a recording plays out of the
//! song's time, which no mix or genre makes up for. Held-out evidence (AudioSet tags) is reported, never optimised.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::audio::Stereo;
use crate::config::SRF;
use crate::dsp::fft::spectrogram;
use crate::dsp::filter;
use crate::genres::GenreProfile;
use crate::loudness;
use crate::model::{Project, SectionKind};
use crate::render::Rendered;
use crate::theory::chord_at;

#[derive(Clone, Debug, Serialize)]
pub struct Issue {
    /// 0..1, how much it costs
    pub severity: f64,
    pub what: String,
    /// where a fix lives: "master", "band:<band>+/-", "width+/-", "level:<track>+/-", "form", "form:long/short",
    /// "form:intro", "form:same", "form:section", "form:peaks-/+", "harmony", "part:<name>", "sound:<part>", "genre",
    /// "production"
    pub target: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct EarsReport {
    pub genre_query: String,
    pub genre_z: f64,
    pub best_alt: String,
    pub best_alt_z: f64,
    /// the strongest reading of stock library music the genre does not count among its confusions (`ears::STOCK`)
    pub stock: String,
    pub stock_z: f64,
    pub production_z: f64,
    pub worst_flaw: String,
    pub worst_flaw_z: f64,
    pub sounds_like: Vec<String>,
    pub p_real: Option<f64>,
    /// held out: AudioSet tags of the master (top labels), not part of the score
    pub audioset: Vec<(String, f32)>,
    /// typicality vs reference tracks (z, >= -1 fits the set), when references were given
    pub ref_typicality_z: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Verdict {
    pub total: f64,
    pub scores: BTreeMap<String, f64>,
    pub measures: BTreeMap<String, f64>,
    pub issues: Vec<Issue>,
    pub ears: Option<EarsReport>,
    pub summary: String,
}

/// each section's loudness (its median short-term LUFS), in the arrangement's order, from a verdict's measures
pub fn section_loudness(measures: &BTreeMap<String, f64>) -> Vec<(String, f64)> {
    measures
        .iter()
        .filter_map(|(k, v)| {
            let name = k.strip_prefix("section_")?.strip_suffix("_lufs")?;
            Some((name.split_once('_').map_or(name, |(_, n)| n).to_string(), *v))
        })
        .collect()
}

pub const BANDS: [(&str, f64, f64); 6] = [
    ("sub", 20.0, 60.0),
    ("low", 60.0, 250.0),
    ("lowmid", 250.0, 1000.0),
    ("mid", 1000.0, 4000.0),
    ("presence", 4000.0, 8000.0),
    ("air", 8000.0, 16000.0),
];

/// healthy range of each band's energy share (dB) for a modern electronic master: around a spectrum whose octave
/// energy is flat from 40 to 150 Hz and falls ~3 dB per octave above (sub -7, low -3, low-mid -8, mid -14,
/// presence -21.5, air -25)
const BAND_RANGE: [(f64, f64); 6] =
    [(-14.0, -4.0), (-7.0, -1.0), (-12.0, -4.0), (-18.0, -8.0), (-26.0, -15.0), (-30.0, -17.0)];

/// energy share (dB) of each band in the averaged power spectrum (Welch, 8192-point frames)
pub fn band_shares(x: &Stereo) -> Vec<f64> {
    let n_fft = 8192;
    let spec = spectrogram(&x.mono(), n_fft, n_fft);
    let mut psd = vec![0.0f64; n_fft / 2 + 1];
    for frame in &spec {
        for (p, v) in psd.iter_mut().zip(frame) {
            *p += (*v as f64).powi(2);
        }
    }
    let hz = |k: usize| k as f64 * SRF / n_fft as f64;
    let total: f64 = psd.iter().enumerate().filter(|(k, _)| hz(*k) >= 20.0).map(|(_, p)| p).sum::<f64>().max(1e-20);
    BANDS
        .iter()
        .map(|(_, lo, hi)| {
            let e: f64 = psd.iter().enumerate().filter(|(k, _)| hz(*k) >= *lo && hz(*k) < *hi).map(|(_, p)| p).sum();
            10.0 * (e / total).max(1e-12).log10()
        })
        .collect()
}

fn correlation(a: &[f32], b: &[f32]) -> f64 {
    let n = a.len().min(b.len()).max(1);
    let (ma, mb) =
        (a.iter().map(|v| *v as f64).sum::<f64>() / n as f64, b.iter().map(|v| *v as f64).sum::<f64>() / n as f64);
    let (mut sab, mut saa, mut sbb) = (0.0, 0.0, 0.0);
    for i in 0..n {
        let (x, y) = (a[i] as f64 - ma, b[i] as f64 - mb);
        sab += x * y;
        saa += x * x;
        sbb += y * y;
    }
    sab / (saa.sqrt() * sbb.sqrt()).max(1e-12)
}

fn clamp01(v: f64) -> f64 {
    v.clamp(0.0, 1.0)
}

fn mix_checks(
    r: &Rendered,
    p: &Project,
    g: Option<&GenreProfile>,
    m: &mut BTreeMap<String, f64>,
    issues: &mut Vec<Issue>,
) -> f64 {
    let x = &r.master;
    let mut score = 1.0;
    let lufs_err = (r.lufs - p.mix.master.lufs as f64).abs();
    m.insert("lufs".into(), r.lufs);
    m.insert("true_peak_db".into(), r.true_peak_db as f64);
    if lufs_err > 1.0 {
        score -= 0.1;
        issues.push(Issue {
            severity: 0.3,
            what: format!("loudness {:.1} LUFS, target {:.1}", r.lufs, p.mix.master.lufs),
            target: "master".into(),
        });
    }
    if r.true_peak_db > p.mix.master.ceiling_db + 0.1 {
        score -= 0.15;
        issues.push(Issue {
            severity: 0.5,
            what: format!("true peak {:.1} dBTP over the {:.1} ceiling", r.true_peak_db, p.mix.master.ceiling_db),
            target: "master".into(),
        });
    }
    if let Some(lra) = loudness::lra(x) {
        m.insert("lra_lu".into(), lra);
    }
    let shares = band_shares(x);
    let advice = [
        (
            "sub",
            "too much sub (boomy): lower the bass / kick, or highpass the music bus",
            "not enough sub: the kick and bass lack weight",
        ),
        ("low", "the low end is too heavy (kick + bass)", "a thin low end: bass or kick too quiet"),
        ("lowmid", "muddy 250 Hz-1 kHz: too much pad / chord body", "hollow low mids: chords and pads are missing"),
        ("mid", "boxy or harsh mids", "recessed mids: the lead and chords are buried"),
        ("presence", "harsh 4-8 kHz", "dull 4-8 kHz: hats, claps and snare lack bite"),
        ("air", "fizzy top end", "no air above 8 kHz: hats and cymbals too quiet or too dark"),
    ];
    for (i, (name, _, _)) in BANDS.iter().enumerate() {
        let v = shares[i];
        m.insert(format!("band_{name}_db"), (v * 10.0).round() / 10.0);
        let (a, b) = p.mix.bands.get(*name).map_or(BAND_RANGE[i], |r| (f64::from(r[0]), f64::from(r[1])));
        let (over, under) = (v - b, a - v);
        // how far out of its range it is, which a quick fix moves it by (`crate::fixes`)
        if over > 0.0 || under > 0.0 {
            m.insert(format!("band_{name}_off_db"), if over > 0.0 { over } else { -under });
        }
        if over > 0.0 {
            let sev = clamp01(over / 6.0);
            score -= 0.12 * sev;
            issues.push(Issue {
                severity: sev,
                what: format!("{} ({name} {v:.1} dB, healthy up to {b:.0})", advice[i].1),
                target: format!("band:{name}+"),
            });
        } else if under > 0.0 {
            let sev = clamp01(under / 6.0);
            score -= 0.12 * sev;
            issues.push(Issue {
                severity: sev,
                what: format!("{} ({name} {v:.1} dB, healthy from {a:.0})", advice[i].2),
                target: format!("band:{name}-"),
            });
        }
    }
    let corr = correlation(&x.l, &x.r);
    let mut lo_l = x.l.clone();
    let mut lo_r = x.r.clone();
    filter::butter(&mut lo_l, false, 150.0, SRF, 4);
    filter::butter(&mut lo_r, false, 150.0, SRF, 4);
    let low_corr = correlation(&lo_l, &lo_r);
    m.insert("stereo_correlation".into(), (corr * 1000.0).round() / 1000.0);
    m.insert("low_end_correlation".into(), (low_corr * 1000.0).round() / 1000.0);
    if corr > 0.96 {
        score -= 0.08;
        issues.push(Issue {
            severity: 0.35,
            what: format!("nearly mono (correlation {corr:.2}): widen pads, chords and effects"),
            target: "width+".into(),
        });
    } else if corr < 0.15 {
        score -= 0.1;
        issues.push(Issue {
            severity: 0.4,
            what: format!("too wide / phasey (correlation {corr:.2})"),
            target: "width-".into(),
        });
    }
    if low_corr < 0.9 {
        score -= 0.08;
        issues.push(Issue {
            severity: 0.4,
            what: format!("low end not mono (correlation {low_corr:.2} below 150 Hz)"),
            target: "width-".into(),
        });
    }
    // each track at the level its genre makes it at (its role's target, the genre's offset for the role, and its
    // own, as a clap under the snare): engineers set a lead about 1.5 dB apart from one mix to another, so 3 dB
    // either way is a choice, more a fault
    for t in &r.tracks {
        let Some(level) = t.level_lufs else { continue };
        let made = g.map_or(0.0, |g| crate::compose::made_level(g, &t.name));
        let meant = crate::render::target_level(&t.role) + p.mix.levels.get(&t.role).copied().unwrap_or(0.0) + made;
        let off = level + f64::from(t.gain_db) - f64::from(meant);
        if off.abs() <= 3.0 {
            continue;
        }
        let sev = clamp01((off.abs() - 3.0) / 6.0);
        score -= 0.2 * sev;
        m.insert(format!("level_{}_off_db", t.name), off);
        let (how, so, sign) =
            if off > 0.0 { ("louder", "it covers the rest", '+') } else { ("quieter", "the rest covers it", '-') };
        issues.push(Issue {
            severity: sev,
            what: format!("{} is {:.1} dB {how} than the {} level: {so}", t.name, off.abs(), t.role),
            target: format!("level:{}{sign}", t.name),
        });
    }
    clamp01(score)
}

fn arc_checks(r: &Rendered, p: &Project, m: &mut BTreeMap<String, f64>, issues: &mut Vec<Issue>) -> f64 {
    let spb = SRF * 60.0 / p.bpm;
    let st = loudness::blocks(&r.master, 3.0, 0.5);
    let mut lv: Vec<(SectionKind, f64, f64)> = Vec::new();
    for (i, s) in p.sections.iter().enumerate() {
        let (a, b) = (s.start_bar as f64 * 4.0 * spb / SRF, (s.start_bar + s.bars) as f64 * 4.0 * spb / SRF);
        let mut vals: Vec<f64> = st.iter().filter(|(t, _)| *t >= a + 1.5 && *t < b - 1.0).map(|(_, l)| *l).collect();
        if vals.is_empty() {
            continue;
        }
        vals.sort_by(|x, y| x.partial_cmp(y).unwrap());
        let median = vals[vals.len() / 2];
        // numbered so that they sort in the arrangement's order
        m.insert(format!("section_{:02}_{}_lufs", i + 1, s.name), (median * 10.0).round() / 10.0);
        lv.push((s.kind, s.energy as f64, median));
    }
    if lv.len() < 2 {
        return 1.0;
    }
    let e: Vec<f32> = lv.iter().map(|x| x.1 as f32).collect();
    let l: Vec<f32> = lv.iter().map(|x| x.2 as f32).collect();
    let c = correlation(&e, &l);
    m.insert("arc_correlation".into(), (c * 100.0).round() / 100.0);
    let mut score = clamp01(0.5 + 0.5 * c);
    let peak = lv
        .iter()
        .filter(|x| matches!(x.0, SectionKind::Drop | SectionKind::Hook))
        .map(|x| x.2)
        .fold(f64::MIN, f64::max);
    let low = lv.iter().filter(|x| matches!(x.0, SectionKind::Break)).map(|x| x.2).fold(f64::MAX, f64::min);
    if peak > f64::MIN && low < f64::MAX {
        let d = peak - low;
        m.insert("drop_minus_break_lu".into(), (d * 10.0).round() / 10.0);
        if d < 3.0 {
            let sev = clamp01((3.0 - d) / 3.0);
            score -= 0.3 * sev;
            issues.push(Issue { severity: sev, what: format!("the drop is only {d:.1} LU louder than the breakdown (aim for 3-7): thin the break or push the drop"), target: "form".into() });
        }
    }
    if c < 0.5 {
        issues.push(Issue {
            severity: clamp01(0.5 - c),
            what: format!("loudness does not follow the arrangement's energy (correlation {c:.2})"),
            target: "form".into(),
        });
    }
    clamp01(score)
}

fn theory_checks(p: &Project, g: Option<&GenreProfile>, m: &mut BTreeMap<String, f64>, issues: &mut Vec<Issue>) -> f64 {
    let key = p.key;
    let mut parts: Vec<f64> = Vec::new();
    if let Some(lead) = p.tracks.iter().find(|t| t.role == "lead") {
        let notes = &lead.notes;
        if !notes.is_empty() {
            // a note outside the key is in place when the chord sounding with it has it: a borrowed chord's third, an
            // applied dominant's leading note
            let in_key = notes
                .iter()
                .filter(|n| {
                    key.contains(n.p)
                        || chord_at(&p.chords, n.t).is_some_and(|c| c.chord.has_pc(n.p.rem_euclid(12) as u8))
                })
                .count() as f64
                / notes.len() as f64;
            let strong: Vec<_> = notes.iter().filter(|n| n.t.fract() < 1e-6 && (n.t as i64) % 2 == 0).collect();
            let chord_tone = if strong.is_empty() {
                1.0
            } else {
                strong
                    .iter()
                    .filter(|n| chord_at(&p.chords, n.t).is_some_and(|c| c.chord.has_pc(n.p.rem_euclid(12) as u8)))
                    .count() as f64
                    / strong.len() as f64
            };
            let lo = notes.iter().map(|n| n.p).min().unwrap_or(0);
            let hi = notes.iter().map(|n| n.p).max().unwrap_or(0);
            let mut sorted = notes.clone();
            sorted.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap());
            let iv: Vec<i32> =
                sorted.windows(2).filter(|w| w[1].t - w[0].t < 2.0).map(|w| (w[1].p - w[0].p).abs()).collect();
            let steps = iv.iter().filter(|d| **d <= 2).count() as f64 / iv.len().max(1) as f64;
            let leaps = iv.iter().filter(|d| **d > 7).count() as f64 / iv.len().max(1) as f64;
            m.insert("lead_in_key".into(), (in_key * 100.0).round() / 100.0);
            m.insert("lead_chord_tones_on_strong_beats".into(), (chord_tone * 100.0).round() / 100.0);
            m.insert("lead_range_semitones".into(), (hi - lo) as f64);
            m.insert("lead_stepwise".into(), (steps * 100.0).round() / 100.0);
            let mut s = 1.0;
            if in_key < 0.95 {
                s -= (0.95 - in_key) * 3.0;
                issues.push(Issue {
                    severity: clamp01((0.95 - in_key) * 4.0),
                    what: format!("{:.0}% of the lead is outside {}", (1.0 - in_key) * 100.0, key.name()),
                    target: "part:lead".into(),
                });
            }
            if chord_tone < 0.6 {
                s -= (0.6 - chord_tone) * 1.2;
                issues.push(Issue {
                    severity: clamp01(0.6 - chord_tone),
                    what: format!("only {:.0}% of the lead's strong-beat notes are chord tones", chord_tone * 100.0),
                    target: "part:lead".into(),
                });
            }
            if hi - lo > 19 {
                s -= 0.15;
                issues.push(Issue {
                    severity: 0.3,
                    what: format!("the lead spans {} semitones (hard to hear as one line)", hi - lo),
                    target: "part:lead".into(),
                });
            }
            if leaps > 0.2 {
                s -= 0.15;
                issues.push(Issue {
                    severity: 0.3,
                    what: format!("{:.0}% of the lead's moves are leaps over a fifth", leaps * 100.0),
                    target: "part:lead".into(),
                });
            }
            parts.push(clamp01(s));
        }
    }
    if let Some(bass) = p.tracks.iter().find(|t| t.role == "bass") {
        let on_change: Vec<_> =
            bass.notes.iter().filter(|n| p.chords.iter().any(|c| (c.start - n.t).abs() < 1e-6)).collect();
        if !on_change.is_empty() {
            let roots = on_change
                .iter()
                .filter(|n| chord_at(&p.chords, n.t).is_some_and(|c| c.chord.bass_pc() == n.p.rem_euclid(12) as u8))
                .count() as f64
                / on_change.len() as f64;
            m.insert("bass_roots_on_changes".into(), (roots * 100.0).round() / 100.0);
            if roots < 0.7 {
                issues.push(Issue {
                    severity: clamp01(0.7 - roots),
                    what: format!("the bass plays the chord root on only {:.0}% of chord changes", roots * 100.0),
                    target: "part:bass".into(),
                });
            }
            parts.push(clamp01(roots / 0.7));
        }
    }
    for t in p.tracks.iter().filter(|t| matches!(t.role.as_str(), "chords" | "pad" | "keys")) {
        let mut groups: BTreeMap<i64, Vec<i32>> = BTreeMap::new();
        for n in &t.notes {
            groups.entry((n.t * 4.0).round() as i64).or_default().push(n.p);
        }
        let v: Vec<Vec<i32>> = groups
            .into_values()
            .map(|mut g| {
                g.sort();
                g
            })
            .collect();
        let moves: Vec<f64> = v
            .windows(2)
            .filter(|w| w[0].len() == w[1].len() && w[0] != w[1])
            .map(|w| w[0].iter().zip(&w[1]).map(|(a, b)| (a - b).abs() as f64).sum::<f64>() / w[0].len() as f64)
            .collect();
        if !moves.is_empty() {
            let avg = moves.iter().sum::<f64>() / moves.len() as f64;
            m.insert(format!("voice_leading_{}", t.name), (avg * 10.0).round() / 10.0);
            if avg > 4.5 {
                issues.push(Issue {
                    severity: clamp01((avg - 4.5) / 4.0),
                    what: format!("{}: voices jump {avg:.1} semitones per chord change on average", t.name),
                    target: format!("part:{}", t.name),
                });
            }
            parts.push(clamp01(1.0 - (avg - 3.0).max(0.0) / 6.0));
        }
    }
    parts.push(form_checks(p, g, m, issues));
    // each part plays a sound its genre has a place for, as the genre's own draws would (`compose::has_place`), each
    // sound of a rack too: one that does not is a check failed outright
    if let Some(g) = g {
        for t in &p.tracks {
            let Some(role) = crate::model::Role::parse(&t.role) else { continue };
            let Some(out) = crate::compose::out_of_place(g, role, &t.sound) else { continue };
            parts.push(0.0);
            issues.push(Issue {
                severity: 0.5,
                what: format!(
                    "{} plays {}, which {} has no place for",
                    t.name,
                    out.describe(),
                    g.name.replace('_', " ")
                ),
                target: format!("sound:{}", t.name),
            });
        }
    }
    if parts.is_empty() { 1.0 } else { parts.iter().sum::<f64>() / parts.len() as f64 }
}

/// What a genre's own forms say a song of it is (`GenreProfile::forms`, at its tempos): how long it runs, how long its
/// intro is at most, how long its longest section, and how many times its hook or drop comes.
struct Norms {
    secs: (f64, f64),
    intro_bars: u32,
    longest_bars: u32,
    peaks: (usize, usize),
}

fn norms(g: &GenreProfile) -> Option<Norms> {
    let forms = &g.forms;
    let bars = |f: &Vec<crate::model::SectionSpec>| f.iter().map(|s| s.bars).sum::<u32>() as f64;
    let peaks = |f: &Vec<crate::model::SectionSpec>| {
        f.iter().filter(|s| matches!(s.kind, SectionKind::Drop | SectionKind::Hook)).count()
    };
    Some(Norms {
        secs: (
            forms.iter().map(bars).reduce(f64::min)? * 240.0 / g.bpm.1.max(1.0),
            forms.iter().map(bars).reduce(f64::max)? * 240.0 / g.bpm.0.max(1.0),
        ),
        intro_bars: forms
            .iter()
            .filter_map(|f| f.first().filter(|s| s.kind == SectionKind::Intro).map(|s| s.bars))
            .max()
            .unwrap_or(0),
        longest_bars: forms.iter().flatten().map(|s| s.bars).max()?,
        peaks: (forms.iter().map(peaks).min()?, forms.iter().map(peaks).max()?),
    })
}

/// what the critic finds in a song's form alone, as [`judge`] reports it, which needs no render
pub fn form_issues(p: &Project) -> Vec<Issue> {
    let mut issues = Vec::new();
    form_checks(p, crate::genres::get(&p.genre).ok().as_ref(), &mut BTreeMap::new(), &mut issues);
    issues
}

/// minutes and seconds
fn mmss(secs: f64) -> String {
    let s = secs.round() as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

/// What a listener hears a section change by (docs/src/engineering/critic.md): the tracks that play in it (that start
/// a note or a clip in it, or sound through half of it, as a take does through the sections after its own), where in
/// the bar each drum strikes, and its chords; not the transitions, which mark where one ends. Two neighbours with the
/// same sound as one section, whatever their notes.
type Texture = (Vec<usize>, Vec<(usize, i64)>, Vec<(i64, crate::theory::chord::Chord)>);

fn texture(p: &Project, s: &crate::model::SectionInfo) -> Texture {
    let (from, to) = (s.start_bar as f64 * 4.0, (s.start_bar + s.bars) as f64 * 4.0);
    let within = |t: f64| t >= from - 1e-9 && t < to - 1e-9;
    let sounds = |t: f64, d: f64| within(t) || (t + d.max(0.0)).min(to) - t.max(from) >= 0.5 * (to - from);
    let sixteenth = |t: f64| ((t - from) * 4.0).round() as i64;
    let (mut playing, mut strikes) = (Vec::new(), std::collections::BTreeSet::new());
    for (i, t) in p.tracks.iter().enumerate() {
        let transition = matches!(t.role.as_str(), "crash" | "riser" | "impact" | "downlifter" | "reverse");
        if transition || (!t.notes.iter().any(|n| sounds(n.t, n.d)) && !t.clips.iter().any(|c| sounds(c.t, c.d))) {
            continue;
        }
        playing.push(i);
        if crate::render::is_drum_role(&t.role) {
            strikes.extend(t.notes.iter().filter(|n| within(n.t)).map(|n| (i, sixteenth(n.t).rem_euclid(16))));
        }
    }
    let chords = p.chords.iter().filter(|c| within(c.start)).map(|c| (sixteenth(c.start), c.chord)).collect();
    (playing, strikes.into_iter().collect(), chords)
}

/// The form against what listeners hear in one and against its genre's own forms (docs/src/engineering/critic.md
/// says why each is what it is): its length within what the genre's forms run, its music under way before twice its
/// genre's longest intro, its sections whole 4-bar phrases, nothing heard as one section (neighbours alike together)
/// much longer than the genre's longest or 32 bars, its hook coming back, its drop if its genre drops, and neither
/// coming far more often than the genre's forms bring theirs. A song without a genre is held to 90 s to 5:30, and to
/// its phrases.
fn form_checks(p: &Project, g: Option<&GenreProfile>, m: &mut BTreeMap<String, f64>, issues: &mut Vec<Issue>) -> f64 {
    let secs = p.secs();
    m.insert("length_s".into(), secs.round());
    let norms = g.and_then(norms);
    let genre = g.map_or(String::new(), |g| g.name.replace('_', " "));
    let mut form = 1.0;
    let (lo, hi, runs) = match &norms {
        Some(n) => {
            // the forms are the genre's shapes, and its records run shorter and longer than them; 5:30 is long for
            // none of them
            let (lo, hi) = (n.secs.0 / 1.5, (1.5 * n.secs.1).max(330.0));
            (lo, hi, format!("where {genre}'s forms run {}-{}", mmss(n.secs.0), mmss(n.secs.1)))
        }
        None => (90.0, 330.0, "where most tracks run 2-5 minutes".to_string()),
    };
    if secs < lo || secs > hi {
        let off = clamp01(if secs < lo { (lo - secs) / lo } else { (secs - hi) / hi } * 2.0);
        form -= 0.3 * (0.4 + 0.6 * off);
        issues.push(Issue {
            severity: 0.3 + 0.4 * off,
            what: format!("{} long, {runs}", mmss(secs)),
            target: if secs < lo { "form:short" } else { "form:long" }.into(),
        });
    }
    if p.sections.iter().any(|s| s.bars % 4 != 0) {
        form -= 0.2;
        issues.push(Issue {
            severity: 0.3,
            what: "a section is not a whole number of 4-bar phrases".into(),
            target: "form".into(),
        });
    }
    let Some(n) = norms.filter(|_| !p.sections.is_empty()) else { return clamp01(form) };
    // under way: the first section past the intros, however quiet (a soft verse is the song begun)
    if let Some(s) = p.sections.iter().find(|s| s.kind != SectionKind::Intro) {
        let at = s.start_bar as f64 * 240.0 / p.bpm.max(1.0);
        m.insert("intro_s".into(), at.round());
        let most = (2 * n.intro_bars).max(16);
        if s.start_bar > most {
            let over = clamp01((s.start_bar - most) as f64 / most as f64);
            form -= 0.2 * (0.5 + 0.5 * over);
            issues.push(Issue {
                severity: 0.25 + 0.3 * over,
                what: format!(
                    "the song takes {} ({} bars) to get under way, where {genre}'s intros run {} bars at most",
                    mmss(at),
                    s.start_bar,
                    n.intro_bars
                ),
                target: "form:intro".into(),
            });
        }
    }
    // what a listener hears as one section, neighbours alike running together: none much longer than the genre's
    // longest, nor than the 32 bars that dance music changes a layer within
    let textures: Vec<Texture> = p.sections.iter().map(|s| texture(p, s)).collect();
    let most = (n.longest_bars * 3 / 2).max(32);
    let (mut heard, mut alike) = (Vec::new(), 0);
    let mut i = 0;
    while i < p.sections.len() {
        let j = (i + 1..p.sections.len()).take_while(|&j| textures[j] == textures[i]).last().unwrap_or(i);
        let bars: u32 = p.sections[i..=j].iter().map(|s| s.bars).sum();
        alike += j - i;
        if bars > most {
            heard.push((i, j, bars));
        }
        i = j + 1;
    }
    m.insert("sections_alike".into(), alike as f64);
    if let Some(&(i, j, bars)) = heard.iter().max_by_key(|h| h.2) {
        form -= (0.15 * heard.len() as f64).min(0.3);
        let (a, b) = (&p.sections[i].name, &p.sections[j].name);
        issues.push(if i == j {
            Issue {
                severity: 0.25,
                what: format!(
                    "{a} plays {bars} bars, where {genre}'s longest section is {}: shorten it, or let something \
                     change in it",
                    n.longest_bars
                ),
                target: "form:section".into(),
            }
        } else {
            let which = if j == i + 1 { format!("{a} and {b}") } else { format!("{a} to {b}") };
            Issue {
                severity: 0.3,
                what: format!(
                    "{which} play alike, {bars} bars heard as one section where {genre}'s longest is {}: change a \
                     part, the drums or the chords between them",
                    n.longest_bars
                ),
                target: "form:same".into(),
            }
        });
    }
    let peaks = p.sections.iter().filter(|s| matches!(s.kind, SectionKind::Drop | SectionKind::Hook)).count();
    m.insert("peaks".into(), peaks as f64);
    let word =
        if g.is_some_and(|g| g.forms.iter().flatten().any(|s| s.kind == SectionKind::Drop)) { "drop" } else { "hook" };
    // a hook heard again is what is remembered, and a drop is what a drop genre's listeners wait for; a song of
    // verses alone is a form of its own, in a hook genre
    let missing = match peaks {
        0 if word == "drop" => Some(format!("the song never drops, where {genre}'s forms do")),
        1 if n.peaks.0 >= 2 => Some(format!(
            "the {word} comes once, where {genre}'s forms bring it {} times: one that comes back is what listeners \
             remember",
            n.peaks.0
        )),
        _ => None,
    };
    if let Some(what) = missing.filter(|_| secs >= lo) {
        let severity = if peaks == 0 { 0.5 } else { 0.3 };
        form -= 0.4 * severity;
        issues.push(Issue { severity, what, target: "form:peaks-".into() });
    } else if peaks > n.peaks.1 + 2 {
        form -= 0.15;
        issues.push(Issue {
            severity: 0.25,
            what: format!("the {word} comes {peaks} times, where {genre}'s forms bring it {} at most", n.peaks.1),
            target: "form:peaks+".into(),
        });
    }
    clamp01(form)
}

/// one of the ears' flaws (`ears::FLAWS`) in a few words, and where it usually comes from in a song of claudius'
fn flaw_advice(flaw: &str) -> (&str, &'static str) {
    match flaw {
        "a cheap synthesized beep" => {
            ("a cheap synth", "sampled instruments in place of synth patches usually read better")
        }
        "a low quality recording with hiss" => {
            ("a hissy recording", "noise-built sounds (hats, shakers, risers) and long reverbs are the usual cause")
        }
        "a distorted clipped recording" => ("a clipped recording", "less drive and saturation, or a quieter master"),
        "a computer-generated electronic tone" => {
            ("a bare electronic tone", "sampled instruments in place of synth patches usually read better")
        }
        "a low bitrate mp3 with artifacts" => ("a low-bitrate MP3", "harsh or fizzy highs are the usual cause"),
        "a recording made on a phone" => ("a phone recording", "a thin or narrow mix is the usual cause"),
        other => (other, "fix the mix issues first"),
    }
}

/// How far the strongest reading of stock library music has caught up with the genre's: none until it is plausible
/// (z 2.5) and within one z of the genre, all once it draws level. The verdict loses up to `STOCK_CUT` of itself by it.
pub fn stock_share(e: &EarsReport) -> f64 {
    if e.stock.is_empty() || e.stock_z < 2.5 {
        return 0.0;
    }
    clamp01(e.stock_z - (e.genre_z - 1.0))
}

/// the share of the verdict a song that reads as much like stock music as like its genre loses
pub const STOCK_CUT: f64 = 0.4;

/// how tightly a recording's onsets must gather on some grid near the song's tempo for them to say how it keeps time
/// (`render::KeptTime::own`): a voice or a pad alone gathers on none, and is not judged on it
const PULSE: f64 = 0.15;

/// the share of its verdict a song keeps at most whose recording plays clearly out of its time: a sample at another
/// tempo, or drifting off the beat, against the song's drums, sounds wrong whatever its mix and its genre
pub const OUT_OF_TIME: f64 = 0.35;

/// Whether the recordings on the song's tracks keep its time (`render::KeptTime`): each whose onsets keep a grid keeps
/// the song's, gathering on its sixteenths half as tightly at least as on those of the tempo they keep best, and
/// does not wander off it by more than 40 ms (a band playing live, its bars each made one length, wanders 30). A
/// voice is not judged: its syllables keep no grid (a few of them, looped, line up by chance on a tempo whose
/// sixteenths fit the loop), and a sung phrase keeps the time the rest of its recording keeps, which is. 1 when all
/// do, less the further the worst is out; none when no recording keeps a grid clearly enough to tell.
fn time_checks(r: &Rendered, p: &Project, m: &mut BTreeMap<String, f64>, issues: &mut Vec<Issue>) -> Option<f64> {
    let mut score: Option<f64> = None;
    for t in r.tracks.iter().filter(|t| t.role != "vocal") {
        let Some(k) = t.time.as_ref().filter(|k| k.own >= PULSE) else { continue };
        m.insert(format!("time_{}_on_grid", t.name), k.on_grid);
        m.insert(format!("time_{}_own", t.name), k.own);
        m.insert(format!("time_{}_own_bpm", t.name), k.own_bpm);
        m.insert(format!("time_{}_drift_ms", t.name), k.drift_ms);
        // a clear grid of its own says it all, a faint one less
        let sure = clamp01((k.own - PULSE) / PULSE);
        let kept = k.on_grid / k.own.max(1e-9);
        let (severity, what) = if kept < 0.5 && (k.own_bpm / p.bpm).ln().abs() > 0.005 {
            let what = format!(
                "'{}' keeps a tempo of its own, about {:.0} BPM, against the song's {:.0}: the recording on it is off \
                 the song's time",
                t.name, k.own_bpm, p.bpm
            );
            (sure * clamp01((0.7 - kept) / 0.4), what)
        } else if k.drift_ms > 40.0 {
            let what = format!("'{}' drifts off the song's beat, by up to {:.0} ms", t.name, k.drift_ms);
            (sure * clamp01((k.drift_ms - 30.0) / 40.0), what)
        } else {
            score = Some(score.unwrap_or(1.0));
            continue;
        };
        score = Some(score.unwrap_or(1.0).min(1.0 - severity));
        issues.push(Issue { severity, what, target: format!("time:{}", t.name) });
    }
    score
}

fn ears_score(e: &EarsReport, issues: &mut Vec<Issue>) -> (f64, f64) {
    let margin = e.genre_z - e.best_alt_z;
    let genre = clamp01(0.5 * clamp01((e.genre_z - 1.0) / 2.5) + 0.5 * clamp01((margin + 1.0) / 2.0));
    let stock = stock_share(e);
    // stock music that wins outright is said by this issue, which then weighs as much as the verdict's cut
    let stock_won = margin < 0.0 && e.best_alt == e.stock;
    if e.genre_z < 2.5 || margin < 0.0 {
        issues.push(Issue {
            severity: clamp01((2.5 - e.genre_z).max(-margin) / 2.5).max(if stock_won { stock } else { 0.0 }),
            what: if margin < 0.0 {
                format!("sounds more like '{}' than '{}'", e.best_alt, e.genre_query)
            } else {
                format!("sounds only faintly like '{}' (the nearest other reading: '{}')", e.genre_query, e.best_alt)
            },
            target: "genre".into(),
        });
    }
    if stock > 0.0 && !stock_won {
        issues.push(Issue {
            severity: stock,
            what: format!(
                "sounds close to stock background music ('{}'): too clean and polite for '{}'",
                e.stock, e.genre_query
            ),
            target: "genre".into(),
        });
    }
    let lead = e.production_z - e.worst_flaw_z;
    let pq = clamp01(0.5 + lead / 4.0);
    if lead < 0.5 {
        let (flaw, cause) = flaw_advice(&e.worst_flaw);
        issues.push(Issue {
            // a near tie is a faint reading, so the issues that name a change come first
            severity: clamp01((0.5 - lead) / 2.5),
            what: if lead < -0.5 {
                format!("sounds more like {flaw} than a professional mix: {cause}")
            } else {
                format!("sounds about as much like {flaw} as a professional mix: {cause}")
            },
            target: "production".into(),
        });
    }
    (genre, pq)
}

/// judge a rendered project; `ears` comes from earshot when it is available
pub fn judge(p: &Project, r: &Rendered, ears: Option<EarsReport>) -> Verdict {
    judge_whole(p, p, r, ears)
}

/// `judge`, of a stretch of a song (`heard`, which `r` renders) that stands for the whole of it (`whole`): the notes
/// and the form read from the whole song, the mix, the arc and the time heard in the stretch. The evolution loop
/// renders a preview, and its changes to the form are judged all the same.
pub fn judge_whole(heard: &Project, whole: &Project, r: &Rendered, ears: Option<EarsReport>) -> Verdict {
    let p = heard;
    let mut measures = BTreeMap::new();
    let mut issues = Vec::new();
    let mut scores = BTreeMap::new();
    // the song's genre, as its parts and its levels were made (none for a file judged as it is)
    let g = crate::genres::get(&p.genre).ok();
    scores.insert("mix".to_string(), mix_checks(r, p, g.as_ref(), &mut measures, &mut issues));
    scores.insert("arc".to_string(), arc_checks(r, p, &mut measures, &mut issues));
    scores.insert("theory".to_string(), theory_checks(whole, g.as_ref(), &mut measures, &mut issues));
    let mut weights: Vec<(&str, f64)> = vec![("mix", 0.3), ("arc", 0.2), ("theory", 0.2)];
    if let Some(e) = &ears {
        let (g, pq) = ears_score(e, &mut issues);
        scores.insert("genre".to_string(), g);
        scores.insert("production".to_string(), pq);
        weights.extend([("genre", 0.15), ("production", 0.15)]);
        if let Some(z) = e.ref_typicality_z {
            scores.insert("refs".to_string(), clamp01((z + 3.0) / 4.0));
            weights.push(("refs", 0.15));
            if z < -1.0 {
                issues.push(Issue {
                    severity: clamp01((-1.0 - z) / 2.0),
                    what: format!("unlike the reference tracks (typicality z {z:+.1}; >= -1 fits them)"),
                    target: "genre".into(),
                });
            }
        }
    }
    let wsum: f64 = weights.iter().map(|w| w.1).sum();
    let mut total = 100.0 * weights.iter().map(|(k, w)| scores.get(*k).copied().unwrap_or(0.0) * w).sum::<f64>() / wsum;
    if let Some(e) = &ears {
        let stock = stock_share(e);
        measures.insert("stock_share".to_string(), stock);
        total *= 1.0 - STOCK_CUT * stock;
    }
    if let Some(time) = time_checks(r, p, &mut measures, &mut issues) {
        scores.insert("time".to_string(), time);
        total *= OUT_OF_TIME + (1.0 - OUT_OF_TIME) * time;
    }
    issues.sort_by(|a, b| b.severity.partial_cmp(&a.severity).unwrap());
    let summary = format!(
        "{:.0}/100 ({}){}",
        total,
        scores.iter().map(|(k, v)| format!("{k} {:.2}", v)).collect::<Vec<_>>().join(", "),
        if issues.is_empty() {
            String::new()
        } else {
            format!(". Fix first: {}", issues.iter().take(3).map(|i| i.what.clone()).collect::<Vec<_>>().join("; "))
        }
    );
    Verdict { total, scores, measures, issues, ears, summary }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn in_form(g: &GenreProfile, form: &[crate::model::SectionSpec]) -> Project {
        let mut bp = crate::compose::generate(g, 1, &Default::default(), None);
        bp.form = form.to_vec();
        bp.bpm = (g.bpm.0 + g.bpm.1) / 2.0;
        crate::compose::arrange(&bp)
    }

    fn form_targets(p: &Project) -> Vec<String> {
        form_issues(p).into_iter().map(|i| i.target).collect()
    }

    #[test]
    fn a_song_in_one_of_its_genres_own_forms_has_nothing_wrong_with_its_form() {
        let mut found = Vec::new();
        for g in crate::genres::all().unwrap() {
            for f in &g.forms {
                found.extend(form_issues(&in_form(&g, f)).into_iter().map(|i| format!("{}: {}", g.name, i.what)));
            }
        }
        assert!(found.is_empty(), "{}", found.join("\n"));
    }

    #[test]
    fn a_take_plays_in_every_section_it_sounds_through() {
        let g = crate::genres::get("synthwave").unwrap();
        let mut p = in_form(&g, &g.forms[0]);
        let i = p.tracks.iter().position(|t| t.role == "pad").unwrap();
        let take = crate::model::Clip {
            t: 0.0,
            d: p.beats(),
            offset: 0.0,
            gain_db: 0.0,
            fade_in: 0.0,
            fade_out: 0.0,
            once: true,
        };
        (p.tracks[i].notes, p.tracks[i].clips) = (Vec::new(), vec![take]);
        assert!(p.sections.iter().all(|s| texture(&p, s).0.contains(&i)));
    }

    #[test]
    fn the_form_checks_hear_a_short_song_a_long_intro_a_section_twice_and_a_hook_heard_once() {
        let g = crate::genres::get("synthwave").unwrap();
        let form = g.forms[0].clone();
        assert!(form_targets(&in_form(&g, &form[..2])).contains(&"form:short".to_string()));

        let mut long = form.clone();
        assert_eq!(long[0].kind, SectionKind::Intro);
        long[0].bars = 64;
        assert!(form_targets(&in_form(&g, &long)).contains(&"form:intro".to_string()));

        // a verse thrice over is 48 bars of one texture, which no synthwave form plays
        let mut thrice = form.clone();
        let at = thrice.iter().position(|s| s.kind == SectionKind::Verse).unwrap();
        for name in ["verse again", "verse once more"] {
            thrice.insert(at + 1, crate::model::SectionSpec { name: name.into(), ..thrice[at].clone() });
        }
        assert!(form_targets(&in_form(&g, &thrice)).contains(&"form:same".to_string()));
        // twice over, 32 bars, is as long as a dance track plays without a change
        thrice.remove(at + 1);
        assert!(!form_targets(&in_form(&g, &thrice)).contains(&"form:same".to_string()));

        let mut once = form.clone();
        let first = once.iter().position(|s| s.kind == SectionKind::Hook).unwrap();
        for s in once.iter_mut().skip(first + 1).filter(|s| s.kind == SectionKind::Hook) {
            s.kind = SectionKind::Verse;
        }
        assert!(form_targets(&in_form(&g, &once)).contains(&"form:peaks-".to_string()));
        assert!(form_targets(&in_form(&g, &form)).is_empty());

        let g = crate::genres::get("trance").unwrap();
        let mut none = g.forms[0].clone();
        none.iter_mut().filter(|s| s.kind == SectionKind::Drop).for_each(|s| s.kind = SectionKind::Groove);
        assert!(form_issues(&in_form(&g, &none)).iter().any(|i| i.what.starts_with("the song never drops")));
    }

    #[test]
    fn a_lead_note_out_of_the_key_is_in_place_on_a_chord_that_has_it() {
        use crate::theory::chord::{Chord, Quality};
        use crate::theory::scale::{Key, Mode};
        use crate::theory::{ChordSpan, Note};
        let g = crate::genres::get("house").unwrap();
        let mut p = crate::compose::arrange(&crate::compose::generate(&g, 1, &Default::default(), None));
        // A minor: Am for a bar, then E7, whose G sharp the key does not have
        p.key = Key::new(9, Mode::Minor);
        p.chords = vec![
            ChordSpan { start: 0.0, len: 4.0, chord: Chord::new(9, Quality::Min) },
            ChordSpan { start: 4.0, len: 4.0, chord: Chord::new(4, Quality::Dom7) },
        ];
        let note = |t: f64, p: i32| Note { t, d: 1.0, p, v: 0.8, lock: None };
        let lead = p.tracks.iter_mut().find(|t| t.role == "lead").unwrap();
        lead.notes = vec![note(0.0, 69), note(1.0, 72), note(4.0, 68), note(5.0, 71)];
        let mut m = BTreeMap::new();
        theory_checks(&p, None, &mut m, &mut Vec::new());
        assert_eq!(m["lead_in_key"], 1.0);
        // an F sharp is neither A minor's nor E7's
        p.tracks.iter_mut().find(|t| t.role == "lead").unwrap().notes.push(note(6.0, 66));
        theory_checks(&p, None, &mut m, &mut Vec::new());
        assert_eq!(m["lead_in_key"], 0.8);
    }

    fn heard(genre_z: f64, alt_z: f64, production_z: f64, flaw_z: f64) -> EarsReport {
        EarsReport {
            genre_query: "house music".into(),
            genre_z,
            best_alt: "techno music".into(),
            best_alt_z: alt_z,
            production_z,
            worst_flaw: "a low quality recording with hiss".into(),
            worst_flaw_z: flaw_z,
            ..Default::default()
        }
    }

    fn issues(e: &EarsReport) -> Vec<Issue> {
        let mut out = Vec::new();
        ears_score(e, &mut out);
        out
    }

    #[test]
    fn the_ears_issues_say_what_to_change_in_words() {
        let got = issues(&heard(3.2, 3.7, 0.9, 1.7));
        let what: Vec<&str> = got.iter().map(|i| i.what.as_str()).collect();
        assert_eq!(
            what,
            [
                "sounds more like 'techno music' than 'house music'",
                "sounds more like a hissy recording than a professional mix: noise-built sounds (hats, shakers, \
                 risers) and long reverbs are the usual cause",
            ]
        );
        let faint = issues(&heard(2.0, 1.5, 1.6, 1.7));
        assert_eq!(faint[0].what, "sounds only faintly like 'house music' (the nearest other reading: 'techno music')");
        assert_eq!(
            faint[1].what.split(':').next(),
            Some("sounds about as much like a hissy recording as a professional mix")
        );
        assert!(issues(&heard(3.5, 2.0, 2.0, 1.0)).is_empty());
    }

    #[test]
    fn a_near_tie_comes_after_the_issues_that_name_a_change() {
        let tie = issues(&heard(3.5, 2.0, 1.6, 1.7));
        // a band out of its range by 2.5 dB has severity 0.42
        assert!(tie[0].severity < 0.3, "{}", tie[0].severity);
        let clear = issues(&heard(3.5, 2.0, 0.2, 1.7));
        assert!(clear[0].severity > 0.6, "{}", clear[0].severity);
    }

    #[test]
    fn a_song_that_reads_as_stock_music_says_so_and_loses_up_to_forty_percent() {
        let mut e = heard(3.2, 2.0, 2.0, 1.0);
        e.stock = "inspirational corporate background music".into();
        // implausible, or a full z behind the genre: nothing
        e.stock_z = 2.4;
        assert_eq!(stock_share(&e), 0.0);
        e.stock_z = 2.5;
        e.genre_z = 3.6;
        assert_eq!(stock_share(&e), 0.0);
        assert!(issues(&e).is_empty());
        // 0.3 behind: most of the cut
        e.genre_z = 3.2;
        e.stock_z = 2.9;
        assert!((stock_share(&e) - 0.7).abs() < 1e-9);
        assert_eq!(
            issues(&e)[0].what,
            "sounds close to stock background music ('inspirational corporate background music'): too clean and \
             polite for 'house music'"
        );
        // level with the genre or past it: all of it
        e.stock_z = 3.4;
        assert_eq!(stock_share(&e), 1.0);
        // past it as the strongest reading of all: one issue says so, not two
        e.best_alt = e.stock.clone();
        e.best_alt_z = 3.4;
        let both = issues(&e);
        assert_eq!(both.len(), 1, "{both:?}");
        assert!(both[0].what.starts_with("sounds more like 'inspirational corporate background music'"));
        // and it weighs as much as the cut it brings, not as a 0.2 z lead would
        assert_eq!(both[0].severity, 1.0);
    }

    #[test]
    fn a_recording_off_the_songs_tempo_or_wandering_off_its_beat_is_named_and_a_voice_is_not_judged() {
        use crate::render::{KeptTime, TrackReport};
        let g = crate::genres::get("house").unwrap();
        let p = crate::compose::arrange(&crate::compose::generate(&g, 1, &Default::default(), None));
        let at = |on_grid, own, own_bpm, drift_ms| KeptTime { on_grid, own, own_bpm, drift_ms };
        let check = |kept: Vec<(&str, &str, KeptTime)>| {
            let tracks = kept
                .into_iter()
                .map(|(name, role, k)| TrackReport {
                    name: name.into(),
                    role: role.into(),
                    sound: String::new(),
                    bus: "music".into(),
                    notes: 0,
                    clips: 1,
                    level_lufs: None,
                    gain_db: 0.0,
                    time: Some(k),
                })
                .collect();
            let r = Rendered {
                master: Stereo::zeros(1),
                stems: Vec::new(),
                tracks,
                lufs: -10.0,
                true_peak_db: -1.0,
                premaster_lufs: -10.0,
                settled: None,
            };
            let mut issues = Vec::new();
            (time_checks(&r, &p, &mut BTreeMap::new(), &mut issues), issues)
        };
        // on the song's grid: nothing to say
        let (s, issues) = check(vec![("original", "sample", at(0.4, 0.4, p.bpm, 10.0))]);
        assert_eq!((s, issues.len()), (Some(1.0), 0));
        // 3 % slow: its own tempo said, and as far out as can be
        let slow = p.bpm * 0.97;
        let (s, issues) = check(vec![("original", "sample", at(0.02, 0.35, slow, 50.0))]);
        assert_eq!(s, Some(0.0));
        assert_eq!(
            (issues[0].what.as_str(), issues[0].target.as_str()),
            (
                format!(
                    "'original' keeps a tempo of its own, about {slow:.0} BPM, against the song's {:.0}: the \
                     recording on it is off the song's time",
                    p.bpm
                )
                .as_str(),
                "time:original"
            )
        );
        // at the song's tempo, wandering 60 ms off its place
        let (s, issues) = check(vec![("break", "drums", at(0.3, 0.3, p.bpm, 60.0))]);
        assert_eq!(issues[0].what, "'break' drifts off the song's beat, by up to 60 ms");
        assert!((s.unwrap() - 0.25).abs() < 1e-9, "{s:?}");
        // a voice is not judged, nor a recording whose onsets keep no grid
        let none = vec![("vox1", "vocal", at(0.02, 0.4, slow, 50.0)), ("pad", "texture", at(0.01, 0.1, slow, 0.0))];
        let (s, issues) = check(none);
        assert!(s.is_none() && issues.is_empty(), "{s:?} {issues:?}");
    }

    #[test]
    fn every_flaw_has_words_and_a_cause() {
        for flaw in crate::ears::FLAWS {
            let (short, cause) = flaw_advice(flaw);
            assert!(short != *flaw && !cause.is_empty(), "{flaw}");
        }
    }
}

//! The critic: is this song any good? Three kinds of evidence, each scored 0..1 with the problems it found in
//! words and a hint for which part of the blueprint to change:
//!   mix      meters on the master: loudness vs target, true peak, tonal balance per band, stereo width, mono low end
//!   arc      the energy arc: section loudness follows the arrangement's energy, drops clearly louder than breaks
//!   theory   the score itself: melody in key (or in its chord), chord tones on strong beats, singable range and
//!            leaps, bass on the roots, smooth voice leading, form
//!   ears     (when earshot's models are available) genre fit against named confusions, production quality
//! The total is a weighted mean (0..100). Held-out evidence (AudioSet tags) is reported, never optimised.

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
    /// where a fix lives: "master", "band:<band>+/-", "width+/-", "level:<track>+/-", "form", "harmony",
    /// "part:<name>", "sound:<part>", "genre", "production"
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
    let secs = p.secs();
    m.insert("length_s".into(), secs.round());
    let mut form = 1.0;
    if !(90.0..=330.0).contains(&secs) {
        form -= 0.3;
        issues.push(Issue {
            severity: 0.3,
            what: format!("{:.0} s long (most tracks run 2-5 minutes)", secs),
            target: "form".into(),
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
    parts.push(form);
    // each part plays a sound its genre has a place for, as the genre's own draws would (`compose::has_place`): one
    // that does not is a check failed outright
    if let Some(g) = g {
        for t in &p.tracks {
            let Some(role) = crate::model::Role::parse(&t.role) else { continue };
            if crate::compose::has_place(g, role, &t.sound) {
                continue;
            }
            parts.push(0.0);
            issues.push(Issue {
                severity: 0.5,
                what: format!(
                    "{} plays {}, which {} has no place for",
                    t.name,
                    t.sound.describe(),
                    g.name.replace('_', " ")
                ),
                target: format!("sound:{}", t.name),
            });
        }
    }
    if parts.is_empty() { 1.0 } else { parts.iter().sum::<f64>() / parts.len() as f64 }
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
    let mut measures = BTreeMap::new();
    let mut issues = Vec::new();
    let mut scores = BTreeMap::new();
    // the song's genre, as its parts and its levels were made (none for a file judged as it is)
    let g = crate::genres::get(&p.genre).ok();
    scores.insert("mix".to_string(), mix_checks(r, p, g.as_ref(), &mut measures, &mut issues));
    scores.insert("arc".to_string(), arc_checks(r, p, &mut measures, &mut issues));
    scores.insert("theory".to_string(), theory_checks(p, g.as_ref(), &mut measures, &mut issues));
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
    fn every_flaw_has_words_and_a_cause() {
        for flaw in crate::ears::FLAWS {
            let (short, cause) = flaw_advice(flaw);
            assert!(short != *flaw && !cause.is_empty(), "{flaw}");
        }
    }
}

//! Is this a real recording / professionally produced audio, or a cheap code-synthesized sound? (earshot/realness.py)
//!
//! p_real = sigmoid(fusion of two standardised logistic heads) from data/realness_model.json: the CLAP head (the
//! CLAP embedding of the clip) and the signal head (measurements a synthesizer leaves behind). The fitted fusion
//! gives the signal head weight 0, so the CLAP head decides and the measurements are evidence in words. A pure,
//! dead-steady tone is decided by rule (p_real <= 0.03). Sounds over 6 s are judged in 4 s windows; their p_real
//! is the window mean and their verdict uses the file-level thresholds. Sources quieter than -30 dBFS RMS are
//! raised to -30 dBFS RMS first.

use serde::{Deserialize, Serialize};

use crate::dsp::fft::{C64, Rfft};
use crate::dsp::filter::{medfilt, sosfilt};
use crate::dsp::spectral::welch;
use crate::dsp::stats::{self, percentile};
use crate::dsp::{db, hanning, resample, rfftfreq, round_to};

pub const MODEL_JSON: &str = include_str!("../data/realness_model.json");
pub const LONG_S: f64 = 6.0;
pub const WIN_S: f64 = 4.0;
pub const SILENT_DB: f64 = -60.0;
pub const LIFT_BELOW_DB: f64 = -30.0;
pub const QUIET_MIN_PEAK_DB: f64 = -80.0;
const SR32: u32 = 32000;
const SRF: f64 = crate::SR as f64;

/// butter(4, [2000, 8000], 'band', fs=32000, output='sos') from scipy
const BAND_SOS: [[f64; 6]; 4] = [
    [0.037973041419476695, 0.07594608283895339, 0.037973041419476695, 1.0, -0.3603343294342602, 0.17410929884676085],
    [1.0, 2.0, 1.0, 1.0, -1.2519264039358684, 0.4537872134492352],
    [1.0, -2.0, 1.0, 1.0, -0.04605835937495241, 0.5866228548586725],
    [1.0, -2.0, 1.0, 1.0, -1.668120576649377, 0.8141384982137755],
];

// ------------------------------------------------------------------------------------------------ model file
#[derive(Deserialize, Clone, Debug)]
pub struct Head {
    pub mean: Vec<f64>,
    pub scale: Vec<f64>,
    pub clip: Option<[Vec<f64>; 2]>,
    pub coef: Vec<f64>,
    pub intercept: f64,
    pub inputs: serde_json::Value,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Heads {
    pub clap: Head,
    pub signal: Head,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Fusion {
    pub heads: Vec<String>,
    pub coef: Vec<f64>,
    pub intercept: f64,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Thresholds {
    pub hi: f64,
    pub lo: f64,
    #[serde(default)]
    pub files: Option<u64>,
    #[serde(default)]
    pub synth_files: Option<u64>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Prompts {
    pub real: Vec<String>,
    pub synth: Vec<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Fact {
    pub feature: String,
    pub side: String,
    pub op: String,
    pub value: f64,
    pub precision: f64,
}

#[derive(Deserialize, Clone, Debug)]
pub struct SilenceBase {
    pub at_least: f64,
    pub real: f64,
    pub synth: f64,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Model {
    pub heads: Heads,
    pub fusion: Fusion,
    pub thresholds: Thresholds,
    pub file_thresholds: Option<Thresholds>,
    pub prompts: Prompts,
    #[serde(default)]
    pub facts: Vec<Fact>,
    #[serde(default)]
    pub silence_base: Vec<SilenceBase>,
}

impl Model {
    pub fn embedded() -> Model {
        serde_json::from_str(MODEL_JSON).expect("data/realness_model.json")
    }

    pub fn thresholds(&self, file: bool) -> &Thresholds {
        if file { self.file_thresholds.as_ref().unwrap_or(&self.thresholds) } else { &self.thresholds }
    }

    pub fn verdict(&self, p: f64, file: bool) -> &'static str {
        let t = self.thresholds(file);
        if p >= t.hi {
            "real or pro-produced"
        } else if p <= t.lo {
            "likely code-synthesized"
        } else {
            "uncertain"
        }
    }

    /// the 12 reference prompts (real first)
    pub fn prompt_texts(&self) -> Vec<String> {
        self.prompts.real.iter().chain(&self.prompts.synth).cloned().collect()
    }
}

/// standardised logistic head: (logit, z)
pub fn head_logit(h: &Head, v: &[f64]) -> f64 {
    let mut s = h.intercept;
    for i in 0..h.coef.len() {
        let mut x = if v[i].is_finite() { v[i] } else { h.mean[i] };
        if let Some([lo, hi]) = &h.clip {
            x = x.clamp(lo[i], hi[i]);
        }
        s += (x - h.mean[i]) / h.scale[i] * h.coef[i];
    }
    s
}

fn fuse(f: &Fusion, clap: f64, signal: f64) -> f64 {
    let mut s = f.intercept;
    for (k, w) in f.heads.iter().zip(&f.coef) {
        s += w * if k == "clap" { clap } else { signal };
    }
    1.0 / (1.0 + (-s).exp())
}

// ------------------------------------------------------------------------------------------------ signal features
/// measurements that separate synthesized from recorded audio (realness.signal_features)
#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub struct SignalFeatures {
    pub silence_frac: f64,
    pub floor_db: f64,
    pub log_flatness: f64,
    pub peakiness_db: f64,
    pub line_share: f64,
    pub bandwidth_khz: f64,
    pub spec_floor_db: f64,
    pub quiet_flat: f64,
    pub tonal_frames: f64,
    pub pitch_wobble: f64,
    pub env_resid_db: f64,
    pub texture_kurt: f64,
    pub crest_db: f64,
}

impl SignalFeatures {
    pub fn get(&self, k: &str) -> f64 {
        match k {
            "silence_frac" => self.silence_frac,
            "floor_db" => self.floor_db,
            "log_flatness" => self.log_flatness,
            "peakiness_db" => self.peakiness_db,
            "line_share" => self.line_share,
            "bandwidth_khz" => self.bandwidth_khz,
            "spec_floor_db" => self.spec_floor_db,
            "quiet_flat" => self.quiet_flat,
            "tonal_frames" => self.tonal_frames,
            "pitch_wobble" => self.pitch_wobble,
            "env_resid_db" => self.env_resid_db,
            "texture_kurt" => self.texture_kurt,
            "crest_db" => self.crest_db,
            _ => f64::NAN,
        }
    }
}

fn frames(y: &[f64], n: usize, hop: usize) -> Vec<&[f64]> {
    if y.len() < n {
        return vec![];
    }
    let k = 1 + (y.len() - n) / hop;
    (0..k).map(|i| &y[i * hop..i * hop + n]).collect()
}

fn frames_or_pad(y: &[f64], n: usize, hop: usize) -> Vec<Vec<f64>> {
    let f = frames(y, n, hop);
    if f.is_empty() {
        let mut v = y.to_vec();
        v.resize(n, 0.0);
        return vec![v];
    }
    f.into_iter().map(|s| s.to_vec()).collect()
}

fn db10(v: f64) -> f64 {
    10.0 * v.max(1e-30).log10()
}

/// realness.signal_features of a mono 48 kHz clip (None when shorter than 20 ms or silent)
pub fn signal_features(x: &[f32]) -> Option<SignalFeatures> {
    let m: Vec<f64> = x.iter().map(|&v| v as f64).collect();
    let peak = m.iter().fold(0.0f64, |a, v| a.max(v.abs()));
    if m.len() < (0.02 * SRF) as usize || peak < 1e-7 {
        return None;
    }
    let mut f = SignalFeatures {
        silence_frac: m.iter().filter(|v| v.abs() < 1e-6).count() as f64 / m.len() as f64,
        ..Default::default()
    };
    let mut y: Vec<f64> = resample(x, crate::SR, SR32).into_iter().map(|v| v as f64).collect();
    let ymax = y.iter().fold(0.0f64, |a, v| a.max(v.abs())) + 1e-12;
    for v in y.iter_mut() {
        *v /= ymax;
    }
    let r: Vec<f64> = frames_or_pad(&y, 640, 640)
        .iter()
        .map(|fr| (stats::mean(&fr.iter().map(|v| v * v).collect::<Vec<_>>()) + 1e-30).sqrt())
        .collect();
    let rdb: Vec<f64> = r.iter().map(|v| 20.0 * v.log10()).collect();
    f.floor_db = (percentile(&rdb, 10.0) - stats::max(&rdb)).max(-140.0);
    let (fr, p) = welch(&y, SR32 as f64, 2048.min(y.len()));
    let sel: Vec<usize> = (0..fr.len()).filter(|&i| fr[i] >= 40.0 && fr[i] <= 15000.0).collect();
    let pb: Vec<f64> = sel.iter().map(|&i| p[i] + 1e-30).collect();
    let logs: Vec<f64> = pb.iter().map(|v| v.ln()).collect();
    f.log_flatness = (stats::mean(&logs).exp() / stats::mean(&pb)).log10();
    f.peakiness_db = db10(stats::max(&pb) / stats::median(&pb));
    let k = ((0.01 * pb.len() as f64) as usize).max(1);
    let mut sorted = pb.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    f.line_share = stats::pairwise_sum(&sorted[sorted.len() - k..]) / stats::pairwise_sum(&pb);
    let lv: Vec<f64> = pb.iter().map(|v| 10.0 * v.log10()).collect();
    let lmax = stats::max(&lv);
    let last = lv.iter().rposition(|&v| v >= lmax - 60.0).unwrap_or(0);
    f.bandwidth_khz = fr[sel[last]] / 1000.0;
    // short-time spectra
    let win = hanning(1024);
    let fb = rfftfreq(1024, SR32 as f64);
    let sb: Vec<usize> = (0..fb.len()).filter(|&i| fb[i] >= 40.0 && fb[i] <= 15000.0).collect();
    let mut fft = Rfft::new(1024);
    let mut buf = vec![0.0; 1024];
    let mut spec = vec![C64::new(0.0, 0.0); 513];
    let s_rows: Vec<Vec<f64>> = frames_or_pad(&y, 1024, 256)
        .iter()
        .map(|frm| {
            for i in 0..1024 {
                buf[i] = frm[i] * win[i];
            }
            fft.process(&mut buf, &mut spec);
            sb.iter().map(|&b| spec[b].norm().powi(2) + 1e-30).collect()
        })
        .collect();
    let nb = sb.len();
    let e: Vec<f64> = s_rows.iter().map(|row| 10.0 * stats::pairwise_sum(row).log10()).collect();
    let emax = stats::max(&e);
    let lfl: Vec<f64> = s_rows
        .iter()
        .zip(&e)
        .filter(|(_, e)| **e > emax - 30.0)
        .map(|(row, _)| {
            let l: Vec<f64> = row.iter().map(|v| 10.0 * v.log10()).collect();
            percentile(&l, 10.0) - stats::max(&l)
        })
        .collect();
    f.spec_floor_db = stats::median(&lfl);
    let act: Vec<bool> = e.iter().map(|v| *v > emax - 50.0).collect();
    let n_act = act.iter().filter(|a| **a).count();
    let quiet: Vec<bool> = if n_act >= 8 {
        let ea: Vec<f64> = e.iter().zip(&act).filter(|(_, a)| **a).map(|(v, _)| *v).collect();
        let q25 = percentile(&ea, 25.0);
        act.iter().zip(&e).map(|(a, v)| *a && *v <= q25).collect()
    } else {
        act.clone()
    };
    let qrows: Vec<&Vec<f64>> = if quiet.iter().any(|q| *q) {
        s_rows.iter().zip(&quiet).filter(|(_, q)| **q).map(|(r, _)| r).collect()
    } else {
        s_rows.iter().collect()
    };
    let qs: Vec<f64> = (0..nb).map(|b| qrows.iter().map(|r| r[b]).sum::<f64>() / qrows.len() as f64).collect();
    let qlogs: Vec<f64> = qs.iter().map(|v| v.ln()).collect();
    f.quiet_flat = (stats::mean(&qlogs).exp() / stats::mean(&qs)).log10();
    // pure-tone frames: the strongest peak (+-2 bins) holds over half of the frame's energy
    let idx: Vec<usize> = s_rows.iter().map(|row| stats::argmax(row).clamp(2, nb - 3)).collect();
    let tonal: Vec<bool> = s_rows
        .iter()
        .zip(&idx)
        .zip(&act)
        .map(|((row, &i), &a)| {
            let share = (((row[i - 2] + row[i - 1]) + row[i]) + row[i + 1]) + row[i + 2];
            a && share / stats::pairwise_sum(row) > 0.5
        })
        .collect();
    f.tonal_frames = tonal.iter().filter(|t| **t).count() as f64 / n_act.max(1) as f64;
    f.pitch_wobble = f64::NAN;
    let nr = s_rows.len();
    if nr >= 3 {
        let trip: Vec<bool> = (0..nr - 2).map(|j| tonal[j] && tonal[j + 1] && tonal[j + 2]).collect();
        if trip.iter().filter(|t| **t).count() >= 4 {
            let df = fb[1] - fb[0];
            let cents: Vec<f64> = s_rows
                .iter()
                .zip(&idx)
                .map(|(row, &i)| {
                    let (a, b, c) = (row[i - 1].ln(), row[i].ln(), row[i + 1].ln());
                    let fq = fb[sb[i]] + 0.5 * (a - c) / (a - 2.0 * b + c - 1e-12) * df;
                    1200.0 * fq.max(20.0).log2()
                })
                .collect();
            let d2: Vec<f64> = (0..nr - 2)
                .filter(|&j| trip[j])
                .map(|j| (cents[j + 2] - 2.0 * cents[j + 1] + cents[j]).abs())
                .collect();
            f.pitch_wobble = (stats::median(&d2) + 0.01).log10();
        }
    }
    // envelope smoothness: 5 ms log-energy residual around a 35 ms median trend, on active frames
    let e5: Vec<f64> = frames_or_pad(&y, 160, 160)
        .iter()
        .map(|fr| 10.0 * (stats::mean(&fr.iter().map(|v| v * v).collect::<Vec<_>>()) + 1e-30).log10())
        .collect();
    let e5max = stats::max(&e5);
    let ok: Vec<bool> = e5.iter().map(|v| *v > e5max - 40.0).collect();
    f.env_resid_db = if ok.iter().filter(|o| **o).count() >= 9 {
        let tr = medfilt(&e5, 7);
        let res: Vec<f64> = (0..e5.len()).filter(|&i| ok[i]).map(|i| (e5[i] - tr[i]).abs()).collect();
        (stats::median(&res) + 0.01).log10()
    } else {
        f64::NAN
    };
    // texture: kurtosis of the 2-8 kHz band after removing its 10 ms envelope
    let z = sosfilt(&BAND_SOS, &y);
    let n = z.len();
    let mut c = Vec::with_capacity(n + 1);
    c.push(0.0f64);
    let mut acc = 0.0;
    for v in &z {
        acc += v * v * (1.0 / 320.0);
        c.push(acc);
    }
    let env: Vec<f64> = (0..n)
        .map(|i| {
            let a = i.saturating_sub(160);
            let b = (i + 160).min(n);
            (c[b] - c[a]).max(0.0).sqrt() + 1e-9
        })
        .collect();
    let any_z = z.iter().any(|v| *v != 0.0);
    let zz: Vec<f64> = if any_z {
        let emax = stats::max(&env) * 10f64.powf(-40.0 / 20.0);
        (0..n).filter(|&i| z[i].abs() > 0.0 && env[i] > emax).map(|i| z[i] / env[i]).collect()
    } else {
        z.clone()
    };
    f.texture_kurt = if zz.len() > 100 {
        let m4 = stats::mean(&zz.iter().map(|v| v.powi(4)).collect::<Vec<_>>());
        let m2 = stats::mean(&zz.iter().map(|v| v * v).collect::<Vec<_>>());
        (m4 / (m2 * m2 + 1e-30) + 1e-9).ln()
    } else {
        f64::NAN
    };
    let ya: Vec<f64> = y.iter().filter(|v| v.abs() > 0.0).map(|v| v * v).collect();
    f.crest_db = if ya.is_empty() { 0.0 } else { 20.0 * (1.0 / (stats::mean(&ya).sqrt() + 1e-12)).log10() };
    Some(f)
}

/// a pure, dead-steady tone: one spectral line in every frame, flat level (crest <= 4.5 dB), no pitch drift
pub fn pure_tone(f: &SignalFeatures) -> bool {
    let w = f.pitch_wobble;
    f.tonal_frames >= 0.95 && f.line_share >= 0.9 && f.crest_db <= 4.5 && (!w.is_finite() || w <= -1.0)
}

fn pct(v: f64) -> String {
    if v > 0.0 && v < 0.095 { format!("{:.1}%", 100.0 * v) } else { format!("{:.0}%", 100.0 * v) }
}

/// one measured reason in words; toward_real = the model reads this value as evidence of a real recording
fn phrase(name: &str, v: f64, toward_real: bool) -> String {
    match name {
        "floor_db" => {
            if v < -95.0 {
                format!("quietest parts {v:.0} dB under the peak: cleaner than a microphone usually records")
            } else {
                format!("noise floor {v:.0} dB under the peak, like a microphone or room")
            }
        }
        "spec_floor_db" => {
            if !toward_real {
                format!(
                    "between its partials the spectrum falls {:.0} dB under the peak (no measurable mic or room noise)",
                    -v
                )
            } else {
                format!("a broadband noise floor {:.0} dB under the spectral peak, as microphones leave", -v)
            }
        }
        "line_share" => {
            if !toward_real {
                format!("{} of the energy sits in a few pure spectral lines (oscillator-like)", pct(v))
            } else {
                format!("energy spread over the spectrum (top 1% of bins hold {})", pct(v))
            }
        }
        "pitch_wobble" => if !toward_real {
            "pitch is dead steady like an oscillator (no natural drift)"
        } else {
            "pitch drifts naturally from frame to frame"
        }
        .into(),
        "env_resid_db" => if !toward_real {
            "envelope is mathematically smooth (exponential / linear ramps)"
        } else {
            "envelope has the micro-fluctuations of a recorded source"
        }
        .into(),
        "texture_kurt" => if !toward_real {
            "its noise is Gaussian like filtered random numbers, not the crackle of a real texture"
        } else {
            "high-frequency texture is crackly / impulsive like a real source"
        }
        .into(),
        "quiet_flat" => if !toward_real {
            "the quiet parts are tonal tails, not broadband room noise"
        } else {
            "the quiet parts are broadband room / mic noise"
        }
        .into(),
        "tonal_frames" => {
            if !toward_real {
                format!("{} of frames are dominated by one pure tone", pct(v))
            } else {
                format!("few pure-tone frames ({})", pct(v))
            }
        }
        "log_flatness" => if !toward_real {
            "spectrum is a few pure lines over near-silence"
        } else {
            "spectrum is dense and noise-rich"
        }
        .into(),
        "peakiness_db" => {
            if !toward_real {
                format!("one spectral line stands {v:.0} dB above the median bin")
            } else {
                format!("no dominant spectral line ({v:.0} dB peak/median)")
            }
        }
        "bandwidth_khz" => format!("content reaches {v:.1} kHz"),
        "crest_db" => {
            if !toward_real {
                format!("crest factor only {v:.0} dB: a steady, oscillator-like level")
            } else {
                format!("crest factor {v:.0} dB: sharp natural transients")
            }
        }
        _ => format!("{name} = {v:.3}"),
    }
}

/// signal facts that fire: (side, precision, text), most precise first
fn facts(m: &Model, f: &SignalFeatures) -> Vec<(String, f64, String)> {
    let mut out: Vec<(String, f64, String)> = m
        .facts
        .iter()
        .filter_map(|q| {
            let v = f.get(&q.feature);
            let hit = v.is_finite() && if q.op == "<=" { v <= q.value } else { v >= q.value };
            hit.then(|| {
                let who = if q.side == "real" { "real/pro" } else { "synthesized" };
                (
                    q.side.clone(),
                    q.precision,
                    format!(
                        "{} ({:.0}% of training clips past this were {who})",
                        phrase(&q.feature, v, q.side == "real"),
                        100.0 * q.precision
                    ),
                )
            })
        })
        .collect();
    out.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    out
}

fn silence_line(m: &Model, f: &SignalFeatures) -> Option<String> {
    let v = f.silence_frac;
    let r = m.silence_base.iter().rfind(|r| v >= r.at_least)?;
    if v < 0.02 {
        return None;
    }
    Some(format!(
        "{} of samples are exact digital silence (edited, gated or synthesized; weak evidence: {} of real/pro and {} of synthesized training clips have >= {})",
        pct(v),
        pct(r.real),
        pct(r.synth),
        pct(r.at_least)
    ))
}

fn clap_evidence(emb: &[f32], prompts: &[Vec<f32>], m: &Model, score: f64) -> (String, String) {
    let cos: Vec<f64> = prompts.iter().map(|p| p.iter().zip(emb).map(|(a, b)| *a as f64 * *b as f64).sum()).collect();
    let n = m.prompts.real.len();
    let names = m.prompt_texts();
    let o = stats::argsort_desc(&cos);
    let i = o[0];
    let j = *o.iter().find(|&&k| (k < n) != (i < n)).unwrap_or(&i);
    let side = if i < n { "real" } else { "synth" };
    let line = format!(
        "CLAP head (decides p_real): score {score:+.1} ({}); closest reference prompt '{}' ({side} side, cos {:.2}; best on the other side '{}', cos {:.2}; prompt cosines are weak context, not the decision)",
        if score >= 0.0 { "toward real" } else { "toward synthesized" },
        names[i],
        cos[i],
        names[j],
        cos[j]
    );
    (names[i].clone(), line)
}

/// RMS above SILENT_DB (quieter clips are not judged)
pub fn audible(x: &[f32]) -> bool {
    !x.is_empty() && db(rms(x)) > SILENT_DB
}

fn rms(x: &[f32]) -> f64 {
    let sq: Vec<f64> = x.iter().map(|&v| (v as f64) * (v as f64)).collect();
    stats::mean(&sq).sqrt()
}

/// a whole sound quieter than -30 dBFS RMS with content (peak over -80 dBFS) is raised to -30 dBFS RMS (peak at
/// most -1 dBFS): (x, gain_db, rms_db, peak_db)
pub fn lift_quiet(x: &[f32]) -> (Vec<f32>, f64, f64, f64) {
    if x.is_empty() {
        return (vec![], 0.0, -200.0, -200.0);
    }
    let rdb = db(rms(x));
    let pdb = db(x.iter().fold(0.0f32, |a, v| a.max(v.abs())) as f64);
    let g = (LIFT_BELOW_DB - rdb).min(-1.0 - pdb);
    if pdb < QUIET_MIN_PEAK_DB || g <= 0.0 {
        return (x.to_vec(), 0.0, rdb, pdb);
    }
    let k = 10f64.powf(g / 20.0) as f32;
    (x.iter().map(|v| v * k).collect(), g, rdb, pdb)
}

/// [(t0, t1, start, end)]: the whole sound up to 6 s, else 4 s windows (the last aligned to the end, unless that
/// would add under 0.25 s of new audio)
pub fn clips(n: usize) -> Vec<(f64, f64, usize, usize)> {
    if n as f64 <= LONG_S * SRF {
        return vec![(0.0, n as f64 / SRF, 0, n)];
    }
    let mut w = crate::audio::windows(n, WIN_S, WIN_S, true);
    if w.len() > 2 && w[w.len() - 1].0 - w[w.len() - 2].0 < 0.25 {
        w.pop();
    }
    w.into_iter().map(|(t0, a, b)| (t0, t0 + WIN_S, a, b)).collect()
}

/// m:ss.ss (report.fmt_t)
pub fn fmt_t(t: f64) -> String {
    let m = (t / 60.0).floor();
    format!("{}:{:05.2}", m as i64, t - 60.0 * m)
}

fn pf(p: f64, t: &Thresholds) -> String {
    let s = format!("{p:.2}");
    let v: f64 = s.parse().unwrap_or(p);
    if [t.lo, t.hi].iter().any(|&th| v == th && (p - th).abs() > 1e-9) { format!("{p:.3}") } else { s }
}

/// one judged clip
#[derive(Serialize, Clone, Debug)]
pub struct ClipScore {
    pub p_real: f64,
    pub logit_clap: f64,
    pub logit_signal: f64,
    pub tone: bool,
    pub features: SignalFeatures,
    /// the closest of the 12 reference prompts (weak context)
    pub closest_prompt: String,
    #[serde(skip)]
    facts: Vec<(String, f64, String)>,
    #[serde(skip)]
    clap_line: String,
}

/// p_real + head logits + evidence for one audible clip, given its CLAP embedding and the 12 prompt embeddings
pub fn score_clip(m: &Model, f: SignalFeatures, emb: &[f32], prompts: &[Vec<f32>]) -> ClipScore {
    let e: Vec<f64> = emb.iter().map(|&v| v as f64).collect();
    let lc = head_logit(&m.heads.clap, &e);
    let names: Vec<String> = match &m.heads.signal.inputs {
        serde_json::Value::Array(a) => a.iter().filter_map(|v| v.as_str().map(String::from)).collect(),
        _ => vec![],
    };
    let sv: Vec<f64> = names.iter().map(|k| f.get(k)).collect();
    let ls = head_logit(&m.heads.signal, &sv);
    let mut p = fuse(&m.fusion, lc, ls);
    let (near, mut line) = clap_evidence(emb, prompts, m, lc);
    let mut fx = facts(m, &f);
    let tone = pure_tone(&f);
    if tone {
        p = p.min(0.03);
        fx.insert(
            0,
            (
                "synth".into(),
                0.99,
                "a pure, dead-steady tone: one spectral line, flat level, no pitch drift (a test tone or bare oscillator; real sources drift and breathe)".into(),
            ),
        );
        line = format!(
            "decided by the pure-tone rule, not the CLAP head (the CLAP head alone scores {lc:+.1}, {}: a bare tone is outside its training data)",
            if lc >= 0.0 { "toward real" } else { "toward synthesized" }
        );
    }
    ClipScore {
        p_real: p,
        logit_clap: lc,
        logit_signal: ls,
        tone,
        features: f,
        closest_prompt: near,
        facts: fx,
        clap_line: line,
    }
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Segment {
    pub start: f64,
    pub end: f64,
    pub p_real: Option<f64>,
    pub verdict: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
}

/// realness of a sound (realness.realness without the file cache)
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Realness {
    pub duration: f64,
    /// probability the sound is a real recording or professionally produced (None when silent)
    pub p_real: Option<f64>,
    pub verdict: String,
    pub why: String,
    pub evidence: Vec<String>,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gain_db: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segments: Option<Vec<Segment>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub synth_windows: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lean_synth_windows: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p_real_min: Option<f64>,
}

fn spans_text(segs: &[&Segment]) -> String {
    let mut out: Vec<(f64, f64)> = Vec::new();
    for s in segs {
        if let Some(l) = out.last_mut().filter(|l| s.start <= l.1 + 1e-6) {
            l.1 = l.1.max(s.end);
        } else {
            out.push((s.start, s.end));
        }
    }
    let txt: Vec<String> = out.iter().take(6).map(|(a, b)| format!("{}-{}", fmt_t(*a), fmt_t(*b))).collect();
    txt.join(", ") + if out.len() > 6 { " ..." } else { "" }
}

/// assemble the result from per-clip scores (None = silent clip); name = label for the summary
#[allow(clippy::too_many_arguments)]
pub fn assemble(
    m: &Model,
    name: &str,
    dur: f64,
    off: f64,
    spans: &[(f64, f64, usize, usize)],
    rows: &[Option<ClipScore>],
    gain: f64,
    rdb: f64,
    pdb: f64,
) -> Realness {
    let mut res = Realness {
        duration: round_to(dur, 3),
        p_real: None,
        verdict: String::new(),
        why: String::new(),
        evidence: vec![],
        summary: String::new(),
        note: None,
        gain_db: None,
        segments: None,
        synth_windows: None,
        lean_synth_windows: None,
        p_real_min: None,
    };
    let scored: Vec<(usize, &ClipScore)> =
        rows.iter().enumerate().filter_map(|(i, r)| r.as_ref().map(|r| (i, r))).collect();
    if scored.is_empty() {
        let peak = if pdb <= -199.0 { "exact digital silence".to_string() } else { format!("peak {pdb:.0} dBFS") };
        let why = format!("silent (no part above {SILENT_DB:.0} dBFS RMS; {peak}), not judged");
        res.verdict = "silent".into();
        res.evidence = vec!["the sound is (near) silent: nothing to judge".into()];
        res.summary = format!("{name}: {why}.");
        res.why = why;
        return res;
    }
    let long_ = spans.len() > 1;
    let ps: Vec<f64> = scored.iter().map(|(_, r)| r.p_real).collect();
    let p = ps.iter().sum::<f64>() / ps.len() as f64;
    let v = m.verdict(p, long_);
    let real_side = p >= 0.5;
    // the most real / synthesized window (first one on ties, like Python's max / min)
    let mut pick = scored[0];
    for &s in &scored[1..] {
        if (real_side && s.1.p_real > pick.1.p_real) || (!real_side && s.1.p_real < pick.1.p_real) {
            pick = s;
        }
    }
    let (s0, r0) = pick;
    let side = if real_side { "real" } else { "synth" };
    let pro: Vec<String> = r0.facts.iter().filter(|f| f.0 == side).take(3).map(|f| f.2.clone()).collect();
    let con: Vec<String> = r0.facts.iter().filter(|f| f.0 != side).take(1).map(|f| f.2.clone()).collect();
    let sil = silence_line(m, &r0.features);
    let mut ev = vec![r0.clap_line.clone()];
    ev.extend(pro.iter().cloned());
    ev.extend(con.iter().map(|t| format!("but: {t}")));
    if let Some(s) = &sil {
        ev.push(s.clone());
    }
    if gain != 0.0 {
        res.gain_db = Some(round_to(gain, 1));
    }
    if gain >= 3.0 {
        ev.push(format!(
            "quiet source (RMS {rdb:.0} dBFS, peak {pdb:.0} dBFS): judged after +{gain:.0} dB of gain (very quiet input bends the CLAP features; level otherwise barely moves p_real, see RESULTS.md)"
        ));
    }
    if long_ {
        let (a, b, _, _) = spans[s0];
        ev.insert(
            0,
            format!(
                "evidence below is from the most {}-sounding window, {}-{} (p_real {:.2})",
                if real_side { "real" } else { "synthesized" },
                fmt_t(off + a),
                fmt_t(off + b.min(dur)),
                r0.p_real
            ),
        );
    }
    if m.fusion.coef.get(1).copied().unwrap_or(0.0) != 0.0 {
        ev.push(format!(
            "fusion weights: CLAP head {:.2}, signal head {:.2} (signal head score {:+.1})",
            m.fusion.coef[0], m.fusion.coef[1], r0.logit_signal
        ));
    } else if !pro.is_empty() || !con.is_empty() || sil.is_some() {
        ev.push("the signal measurements above are context, not votes: the signal head added no held-out accuracy, so its fusion weight is 0".into());
    }
    let t = m.thresholds(false);
    if !long_ {
        let lc = r0.logit_clap;
        let why = if r0.tone {
            format!("pure-tone rule (the CLAP head alone: {lc:+.1})")
        } else {
            format!("CLAP head score {lc:+.1} ({})", if lc >= 0.0 { "toward real" } else { "toward synthesized" })
        } + &if !pro.is_empty() && !r0.tone { format!("; {}", pro[0]) } else { String::new() };
        res.p_real = Some(round_to(p, 3));
        res.verdict = v.into();
        res.summary = format!("{name}: {v} (p_real {}; clip thresholds lo {} / hi {}): {why}.", pf(p, t), t.lo, t.hi);
        res.why = why;
        res.evidence = ev;
        res.note = Some(format!(
            "p_real is calibrated on held-out clips (lo {} / hi {} hold ~92% precision there). It hears how a sound sounds, not where it came from: clean professionally synthesized tones (UI sounds, chimes, synth stabs) also read as code-synthesized. Trained on Freesound/Mixkit recordings, pro launch films and music vs numpy-synthesized sounds: see eval/realness/RESULTS.md for accuracy and what fools it",
            t.lo, t.hi
        ));
        return res;
    }
    let mut segs = Vec::new();
    for ((a, b, _, _), r) in spans.iter().zip(rows) {
        let mut seg = Segment {
            start: round_to(off + a, 3),
            end: round_to(off + b.min(dur), 3),
            p_real: None,
            verdict: "silent".into(),
            why: None,
        };
        if let Some(r) = r {
            let sd = if r.p_real >= 0.5 { "real" } else { "synth" };
            seg.p_real = Some(round_to(r.p_real, 3));
            seg.verdict = m.verdict(r.p_real, false).into();
            seg.why = r.facts.iter().find(|f| f.0 == sd).map(|f| f.2.clone());
        }
        segs.push(seg);
    }
    let live: Vec<&Segment> = segs.iter().filter(|s| s.p_real.is_some()).collect();
    let lean: Vec<&Segment> = live.iter().copied().filter(|s| s.p_real.unwrap() < 0.5).collect();
    let flagged: Vec<&Segment> = live.iter().copied().filter(|s| s.p_real.unwrap() <= t.lo).collect();
    let ft = m.thresholds(true);
    let mlc = scored.iter().map(|(_, r)| r.logit_clap).sum::<f64>() / scored.len() as f64;
    let tones = scored.iter().filter(|(_, r)| r.tone).count();
    let why = format!(
        "{} of {} windows lean synthesized (p_real < 0.5); mean CLAP head score {mlc:+.1}",
        lean.len(),
        live.len()
    ) + &if tones > 0 { format!("; {tones} window(s) decided by the pure-tone rule") } else { String::new() };
    let pmin = ps.iter().copied().fold(f64::INFINITY, f64::min);
    let tail = if !lean.is_empty() {
        format!(" Leaning synthesized: {}", spans_text(&lean))
            + &if !flagged.is_empty() {
                format!(
                    "; {} at or below the 4 s-window threshold {} (code-synthesized): {}.",
                    flagged.len(),
                    t.lo,
                    spans_text(&flagged)
                )
            } else {
                format!("; none at or below the 4 s-window threshold {}.", t.lo)
            }
    } else {
        format!(
            " No window leans synthesized (lowest p_real {pmin:.2}); that does not rule out a code-made layer under a louder real mix (most are missed): judge the source files and stems."
        )
    };
    res.p_real = Some(round_to(p, 3));
    res.verdict = v.into();
    res.summary = format!(
        "{name}: {v} (p_real {}, mean of {} 4 s windows; file thresholds lo {} / hi {}, weak calibration): {why}.{tail}",
        pf(p, ft),
        live.len(),
        ft.lo,
        ft.hi
    );
    res.why = why;
    res.evidence = ev;
    res.synth_windows = Some(flagged.len());
    res.lean_synth_windows = Some(lean.len());
    res.p_real_min = Some(round_to(pmin, 3));
    res.note = Some(format!(
        "a long file's p_real is the mean of its window probabilities; its verdict uses file-level thresholds lo {} / hi {} read from held-out long files ({} files, only {} synthesized: weak calibration). Each window is calibrated on held-out 4 s clips (lo {} / hi {}). Windows find stretches made entirely of code synthesis; a code-made layer under a louder real mix is usually missed, so judge the source files and stems too. It hears how a sound sounds, not where it came from (clean pro-synthesized UI tones read as code-synthesized): see eval/realness/RESULTS.md",
        ft.lo,
        ft.hi,
        ft.files.map_or("?".into(), |v| v.to_string()),
        ft.synth_files.map_or("?".into(), |v| v.to_string()),
        t.lo,
        t.hi
    ));
    res.segments = Some(segs);
    res
}

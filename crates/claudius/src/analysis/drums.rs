//! Drums heard hit by hit: where each strikes, how hard, and what it is, a kick, a snare or a hat, whether a kit
//! played it or a mouth did (beatboxing).
//!
//! The onsets are those of the whole sound and of its top over 5 kHz (a quiet hat under a kick's tail), one to a hit
//! within 25 ms. Each one's sound is what it adds: every band at its strongest in the 50 ms after it (20 ms at a time,
//! a period of the lowest kick), less what it had in the 20 ms before (the tail of the hit before), and of that, how
//! much lies under 150 Hz, in a drum's body (150 to 800 Hz) and over 5 kHz. An onset that doubles the energy of
//! neither the whole sound nor its low or high band (one carrying a twentieth of it at least) is a ripple inside a
//! sound, not a hit, and one 30 dB under the loudest hit is a breath or the room.
//!
//! Kicks live low; hats high, with next to nothing in a drum's body; snares, claps and a beatboxer's "k" and "pf" in
//! between. The three are found as clusters of the recording's own hits (a mouth's kick is not a kit's), then named
//! by where their middles lie. A cluster whose middle isn't near where its kind lies is not that kind (a loop of kicks
//! alone still makes three clusters), and its hits are named by where each lies alone. A snare adds most of its
//! energy between the low and the high band, so a hit that adds little there is a kick or a hat, whichever band it
//! adds more to. A hit with a second sound under it, a kick and a hat struck at once, is both when it adds to that
//! other band well beyond what hits of its own kind do there: half what that kind's own hits add, or, when they all
//! lie under other hits, three times what its own kind adds.
//!
//! [`fold`] then sets the hits on a song's grid and folds the times round into a pattern of a few bars: a step hit
//! nearly every time round is a hit, one hit some of the times round is a hit with that chance.

use crate::audio::Stereo;
use crate::config::SRF;
use crate::dsp::filter;

/// what a drum hit is
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    Kick,
    Snare,
    Hat,
}

impl Kind {
    pub const ALL: [Kind; 3] = [Kind::Kick, Kind::Snare, Kind::Hat];

    pub fn name(self) -> &'static str {
        match self {
            Kind::Kick => "kick",
            Kind::Snare => "snare",
            Kind::Hat => "hat",
        }
    }
}

/// a drum hit heard: when (seconds from the start), what, and how loud (the level of its first 30 ms)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub t: f64,
    pub kind: Kind,
    pub level: f32,
}

/// an onset and its sound: how much of what it adds lies low (under 150 Hz), in a drum's body (150 to 800 Hz) and
/// high (over 5 kHz), and how much each band (low, middle, high) rose at it
struct Sound {
    t: f64,
    level: f32,
    low: f32,
    body: f32,
    high: f32,
    rise: [f32; 3],
    /// what it adds from 800 Hz to 5 kHz, where a snare's and a clap's noise lies and a kick's next to nothing
    upper: f32,
}

impl Sound {
    /// the share of what it adds between the low and the high band
    fn middle(&self) -> f32 {
        (1.0 - self.low - self.high).max(0.0)
    }

    /// the share of what it adds from 800 Hz to 5 kHz
    fn upper_share(&self) -> f32 {
        self.upper / self.rise.iter().sum::<f32>().max(1e-12)
    }
}

/// the mean square of `x` from sample `a` to `b`
fn power(x: &[f32], a: usize, b: usize) -> f32 {
    let b = b.min(x.len());
    if b <= a {
        return 0.0;
    }
    x[a..b].iter().map(|v| v * v).sum::<f32>() / (b - a) as f32
}

/// The drum hits in `x`, in time order.
pub fn hear(x: &Stereo) -> Vec<Hit> {
    let sounds = sounds(x);
    classify(&sounds)
}

/// the onsets of `x` that are hits, and their sounds
fn sounds(x: &Stereo) -> Vec<Sound> {
    let mono = x.mono();
    let mut low = mono.clone();
    filter::butter(&mut low, false, 150.0, SRF, 4);
    let mut high = mono.clone();
    filter::butter(&mut high, true, 5000.0, SRF, 4);
    let band = |lo: f64, hi: f64| {
        let mut y = mono.clone();
        filter::butter(&mut y, true, lo, SRF, 4);
        filter::butter(&mut y, false, hi, SRF, 4);
        y
    };
    let (body, upper) = (band(150.0, 800.0), band(800.0, 5000.0));
    let ms = |s: f64| (s * SRF / 1000.0) as usize;
    // the onsets of the whole sound and of its top (a quiet hat under a kick's tail), one to a hit: the body of a
    // kick arriving after its click is the same hit
    let mut found = super::onsets::detect(&mono, 0.06, 0.03);
    found.extend(super::onsets::detect(&high, 0.02, 0.03));
    found.sort_unstable();
    let mut ons: Vec<usize> = Vec::new();
    for o in found {
        if ons.last().is_none_or(|&l| o >= l + ms(25.0)) {
            ons.push(o);
        }
    }
    let sounds: Vec<Sound> = ons
        .iter()
        .enumerate()
        .map(|(i, &o)| {
            let next = ons.get(i + 1).copied().unwrap_or(mono.len());
            let end = (o + ms(50.0)).min(next.saturating_sub(ms(2.0))).max(o + ms(10.0));
            // each band at its strongest after the onset (20 ms at a time, a period of the lowest kick), against the
            // 20 ms just before it
            let (a, b) = (o.saturating_sub(ms(21.0)), o.saturating_sub(ms(1.0)));
            let peak = |x: &[f32]| {
                (o..end.max(o + ms(5.0))).step_by(ms(5.0)).map(|w| power(x, w, w + ms(20.0))).fold(0.0f32, f32::max)
            };
            let rise = |x: &[f32]| (peak(x) - power(x, a, b)).max(0.0);
            let (lo, bd, up, hi) = (rise(&low), rise(&body), rise(&upper), rise(&high));
            let all = (lo + bd + up + hi).max(1e-12);
            let level = peak(&mono);
            let grew = |x: &[f32]| {
                let now = peak(x);
                now >= 2.0 * power(x, a, b) && now >= level / 20.0
            };
            let new = grew(&mono) || grew(&low) || grew(&high);
            Sound {
                t: o as f64 / SRF,
                level: if new { power(&mono, o, o + ms(30.0)).sqrt() } else { 0.0 },
                low: lo / all,
                body: bd / all,
                high: hi / all,
                rise: [lo, bd + up, hi],
                upper: up,
            }
        })
        .filter(|s| s.level > 1e-4)
        .collect();
    // breath and room noise between the hits: 30 dB under the loudest hit
    let loudest = sounds.iter().map(|s| s.level).fold(0.0f32, f32::max);
    sounds.into_iter().filter(|s| s.level >= 0.03 * loudest).collect()
}

/// what each of `sounds` is (two kinds, where a second sound lies under one)
fn classify(sounds: &[Sound]) -> Vec<Hit> {
    // a snare adds most of its energy in the middle: a hit that adds little there is a kick or a hat. And where a
    // quarter of the recording's snares at least have their noise from 800 Hz to 5 kHz more than over it, as claps
    // and most snares do, one with its noise over 5 kHz more is a hat (an open hat, which a drum stem's bleed gives a
    // body); a recording whose snares are all bright keeps them
    let named = name(sounds);
    let under = |s: &Sound| s.upper_share() >= s.high;
    // the snares that stay snares: not those adding too little in the middle, which are a kick or a hat
    let snares: Vec<&Sound> =
        sounds.iter().zip(&named).filter(|(s, k)| **k == Kind::Snare && s.middle() >= 0.35).map(|(s, _)| s).collect();
    let split = snares.iter().filter(|s| under(s)).count() * 4 >= snares.len() && snares.iter().any(|s| !under(s));
    let kinds: Vec<Kind> = named
        .into_iter()
        .zip(sounds)
        .map(|(k, s)| match k {
            Kind::Snare if s.middle() < 0.35 => {
                if s.low >= s.high {
                    Kind::Kick
                } else {
                    Kind::Hat
                }
            }
            Kind::Snare if split && !under(s) => Kind::Hat,
            k => k,
        })
        .collect();
    let mut out = Vec::new();
    for (s, &kind) in sounds.iter().zip(&kinds) {
        out.push(Hit { t: s.t, kind, level: s.level });
    }
    // a second sound under a hit: what it adds in that kind's band beyond what its own kind usually adds there (a
    // quarter of them add less), at least half what that kind's own hits add, or, when every one of them lies under
    // another hit (open hats on the kicks), three times what its own kind adds there and a thousandth of the loudest
    let usual = |kind: Kind, band: usize, at: f32| {
        let mut r: Vec<f32> =
            sounds.iter().zip(&kinds).filter(|(_, k)| **k == kind).map(|(s, _)| s.rise[band]).collect();
        r.sort_by(f32::total_cmp);
        r.get(((r.len() as f32 * at) as usize).min(r.len().saturating_sub(1))).copied()
    };
    let loudest = sounds.iter().map(|s| s.level * s.level).fold(0.0f32, f32::max);
    for (kind, band) in [(Kind::Kick, 0), (Kind::Hat, 2)] {
        let typical = usual(kind, band, 0.5);
        for (s, &k) in sounds.iter().zip(&kinds) {
            let own = usual(k, band, 0.25).unwrap_or(0.0);
            let need = typical.map_or((3.0 * own).max(1e-3 * loudest), |t| 0.5 * t);
            if k != kind && s.rise[band] - own >= need {
                let part = ((s.rise[band] - own) / s.rise.iter().sum::<f32>().max(1e-12)).sqrt();
                out.push(Hit { t: s.t, kind, level: s.level * part });
            }
        }
    }
    // a snare or a clap struck with a kick (house's claps on two and four): a kick that adds from 800 Hz to 5 kHz half
    // what the louder snares heard alone add there, beyond what kicks usually do (a rim on the kick adds less)
    let upper = |kind: Kind, at: f32| {
        let mut r: Vec<f32> = sounds.iter().zip(&kinds).filter(|(_, k)| **k == kind).map(|(s, _)| s.upper).collect();
        r.sort_by(f32::total_cmp);
        r.get(((r.len() as f32 * at) as usize).min(r.len().saturating_sub(1))).copied()
    };
    if let (Some(snares), Some(own)) = (upper(Kind::Snare, 0.75), upper(Kind::Kick, 0.25)) {
        for (s, _) in sounds.iter().zip(&kinds).filter(|(_, k)| **k == Kind::Kick) {
            if s.upper - own >= 0.5 * snares && snares > 0.0 {
                let part = ((s.upper - own) / s.rise.iter().sum::<f32>().max(1e-12)).sqrt();
                out.push(Hit { t: s.t, kind: Kind::Snare, level: s.level * part });
            }
        }
    }
    out.sort_by(|a, b| a.t.total_cmp(&b.t).then(a.kind.cmp(&b.kind)));
    out
}

/// What a sound is by where it lies alone: low enough is a kick, high enough a hat, the rest a snare. A kick's hits
/// add a quarter of their energy under 150 Hz or more, which few snares do; a hat's add half over 5 kHz, which few
/// snares and claps do, or at least a fifth with next to nothing in a drum's body, as a dark hat does.
fn by_itself(s: &Sound) -> Kind {
    if s.low >= 0.25 {
        Kind::Kick
    } else if s.high >= 0.5 || (s.high >= 0.2 && s.body < 0.03) {
        Kind::Hat
    } else {
        Kind::Snare
    }
}

/// The kinds of `sounds`: three clusters of where they lie (low, in a drum's body, high), named by where their
/// middles lie.
fn name(sounds: &[Sound]) -> Vec<Kind> {
    let at = |s: &Sound| [s.low, s.body, s.high];
    if sounds.len() < 3 {
        return sounds.iter().map(by_itself).collect();
    }
    let d = |a: [f32; 3], b: [f32; 3]| a.iter().zip(&b).map(|(x, y)| (x - y).powi(2)).sum::<f32>().sqrt();
    // each starts amid the quarter of the hits that lie most its way: the lowest, the most in the middle, the
    // highest (one stray sound can't take a cluster for itself)
    let start = |by: &dyn Fn(&Sound) -> f32| {
        let mut v: Vec<&Sound> = sounds.iter().collect();
        v.sort_by(|a, b| by(b).total_cmp(&by(a)));
        let top = &v[..v.len().div_ceil(4)];
        let n = top.len() as f32;
        std::array::from_fn(|i| top.iter().map(|s| at(s)[i]).sum::<f32>() / n)
    };
    let mut middles = [start(&|s| s.low), start(&|s| s.middle()), start(&|s| s.high)];
    let mut of = vec![0usize; sounds.len()];
    for _ in 0..20 {
        for (o, s) in of.iter_mut().zip(sounds) {
            *o = (0..3).fold(0, |b, c| if d(at(s), middles[c]) < d(at(s), middles[b]) { c } else { b });
        }
        for (c, m) in middles.iter_mut().enumerate() {
            let members: Vec<[f32; 3]> = sounds.iter().zip(&of).filter(|(_, o)| **o == c).map(|(s, _)| at(s)).collect();
            if !members.is_empty() {
                let n = members.len() as f32;
                *m = std::array::from_fn(|i| members.iter().map(|p| p[i]).sum::<f32>() / n);
            }
        }
    }
    let kick = (0..3).fold(0, |b, c| if middles[c][0] > middles[b][0] { c } else { b });
    let hat = (0..3)
        .filter(|&c| c != kick)
        .fold(usize::MAX, |b, c| if b == usize::MAX || middles[c][2] > middles[b][2] { c } else { b });
    let named = |c: usize| {
        if c == kick {
            Kind::Kick
        } else if c == hat {
            Kind::Hat
        } else {
            Kind::Snare
        }
    };
    // a cluster whose middle isn't near where its kind lies is not that kind: the recording lacks it (a loop of kicks
    // alone still makes three clusters), and its hits are named by where they lie alone. Nearer than alone, as a
    // mouth's kick lies higher than a kit's.
    let fits: Vec<bool> = (0..3)
        .map(|c| {
            let [low, body, high] = middles[c];
            match named(c) {
                Kind::Kick => low >= 0.12 && high < 0.3,
                Kind::Hat => high >= 0.2 && low < 0.12 && body < 0.1,
                Kind::Snare => 1.0 - low - high >= 0.4 && low < 0.25,
            }
        })
        .collect();
    of.iter().zip(sounds).map(|(&c, s)| if fits[c] { named(c) } else { by_itself(s) }).collect()
}

/// a kind's pattern: each sixteenth's velocity (0 for none), and how often it was hit across the times round
#[derive(Clone, Debug, PartialEq)]
pub struct Steps {
    pub kind: Kind,
    pub v: Vec<f32>,
    pub often: Vec<f32>,
}

/// The hits on a grid of `bpm` whose first downbeat is at `start` seconds, folded into a pattern of `bars` bars:
/// every kind heard, each sixteenth hit at least a quarter of the times round, with the median velocity of its
/// hits (against the loudest of that kind) and how often.
pub fn fold(hits: &[Hit], bpm: f64, start: f64, bars: u32) -> Vec<Steps> {
    let n = (bars.max(1) * 16) as i64;
    let at = |h: &Hit| ((h.t - start) * bpm / 60.0 * 4.0).round() as i64;
    let rounds = hits.iter().map(|h| at(h).div_euclid(n)).filter(|r| *r >= 0).max().map_or(1, |r| r + 1) as usize;
    let mut out = Vec::new();
    for kind in Kind::ALL {
        let of: Vec<&Hit> = hits.iter().filter(|h| h.kind == kind && at(h) >= 0).collect();
        let loudest = of.iter().map(|h| h.level).fold(0.0f32, f32::max);
        if of.is_empty() || loudest <= 0.0 {
            continue;
        }
        // the loudest of each step each time round
        let mut heard = vec![vec![0f32; rounds]; n as usize];
        for h in &of {
            let k = at(h);
            let (r, s) = (k.div_euclid(n) as usize, k.rem_euclid(n) as usize);
            heard[s][r] = heard[s][r].max(h.level);
        }
        let (mut v, mut often) = (vec![0f32; n as usize], vec![0f32; n as usize]);
        for s in 0..n as usize {
            let mut levels: Vec<f32> = heard[s].iter().copied().filter(|&l| l > 0.0).collect();
            let share = levels.len() as f32 / rounds as f32;
            if levels.is_empty() || share < 0.25 {
                continue;
            }
            levels.sort_by(f32::total_cmp);
            v[s] = (levels[levels.len() / 2] / loudest).powf(0.6).clamp(0.25, 1.0);
            often[s] = share;
        }
        out.push(Steps { kind, v, often });
    }
    out
}

impl Steps {
    /// as a step string: `X` for an accent, `1` to `9` for velocities, `.` a rest, a bar to each `|`
    pub fn string(&self) -> String {
        let mut s = String::new();
        for (i, &v) in self.v.iter().enumerate() {
            if i > 0 && i % 16 == 0 {
                s.push('|');
            }
            s.push(match (v * 10.0).round() as u32 {
                0 if v <= 0.0 => '.',
                0 => '1',
                d @ 1..=9 => char::from_digit(d, 10).unwrap_or('.'),
                _ => 'X',
            });
        }
        s
    }

    /// the steps hit only some of the times round, with the chance they were: (step, chance)
    pub fn chances(&self) -> Vec<(usize, f32)> {
        (0..self.v.len())
            .filter(|&s| self.v[s] > 0.0 && self.often[s] < 0.75)
            .map(|s| (s, (self.often[s] * 20.0).round() / 20.0))
            .collect()
    }

    /// how many sixteenths are hit
    pub fn hits(&self) -> usize {
        self.v.iter().filter(|&&v| v > 0.0).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;

    /// house at 100 BPM for `bars` bars: a kick on every beat, a clap on two and four (on the kick), an open hat on
    /// every offbeat, and a clap alone at the end of every bar
    fn house(bars: usize) -> Stereo {
        let spb = 0.6;
        let n = (bars as f64 * 4.0 * spb * SRF) as usize + 9_600;
        let mut l = vec![0f32; n];
        let mut rng = Rng::new(3);
        let sr = SRF as f32;
        let fade = |i: usize, len: f32| ((len * sr - i as f32) / (0.01 * sr)).min(1.0);
        let mut add = |at: f64, x: Vec<f32>| {
            let a = (0.1 * SRF + at * spb * SRF) as usize;
            for (o, v) in l[a..].iter_mut().zip(x) {
                *o += v;
            }
        };
        let kick = || -> Vec<f32> {
            let mut ph = 0f32;
            (0..(0.4 * sr) as usize)
                .map(|i| {
                    let t = i as f32 / sr;
                    ph += std::f32::consts::TAU * (50.0 + 100.0 * (-t / 0.03).exp()) / sr;
                    0.9 * ph.sin() * (-t / 0.15).exp() * fade(i, 0.4)
                })
                .collect()
        };
        let mut noise = |lo: f64, hi: f64, decay: f32, gain: f32, len: f32| -> Vec<f32> {
            let mut x: Vec<f32> = (0..(len * sr) as usize).map(|_| rng.f32() * 2.0 - 1.0).collect();
            filter::butter(&mut x, true, lo, SRF, 4);
            filter::butter(&mut x, false, hi, SRF, 4);
            x.iter().enumerate().map(|(i, v)| gain * v * (-(i as f32 / sr) / decay).exp() * fade(i, len)).collect()
        };
        for b in 0..bars {
            let bar = b as f64 * 4.0;
            for k in 0..4 {
                add(bar + k as f64, kick());
                add(bar + k as f64 + 0.5, noise(6000.0, 16_000.0, 0.12, 0.3, 0.3));
            }
            for c in [1.0, 3.0, 3.75] {
                add(bar + c, noise(900.0, 4500.0, 0.05, 0.7, 0.2));
            }
        }
        Stereo { l: l.clone(), r: l }
    }

    #[test]
    fn a_clap_struck_with_the_kick_is_both_and_an_open_hat_is_no_snare() {
        let steps = fold(&hear(&house(4)), 100.0, 0.1, 1);
        assert_eq!(of(&steps, Kind::Kick), [0, 4, 8, 12]);
        assert_eq!(of(&steps, Kind::Snare), [4, 12, 15], "the claps on two and four, and the one alone");
        assert_eq!(of(&steps, Kind::Hat), [2, 6, 10, 14]);
    }

    /// a kit (or a mouth, `mouth`) playing `bars` bars at 100 BPM: kick on 1, 3 and the "and" of 3, snare on 2 and
    /// 4, hats on the eighths between, and a kick with a hat on top of it on 1
    fn beat(bars: usize, mouth: bool) -> Stereo {
        let spb = 0.6;
        let n = (bars as f64 * 4.0 * spb * SRF) as usize + 9_600;
        let mut l = vec![0f32; n];
        let mut rng = Rng::new(7);
        let mut add = |at: f64, x: Vec<f32>| {
            let a = (0.1 * SRF + at * spb * SRF) as usize;
            for (i, v) in x.into_iter().enumerate() {
                if a + i < n {
                    l[a + i] += v;
                }
            }
        };
        let sr = SRF as f32;
        // each sound fades out over its last 10 ms, as a sound that stops dead clicks
        let fade = |i: usize, len: f32| ((len * sr - i as f32) / (0.01 * sr)).min(1.0);
        let tone = |f0: f32, f1: f32, decay: f32, gain: f32, len: f32| -> Vec<f32> {
            let mut ph = 0f32;
            (0..(len * sr) as usize)
                .map(|i| {
                    let t = i as f32 / sr;
                    ph += std::f32::consts::TAU * (f1 + (f0 - f1) * (-t / 0.03).exp()) / sr;
                    gain * ph.sin() * (-t / decay).exp() * fade(i, len)
                })
                .collect()
        };
        let mut noise = |lo: f64, hi: f64, decay: f32, gain: f32, len: f32| -> Vec<f32> {
            let mut x: Vec<f32> = (0..(len * sr) as usize).map(|_| rng.f32() * 2.0 - 1.0).collect();
            filter::butter(&mut x, true, lo, SRF, 4);
            filter::butter(&mut x, false, hi, SRF, 4);
            x.iter().enumerate().map(|(i, v)| gain * v * (-(i as f32 / sr) / decay).exp() * fade(i, len)).collect()
        };
        for b in 0..bars {
            let bar = b as f64 * 4.0;
            for k in [0.0, 2.0, 2.5] {
                let kick = if mouth { tone(220.0, 90.0, 0.05, 0.7, 0.15) } else { tone(150.0, 50.0, 0.15, 0.9, 0.4) };
                add(bar + k, kick);
            }
            for s in [1.0, 3.0] {
                let mut snare =
                    if mouth { noise(900.0, 3500.0, 0.06, 0.8, 0.2) } else { noise(1000.0, 6000.0, 0.1, 0.6, 0.3) };
                if !mouth {
                    for (i, v) in tone(200.0, 190.0, 0.06, 0.25, 0.3).into_iter().enumerate() {
                        snare[i] += v;
                    }
                }
                add(bar + s, snare);
            }
            for h in [0.0, 0.5, 1.5, 3.5] {
                let hat = if mouth {
                    noise(5000.0, 11_000.0, 0.03, 0.5, 0.1)
                } else {
                    noise(7000.0, 16_000.0, 0.03, 0.35, 0.1)
                };
                add(bar + h, hat);
            }
        }
        Stereo { l: l.clone(), r: l }
    }

    fn steps(x: &Stereo, bars: u32) -> Vec<Steps> {
        fold(&hear(x), 100.0, 0.1, bars)
    }

    fn of(steps: &[Steps], kind: Kind) -> Vec<usize> {
        steps.iter().find(|s| s.kind == kind).map_or(vec![], |s| (0..s.v.len()).filter(|&i| s.v[i] > 0.0).collect())
    }

    #[test]
    fn a_kit_is_heard_as_its_kick_snare_and_hats() {
        let s = steps(&beat(4, false), 1);
        assert_eq!(of(&s, Kind::Kick), [0, 8, 10]);
        assert_eq!(of(&s, Kind::Snare), [4, 12]);
        assert_eq!(of(&s, Kind::Hat), [0, 2, 6, 14], "the hat on the kick too");
        assert!(s.iter().all(|k| k.chances().is_empty()), "every step every time round");
    }

    #[test]
    fn a_mouth_is_heard_as_a_kit() {
        let s = steps(&beat(4, true), 1);
        assert_eq!(of(&s, Kind::Kick), [0, 8, 10]);
        assert_eq!(of(&s, Kind::Snare), [4, 12]);
        assert_eq!(of(&s, Kind::Hat), [0, 2, 6, 14]);
    }

    #[test]
    fn a_step_hit_some_times_round_has_that_chance() {
        let hit = |t: f64, kind: Kind| Hit { t, kind, level: 0.5 };
        // four bars at 120 BPM (2 s a bar): a kick on every downbeat, a snare on beat 2 in two of the bars
        let hits = [
            hit(0.0, Kind::Kick),
            hit(2.0, Kind::Kick),
            hit(4.0, Kind::Kick),
            hit(6.0, Kind::Kick),
            hit(0.5, Kind::Snare),
            hit(4.5, Kind::Snare),
        ];
        let s = fold(&hits, 120.0, 0.0, 1);
        let snare = s.iter().find(|k| k.kind == Kind::Snare).unwrap();
        assert_eq!(snare.chances(), [(4, 0.5)]);
        assert_eq!(snare.string(), "....X...........");
        assert_eq!(s.iter().find(|k| k.kind == Kind::Kick).unwrap().chances(), []);
    }

    #[test]
    fn silence_has_no_drums() {
        assert!(hear(&Stereo::zeros(48_000)).is_empty());
    }
}

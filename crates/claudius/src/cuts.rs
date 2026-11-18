//! Where a recording's voice may be cut without cutting a word: the words it sings and the phrases they make, found in
//! its loudness where a sung pitch is near (a breath, a click or a drum alone is no word), and a cut in the gap before
//! or after a phrase, in the stretch of quiet nearest it and on a zero crossing, so that neither a word's consonant nor
//! a held note's release is cut short. A singer's rests and breaths part the phrases (Bohak and Marolt, ISMIR 2012; Nakano et al., ICMPC 2008),
//! and a cut goes back to the quiet before an onset (Jehan, 2005).

use crate::audio::Stereo;
use crate::config::SRF;

/// the loudness and the pitch are read every 10 ms
const HOP: f64 = 0.01;
/// a word is heard this far under the voice's loudest in the two seconds round it (dB): its reverb and what bleeds
/// into it lie below
const BELOW_DB: f64 = 24.0;
/// and this far under its loudest anywhere (dB)
const FLOOR_DB: f64 = 40.0;
/// a word heard goes on, before and after, while it is no further than this under the voice's loudest round it (dB):
/// a soft onset, a held note fading
const EDGE_DB: f64 = 38.0;
/// a sound this near a sung pitch belongs to the word (its consonants), seconds
const NEAR: f64 = 0.06;
/// words parted by less than this run together, seconds
const JOIN: f64 = 0.04;
/// a valley at least this much under the word either side of it parts them (dB)
const VALLEY_DB: f64 = 10.0;
/// how far either side of a valley the words it parts are looked for, seconds
const VALLEY_REACH: f64 = 0.2;
/// the shortest word, seconds
const SHORTEST: f64 = 0.06;
/// a rest at least this long ends a phrase, seconds
const REST: f64 = 0.25;
/// kept before whatever is sung after a cut, seconds
const BEFORE: f64 = 0.03;
/// the furthest a cut before a phrase looks for quiet, and after one, seconds
const LEAD: f64 = 0.3;
const AFTER: f64 = 0.5;
/// a moment no more than this over the quietest of the gap it is in is quiet (dB), and so is one under `SILENT`
/// (dBFS); a cut goes in a stretch of quiet at least `QUIET_FOR` long (seconds): a shorter one is a consonant's
/// closure, or a dip between syllables
const QUIET_DB: f64 = 6.0;
const SILENT: f64 = -60.0;
const QUIET_FOR: f64 = 0.06;
/// a phrase whose loudness in the vocals follows the rest's at least this closely (Pearson's r, every 10 ms) is an
/// instrument the separation split between them, not a voice (`voice_of`)
const FOLLOWS: f64 = 0.2;

/// a stretch of a recording, in seconds from its start
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Span {
    pub start: f64,
    pub end: f64,
}

impl Span {
    pub fn len(&self) -> f64 {
        self.end - self.start
    }

    /// whether a cut at `t` would fall inside it
    pub fn cut_by(&self, t: f64) -> bool {
        t > self.start && t < self.end
    }
}

/// What a voice sings: its words in the order sung, or their syllables where the singer parts them, and its phrases,
/// each the words between two rests. A cut between two words cuts neither, but may part a word's syllables: a
/// sample is cut between phrases.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sung {
    pub words: Vec<Span>,
    pub phrases: Vec<Span>,
}

impl Sung {
    /// the words a phrase holds
    pub fn words_of(&self, phrase: Span) -> impl Iterator<Item = &Span> {
        self.words.iter().filter(move |w| w.start >= phrase.start - 1e-9 && w.end <= phrase.end + 1e-9)
    }

    /// whether a cut at `t` would cut a word
    pub fn cuts_a_word(&self, t: f64) -> bool {
        self.words.iter().any(|w| w.cut_by(t))
    }
}

/// The words and phrases `voice` sings, `pitches` its sung pitch every 10 ms (`analysis::melody::pitches`). The voice
/// is a recording's vocals apart from the rest, or its mix, where the pitch tells the voice from the drums.
pub fn sung(voice: &Stereo, pitches: &[Option<f32>]) -> Sung {
    let db = loudness(voice);
    let n = db.len();
    if n == 0 {
        return Sung::default();
    }
    let loudest = db.iter().copied().fold(f64::MIN, f64::max);
    let reach = (1.0 / HOP) as usize;
    let near = (NEAR / HOP).round() as usize;
    // a sung pitch in a voice's range, A2 to C6
    let voiced: Vec<bool> =
        (0..n).map(|i| pitches.get(i).copied().flatten().is_some_and(|m| (45.0..=84.0).contains(&m))).collect();
    let mut voiced_near = vec![false; n];
    for (i, v) in voiced.iter().enumerate() {
        if *v {
            for j in i.saturating_sub(near)..(i + near + 1).min(n) {
                voiced_near[j] = true;
            }
        }
    }
    let top = moving_max(&db, reach);
    let loud: Vec<bool> =
        (0..n).map(|i| voiced_near[i] && db[i] > top[i] - BELOW_DB && db[i] > loudest - FLOOR_DB).collect();
    let audible = |i: usize| db[i] > top[i] - EDGE_DB && db[i] > loudest - FLOOR_DB - 10.0;
    // runs of loud frames, each widened over its soft onset and its release, joined over a short dip, then parted at
    // deep valleys
    let mut runs: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i < n {
        if !loud[i] {
            i += 1;
            continue;
        }
        let mut start = i;
        while i < n && loud[i] {
            i += 1;
        }
        let floor = runs.last().map_or(0, |r| r.1);
        while start > floor && audible(start - 1) {
            start -= 1;
        }
        while i < n && audible(i) {
            i += 1;
        }
        match runs.last_mut() {
            Some(last) if start.saturating_sub(last.1) as f64 * HOP < JOIN => last.1 = i,
            _ => runs.push((start, i)),
        }
    }
    let valley = (VALLEY_REACH / HOP) as usize;
    let mut words: Vec<Span> = Vec::new();
    for (a, b) in runs {
        let mut from = a;
        for m in a + 1..b.saturating_sub(1) {
            let local_min = db[m] <= db[m - 1] && db[m] <= db[m + 1];
            if !local_min {
                continue;
            }
            let left = db[m.saturating_sub(valley).max(from)..m].iter().copied().fold(f64::MIN, f64::max);
            let right = db[m + 1..(m + valley + 1).min(b)].iter().copied().fold(f64::MIN, f64::max);
            let long_enough = |x: usize, y: usize| (y - x) as f64 * HOP >= SHORTEST;
            if db[m] < left.min(right) - VALLEY_DB && long_enough(from, m) && long_enough(m, b) {
                words.push(span(from, m));
                from = m;
            }
        }
        words.push(span(from, b));
    }
    // a word holds a sung pitch: a breath, a click, a consonant alone is no word
    words.retain(|w| {
        let (a, b) = ((w.start / HOP) as usize, ((w.end / HOP) as usize).min(n));
        w.len() >= SHORTEST && voiced[a..b].iter().filter(|v| **v).count() as f64 * HOP >= 0.03
    });
    let mut phrases: Vec<Span> = Vec::new();
    for w in &words {
        match phrases.last_mut() {
            Some(p) if w.start - p.end < REST => p.end = w.end,
            _ => phrases.push(*w),
        }
    }
    Sung { words, phrases }
}

fn span(a: usize, b: usize) -> Span {
    Span { start: a as f64 * HOP, end: b as f64 * HOP }
}

/// The phrases of `sung` a voice sings, and their words: those whose loudness in `vocals` does not rise and fall with
/// that of `rest`, what plays besides the voice. A separation hears a riff, a lead or a pad as partly a voice (a
/// distorted guitar most of all) and splits it between the vocals and the rest, in both of which it then comes and
/// goes at once; a voice comes and goes on its own, and the rest gives way to it where it sings.
pub fn voice_of(sung: &Sung, vocals: &Stereo, rest: &Stereo) -> Sung {
    let (v, r) = (levels(vocals), levels(rest));
    let phrases: Vec<Span> = sung.phrases.iter().copied().filter(|p| follows(&v, &r, *p) < FOLLOWS).collect();
    let words = phrases.iter().flat_map(|p| sung.words_of(*p).copied().collect::<Vec<_>>()).collect();
    Sung { words, phrases }
}

/// how closely `a` follows `b` over `span`, levels every 10 ms: Pearson's r of the two there, 0 where either holds
/// still
fn follows(a: &[f64], b: &[f64], span: Span) -> f64 {
    let (i, j) = ((span.start / HOP) as usize, ((span.end / HOP) as usize).min(a.len()).min(b.len()));
    if j < i + 3 {
        return 0.0;
    }
    let (a, b) = (&a[i..j], &b[i..j]);
    let n = a.len() as f64;
    let (ma, mb) = (a.iter().sum::<f64>() / n, b.iter().sum::<f64>() / n);
    let (mut ab, mut aa, mut bb) = (0.0, 0.0, 0.0);
    for (x, y) in a.iter().zip(b) {
        ab += (x - ma) * (y - mb);
        aa += (x - ma).powi(2);
        bb += (y - mb).powi(2);
    }
    if aa > 0.0 && bb > 0.0 { ab / (aa * bb).sqrt() } else { 0.0 }
}

/// the loudness of `x` every 10 ms, in dB, over 20 ms round each
fn loudness(x: &Stereo) -> Vec<f64> {
    power(x).into_iter().map(|e| 10.0 * (e + 1e-12).log10()).collect()
}

/// the level of `x` every 10 ms (its RMS over 20 ms round each)
fn levels(x: &Stereo) -> Vec<f64> {
    power(x).into_iter().map(f64::sqrt).collect()
}

/// the power of `x` every 10 ms, over 20 ms round each
fn power(x: &Stereo) -> Vec<f64> {
    let hop = (HOP * SRF) as usize;
    let n = x.len() / hop;
    (0..n)
        .map(|f| {
            let (a, b) = ((f * hop).saturating_sub(hop / 2), ((f + 1) * hop + hop / 2).min(x.len()));
            (a..b).map(|i| (0.5 * (x.l[i] + x.r[i]) as f64).powi(2)).sum::<f64>() / (b - a).max(1) as f64
        })
        .collect()
}

/// the loudest of `v` within `reach` either side of each
fn moving_max(v: &[f64], reach: usize) -> Vec<f64> {
    (0..v.len())
        .map(|i| v[i.saturating_sub(reach)..(i + reach + 1).min(v.len())].iter().copied().fold(f64::MIN, f64::max))
        .collect()
}

/// Where a cut before a phrase starting at `start` goes, `after` the end of whatever is sung before it: at the end of
/// the last stretch of quiet before it, at most `LEAD` before it, so that a consonant or a breath heard late is kept
/// (at the quietest moment, in a gap with no such stretch), on a zero crossing of `x`; or on the bar line `bar`, when
/// that falls in the same quiet, so that the phrase's bars start with the one it is sung in.
pub fn cut_before(x: &Stereo, start: f64, after: f64, bar: f64) -> f64 {
    let lo = (start - LEAD).max(after).max(0.0);
    let hi = (start - 0.005).max(lo);
    match quiet(x, lo, hi).last() {
        Some(s) if (s.start..=s.end).contains(&bar) => bar,
        Some(s) => zero_near(x, s.end, 0.002),
        None => zero_near(x, quietest(x, lo, hi), 0.002),
    }
}

/// Where a cut after a phrase ending at `end` goes, `before` the start of whatever is sung after it: at the start of
/// the first stretch of quiet after it, at most `AFTER` past it, so that a held note is kept to the end of its
/// release (at the quietest moment, in a gap with no such stretch), on a zero crossing of `x`.
pub fn cut_after(x: &Stereo, end: f64, before: f64) -> f64 {
    let hi = (end + AFTER).min(before - BEFORE).min(x.secs());
    let lo = (end + 0.01).min(hi).max(end);
    let t = quiet(x, lo, hi).first().map_or_else(|| quietest(x, lo, hi), |s| s.start);
    zero_near(x, t, 0.002)
}

/// the start of the quietest 5 ms of `x` between `lo` and `hi`, in seconds
fn quietest(x: &Stereo, lo: f64, hi: f64) -> f64 {
    let w = (0.005 * SRF) as usize;
    let (a, b) = ((lo * SRF) as usize, ((hi * SRF) as usize).min(x.len().saturating_sub(w)));
    if b <= a {
        return lo;
    }
    let energy = |s: usize| (s..s + w).map(|i| (0.5 * (x.l[i] + x.r[i]) as f64).powi(2)).sum::<f64>();
    let step = (w / 5).max(1);
    let best = (a..=b).step_by(step).min_by(|p, q| energy(*p).total_cmp(&energy(*q))).unwrap_or(a);
    best as f64 / SRF
}

/// the stretches of quiet in `x` between `lo` and `hi`, each at least `QUIET_FOR` long and 5 ms inside the sound
/// either side of it: where every 10 ms is quiet
fn quiet(x: &Stereo, lo: f64, hi: f64) -> Vec<Span> {
    let w = (0.01 * SRF) as usize;
    let step = (0.002 * SRF) as usize;
    let (a, b) = ((lo * SRF) as usize, ((hi * SRF) as usize).min(x.len()).saturating_sub(w));
    if b <= a {
        return Vec::new();
    }
    let marks: Vec<(usize, f64)> = (a..=b)
        .step_by(step)
        .map(|s| {
            let e = (s..s + w).map(|i| (0.5 * (x.l[i] + x.r[i]) as f64).powi(2)).sum::<f64>() / w as f64;
            (s, 10.0 * (e + 1e-12).log10())
        })
        .collect();
    let floor = marks.iter().map(|m| m.1).fold(f64::MAX, f64::min);
    let still = (floor + QUIET_DB).max(SILENT);
    let mut out = Vec::new();
    let mut keep = |first: usize, last: usize| {
        let s = Span { start: first as f64 / SRF + 0.005, end: (last + w) as f64 / SRF - 0.005 };
        if s.len() >= QUIET_FOR {
            out.push(s);
        }
    };
    let mut run: Option<(usize, usize)> = None;
    for &(s, db) in &marks {
        if db <= still {
            run = Some(run.map_or((s, s), |(first, _)| (first, s)));
        } else if let Some((first, last)) = run.take() {
            keep(first, last);
        }
    }
    if let Some((first, last)) = run {
        keep(first, last);
    }
    out
}

/// the zero crossing of `x` nearest `t`, within `reach` seconds of it; `t` where it has none
fn zero_near(x: &Stereo, t: f64, reach: f64) -> f64 {
    let mid = |i: usize| 0.5 * (x.l[i] + x.r[i]);
    let (c, r) = ((t * SRF) as usize, (reach * SRF) as usize);
    for d in 0..=r {
        for i in [c.saturating_sub(d), c + d] {
            if i + 1 < x.len() && (mid(i) <= 0.0) != (mid(i + 1) <= 0.0) {
                return (i + 1) as f64 / SRF;
            }
        }
    }
    t
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A line sung over drums panned hard to the sides, and the words in it: each word a syllable or two of a sung A3
    /// or C4 (a consonant's hiss before it), the words of a phrase parted by short gaps, the phrases by rests.
    pub(crate) fn sung_over_drums(phrases: &[(f64, &[(f64, u32)])], secs: f64) -> (Stereo, Vec<Span>) {
        let n = (secs * SRF) as usize;
        let mut x = Stereo::zeros(n);
        let mut rng = crate::rng::Rng::new(7);
        // a kick on the beats to the left, hats on the eighths to the right, at 120 BPM
        for i in 0..n {
            let t = i as f64 / SRF;
            let beat = t % 0.5;
            if beat < 0.08 {
                x.l[i] += ((std::f64::consts::TAU * 55.0 * beat).sin() * (-beat * 40.0).exp() * 0.5) as f32;
            }
            if t % 0.25 < 0.02 {
                x.r[i] += (rng.f32() - 0.5) * 0.3;
            }
        }
        let mut words = Vec::new();
        let mut phase = 0f64;
        for (start, line) in phrases {
            let mut t = *start;
            for (k, (len, syllables)) in line.iter().enumerate() {
                let (w0, w1) = (t, t + len);
                words.push(Span { start: w0, end: w1 });
                // a consonant's hiss, then the syllables of the word, a dip between them
                let hiss = 0.03;
                let syl = (len - hiss) / *syllables as f64;
                let f0 = if k % 2 == 0 { 220.0 } else { 261.6 };
                for i in (w0 * SRF) as usize..((w1 * SRF) as usize).min(n) {
                    let u = i as f64 / SRF - w0;
                    let v = if u < hiss {
                        ((rng.f32() - 0.5) * 0.2) as f64
                    } else {
                        let s = (u - hiss) % syl;
                        let env = (s / 0.015).min(1.0) * ((syl - s) / 0.03).min(1.0) * (0.6 + 0.4 * (s / syl));
                        phase += std::f64::consts::TAU * f0 / SRF;
                        env * (phase.sin() * 0.4 + (2.0 * phase).sin() * 0.15 + (3.0 * phase).sin() * 0.06)
                    };
                    x.l[i] += v as f32;
                    x.r[i] += v as f32;
                }
                t = w1 + 0.09;
            }
        }
        (x, words)
    }

    #[test]
    fn the_words_and_phrases_of_a_line_are_heard_where_they_are_sung() {
        // two phrases, of three words and of two, the second after a rest of a second
        let line1: &[(f64, u32)] = &[(0.4, 1), (0.55, 2), (0.3, 1)];
        let line2: &[(f64, u32)] = &[(0.6, 2), (0.45, 1)];
        let (x, words) = sung_over_drums(&[(1.03, line1), (3.27, line2)], 6.0);
        let sung = sung(&x, &crate::analysis::melody::pitches(&x));
        // each heard is a word, or a syllable of one, and never spans a gap
        for heard in &sung.words {
            let within = words.iter().any(|w| heard.start >= w.start - 0.04 && heard.end <= w.end + 0.06);
            assert!(within, "{heard:?} spans a gap: {words:?}");
        }
        for truth in &words {
            let covered: f64 =
                sung.words.iter().map(|h| (h.end.min(truth.end) - h.start.max(truth.start)).max(0.0)).sum();
            assert!(covered >= 0.85 * truth.len(), "{truth:?} heard as {:?}", sung.words);
        }
        let phrases: Vec<(f64, f64)> = sung.phrases.iter().map(|p| (p.start, p.end)).collect();
        let truth = [(1.03, words[2].end), (3.27, words[4].end)];
        assert_eq!(phrases.len(), 2, "{phrases:?}");
        for ((s, e), (ts, te)) in phrases.iter().zip(truth) {
            // a kick under a phrase's first word may start it a little early, which no cut minds
            assert!((s - ts).abs() <= 0.06 && (e - te).abs() <= 0.06, "{phrases:?} against {truth:?}");
        }
    }

    #[test]
    fn a_cut_before_or_after_a_phrase_falls_in_its_gap_on_a_zero_crossing() {
        let line: &[(f64, u32)] = &[(0.4, 1), (0.5, 2)];
        let (x, words) = sung_over_drums(&[(1.03, line)], 3.0);
        let (first, last) = (words[0], words[1]);
        let before = cut_before(&x, first.start, 0.0, 0.0);
        assert!(before < first.start && first.start - before <= LEAD + 1e-9, "{before}");
        let after = cut_after(&x, last.end, x.secs());
        assert!(after >= last.end && after - last.end <= AFTER + 0.003, "{after}");
        for t in [before, after] {
            let i = (t * SRF) as usize;
            let mid = |i: usize| 0.5 * (x.l[i] + x.r[i]);
            assert!((mid(i - 1) <= 0.0) != (mid(i) <= 0.0) || mid(i).abs() < 1e-3, "on a zero crossing at {t}");
        }
    }

    #[test]
    fn a_cut_keeps_a_consonant_before_its_closure_and_a_held_note_to_the_end_of_its_release() {
        // a hiss, a closure of 25 ms, then a vowel held for a second that fades out over a quarter of a second, heard
        // from the vowel to the start of its fade
        let n = (3.0 * SRF) as usize;
        let mut x = Stereo::zeros(n);
        let mut rng = crate::rng::Rng::new(3);
        for i in 0..n {
            let t = i as f64 / SRF;
            let v = if (1.0..1.06).contains(&t) {
                ((rng.f32() - 0.5) * 0.1) as f64
            } else if (1.085..2.385).contains(&t) {
                let env = if t < 2.085 { 1.0 } else { (-(t - 2.085) / 0.05).exp() };
                env * 0.4 * (std::f64::consts::TAU * 220.0 * t).sin()
            } else {
                0.0
            };
            (x.l[i], x.r[i]) = (v as f32, v as f32);
        }
        let before = cut_before(&x, 1.085, 0.0, 0.0);
        assert!((0.7..=1.0).contains(&before), "the hiss kept: {before}");
        // on a bar line in the quiet before it, and not on one that falls in its hiss
        assert_eq!(cut_before(&x, 1.085, 0.0, 0.9), 0.9);
        assert!(cut_before(&x, 1.085, 0.0, 1.03) <= 1.0);
        let after = cut_after(&x, 2.085, x.secs());
        assert!(after >= 2.3, "the release kept: {after}");
    }

    #[test]
    fn drums_alone_are_no_words() {
        let (x, _) = sung_over_drums(&[], 4.0);
        assert_eq!(sung(&x, &crate::analysis::melody::pitches(&x)), Sung::default());
    }

    #[test]
    fn a_riff_the_separation_splits_between_the_vocals_and_the_rest_is_no_voice() {
        // a riff of eighths on an F#3 all through, split between the vocals and the rest as a separation splits a
        // distorted guitar; from 6 s a voice sings two phrases, and the vocals hold little of the riff while it does
        let n = (12.0 * SRF) as usize;
        let (mut vocals, mut rest) = (Stereo::zeros(n), Stereo::zeros(n));
        let tone = |f0: f64, t: f64| {
            let p = std::f64::consts::TAU * f0 * t;
            p.sin() * 0.5 + (2.0 * p).sin() * 0.25 + (3.0 * p).sin() * 0.12
        };
        let mut words = Vec::new();
        for (start, lens) in [(6.2, [0.35, 0.5, 0.3]), (8.6, [0.45, 0.3, 0.55])] {
            let mut t = start;
            for len in lens {
                words.push(Span { start: t, end: t + len });
                t += len + 0.09;
            }
        }
        for i in 0..n {
            let t = i as f64 / SRF;
            let s = t % 0.25;
            let riff = (s / 0.005).min(1.0) * (-s / 0.07).exp() * tone(185.0, t) * 0.6;
            let voice = words.iter().enumerate().find(|(_, w)| (w.start..w.end).contains(&t)).map_or(0.0, |(k, w)| {
                let u = t - w.start;
                (u / 0.02).min(1.0) * ((w.end - t) / 0.03).min(1.0) * tone(if k % 2 == 0 { 220.0 } else { 261.6 }, t)
            });
            let share = if t < 6.0 { 0.6 } else { 0.08 };
            let v = (share * riff + 0.6 * voice) as f32;
            let r = ((1.0 - share) * riff) as f32;
            (vocals.l[i], vocals.r[i], rest.l[i], rest.r[i]) = (v, v, r, r);
        }
        let heard = sung(&vocals, &crate::analysis::melody::pitches(&vocals));
        assert!(heard.phrases.iter().any(|p| p.start < 5.0), "the riff in the vocals heard as words: {heard:?}");
        let voice = voice_of(&heard, &vocals, &rest);
        assert!(voice.phrases.iter().all(|p| p.start >= 5.9), "only the voice's phrases: {:?}", voice.phrases);
        for w in &words {
            let mid = 0.5 * (w.start + w.end);
            assert!(voice.phrases.iter().any(|p| p.cut_by(mid)), "{w:?} sung, in {:?}", voice.phrases);
            assert!(voice.words.iter().any(|h| h.cut_by(mid)), "{w:?} sung, heard as {:?}", voice.words);
        }
    }
}

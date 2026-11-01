//! How late a take lands, measured. Claudius plays clicks at known times through the output and records them through
//! the input, as it plays a song and records a take, and finds where in the recording each click arrived. What the
//! devices do not report (the processing in a laptop's speakers and microphone, a Bluetooth link) shows as the clicks
//! arriving late, by as much as a take would.

use super::Pcm;

/// when the clicks play, in seconds from the start: unevenly apart, so that only the true lag lines them all up
pub const CLICKS: [f64; 8] = [0.4, 0.95, 1.45, 2.1, 2.6, 3.3, 3.75, 4.45];

/// how late a click may come back, and how early (when the devices report more than they do), in seconds
pub const LATEST: f64 = 0.6;
const EARLIEST: f64 = -0.1;

/// the clicks that must be heard of the eight, and how far from the others' middle one may land, in seconds
const ENOUGH: usize = 5;
const AGREE: f64 = 0.0015;

/// one click: a few cycles at 3 kHz, which small speakers play and a microphone hears sharply, from silence
fn click(rate: f64) -> impl Iterator<Item = f32> {
    (0..(0.004 * rate) as usize).map(move |n| {
        let t = n as f64 / rate;
        (0.5 * (std::f64::consts::TAU * 3000.0 * t).sin() * (-t / 0.0008).exp()) as f32
    })
}

/// what a measurement plays: the clicks, mono, at `rate`, and long enough for the last to come back
pub fn clicks(rate: u32) -> Pcm {
    let r = rate as f64;
    let mut samples = vec![0.0; ((CLICKS[CLICKS.len() - 1] + 0.2) * r) as usize];
    for t in CLICKS {
        let at = (t * r).round() as usize;
        for (s, c) in samples[at..].iter_mut().zip(click(r)) {
            *s = c;
        }
    }
    Pcm { rate, channels: 1, samples }
}

/// a second-order high-pass at `hz`: what the clicks are heard above (hum, rumble and most of a voice are not)
fn highpass(x: &[f32], rate: f64, hz: f64) -> Vec<f32> {
    let w = std::f64::consts::TAU * hz / rate;
    let alpha = w.sin() / std::f64::consts::SQRT_2;
    let a0 = 1.0 + alpha;
    let b = [(1.0 + w.cos()) / 2.0 / a0, -(1.0 + w.cos()) / a0, (1.0 + w.cos()) / 2.0 / a0];
    let a = [-2.0 * w.cos() / a0, (1.0 - alpha) / a0];
    let (mut x1, mut x2, mut y1, mut y2) = (0.0, 0.0, 0.0, 0.0);
    x.iter()
        .map(|&v| {
            let v = v as f64;
            let y = b[0] * v + b[1] * x1 + b[2] * x2 - a[0] * y1 - a[1] * y2;
            (x2, x1, y2, y1) = (x1, v, y1, y);
            y as f32
        })
        .collect()
}

/// How late the clicks came back in `heard`, in seconds: what the input captured, mono at `rate`, from the moment the
/// measurement began as the devices' timestamps tell it. Each click is found where it first arrives (the sound straight
/// from the speaker, not its echoes), and the answer is the middle of their lateness; or why there is none (an input
/// that fell silent in places is processed, which no measurement gets through).
#[cfg(any(not(target_arch = "wasm32"), test))]
pub fn lateness(heard: &[f32], rate: f64) -> Result<f64, String> {
    lateness_where(heard, rate, super::take::hushed(heard, 1, rate))
}

/// `lateness`, told how often the input fell silent (`take::hushed`): when what is heard holds silence that is not the
/// input's (a browser's lost frames, filled in), the recorder knows better than the samples do
pub fn lateness_where(heard: &[f32], rate: f64, hushed: usize) -> Result<f64, String> {
    find(heard, rate).map_err(|e| if hushed > 0 { format!("{}: then measure again", super::take::HUSHED) } else { e })
}

fn find(heard: &[f32], rate: f64) -> Result<f64, String> {
    let deaf = "claudius did not hear its clicks: turn the speakers up, or hold the headphones to the microphone, and \
                measure again";
    let x = highpass(heard, rate, 1000.0);
    let abs = |n: i64| if n >= 0 { x.get(n as usize).map_or(0.0, |v| v.abs()) } else { 0.0 };
    // first, roughly: the loudest of each millisecond, and the lag at which enough clicks line up with something loud
    // (a lag is as good as the quietest of its best `ENOUGH`, so that one bang lines up nothing)
    let at = |t: f64| (t * rate).floor() as i64;
    let loudest: Vec<f32> = (0..(x.len() as f64 / rate * 1000.0) as i64)
        .map(|k| (at(k as f64 / 1000.0)..at((k + 1) as f64 / 1000.0)).map(abs).fold(0.0, f32::max))
        .collect();
    let ms = |t: f64| (t * 1000.0).round() as i64;
    let block = |k: i64| if k >= 0 { loudest.get(k as usize).copied().unwrap_or(0.0) } else { 0.0 };
    let score = |lag: i64| {
        let mut each: Vec<f32> = CLICKS.iter().map(|&t| block(ms(t) + lag).max(block(ms(t) + lag + 1))).collect();
        each.sort_by(|a, b| b.total_cmp(a));
        each[ENOUGH - 1]
    };
    let lags = ms(EARLIEST)..=ms(LATEST);
    let (best, top) = lags.clone().map(|l| (l, score(l))).fold((0, 0.0), |b, s| if s.1 > b.1 { s } else { b });
    // echoes follow a click by up to tens of milliseconds: a rival is a lag that lines up other sounds
    let rival = lags.filter(|l| (l - best).abs() > 20).map(score).fold(0.0, f32::max);
    if top <= 0.0 || top < 2.0 * rival {
        return Err(deaf.into());
    }
    // the room before the first click: what a click must stand well above
    let mut quiet: Vec<f32> = (0..at(CLICKS[0] + EARLIEST - 0.02).max(0)).map(abs).collect();
    quiet.sort_by(f32::total_cmp);
    let noise = quiet.get(quiet.len() * 9 / 10).copied().unwrap_or(0.0);
    // then exactly: where each click first arrives, near that lag
    let mut late: Vec<f64> = CLICKS
        .iter()
        .filter_map(|&t| {
            let near = t + best as f64 / 1000.0;
            let (from, to) = (at(near - 0.015), at(near + 0.006));
            let peak = (from..=to).map(abs).fold(0.0, f32::max);
            if peak < 4.0 * noise || peak <= 0.0 {
                return None;
            }
            let rise = (0.3 * peak).max(3.0 * noise);
            (from..=to).find(|&n| abs(n) >= rise).map(|n| n as f64 / rate - t)
        })
        .collect();
    if late.len() < ENOUGH {
        return Err(deaf.into());
    }
    late.sort_by(f64::total_cmp);
    let middle = late[late.len() / 2];
    let agree: Vec<f64> = late.iter().copied().filter(|l| (l - middle).abs() <= AGREE).collect();
    if agree.len() < ENOUGH {
        let (a, b) = (late[0] * 1000.0, late[late.len() - 1] * 1000.0);
        return Err(format!(
            "the clicks came back at different times, {a:.0} to {b:.0} ms late: measure again somewhere quieter, \
             with the microphone nearer the speakers"
        ));
    }
    Ok(agree[agree.len() / 2])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// a cheap repeatable noise, -1..1
    fn noise(seed: u64) -> impl FnMut() -> f32 {
        let mut s = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        move || {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((s >> 40) as f32 / (1u64 << 24) as f32) * 2.0 - 1.0
        }
    }

    /// how a room hears what a speaker plays
    struct Room {
        /// seconds from the output's timestamps to the input's, beyond what they report
        late: f64,
        gain: f32,
        /// echoes after the sound straight from the speaker: seconds after it, and how loud
        echoes: Vec<(f64, f32)>,
        /// a reverberant tail, how loud
        tail: f32,
        /// the room's own noise, and mains hum, how loud
        hiss: f32,
        hum: f32,
        seed: u64,
    }

    impl Default for Room {
        fn default() -> Self {
            Room {
                late: 0.044,
                gain: 0.3,
                echoes: vec![(0.0023, 0.6), (0.0079, -0.4)],
                tail: 0.05,
                hiss: 0.003,
                hum: 0.0,
                seed: 1,
            }
        }
    }

    impl Room {
        /// what the microphone hears of `played` (mono, at `rate`), and how late it truly is, to the sample
        fn hear(&self, played: &[f32], rate: f64) -> (Vec<f32>, f64) {
            let shift = (self.late * rate).round() as i64;
            let len = played.len() + (0.3 * rate) as usize;
            // a small speaker: nothing much below 300 Hz or above 8 kHz
            let (mut lp, mut hp, mut prev) = (0f32, 0f32, 0f32);
            let (kl, kh) = (
                (-std::f64::consts::TAU * 8000.0 / rate).exp() as f32,
                (-std::f64::consts::TAU * 300.0 / rate).exp() as f32,
            );
            let speaker: Vec<f32> = played
                .iter()
                .map(|&v| {
                    lp = lp * kl + v * (1.0 - kl);
                    hp = kh * (hp + lp - prev);
                    prev = lp;
                    hp
                })
                .collect();
            let mut out = vec![0f32; len];
            let mut put = |at: i64, v: f32| {
                if at >= 0 && (at as usize) < len {
                    out[at as usize] += v;
                }
            };
            let mut rnd = noise(self.seed);
            let tail: Vec<f32> = (0..(0.15 * rate) as usize)
                .map(|n| rnd() * self.tail * (-(n as f64) / rate / 0.04).exp() as f32)
                .collect();
            for (n, &v) in speaker.iter().enumerate().filter(|(_, v)| v.abs() > 1e-6) {
                let at = n as i64 + shift;
                put(at, v * self.gain);
                for &(after, g) in &self.echoes {
                    put(at + (after * rate).round() as i64, v * self.gain * g);
                }
                for (k, &t) in tail.iter().enumerate().step_by(7) {
                    put(at + (0.012 * rate) as i64 + k as i64, v * self.gain * t);
                }
            }
            let mut hiss = noise(self.seed + 1);
            for (n, s) in out.iter_mut().enumerate() {
                let hum = (std::f64::consts::TAU * 50.0 * n as f64 / rate).sin() as f32 * self.hum;
                *s += hiss() * self.hiss + hum;
            }
            (out, shift as f64 / rate)
        }
    }

    fn measure(room: &Room, rate: u32) -> (Result<f64, String>, f64) {
        let (heard, truth) = room.hear(&clicks(rate).samples, rate as f64);
        (lateness(&heard, rate as f64), truth)
    }

    fn within(room: &Room, rate: u32, by: f64) {
        let (got, truth) = measure(room, rate);
        let got = got.unwrap_or_else(|e| panic!("{e} (truth {:.1} ms)", truth * 1000.0));
        assert!((got - truth).abs() <= by, "measured {:.2} ms, truly {:.2} ms", got * 1000.0, truth * 1000.0);
    }

    #[test]
    fn the_clicks_are_short_sharp_and_where_they_should_be() {
        let pcm = clicks(48_000);
        assert_eq!((pcm.rate, pcm.channels), (48_000, 1));
        assert!((pcm.samples.len() as f64 / 48_000.0 - (CLICKS[7] + 0.2)).abs() < 1e-3);
        let peak = pcm.samples.iter().fold(0f32, |m, s| m.max(s.abs()));
        assert!(peak > 0.4 && peak <= 0.5, "loud enough, and not too loud: {peak}");
        for t in CLICKS {
            let at = (t * 48_000.0).round() as usize;
            assert_eq!(pcm.samples[at - 1], 0.0, "silence before each click");
            assert!(pcm.samples[at + 1..at + 8].iter().any(|s| s.abs() > 0.2), "and it starts at once");
            assert!(pcm.samples[at + 200..at + 400].iter().all(|s| s.abs() == 0.0), "it is over in 4 ms");
        }
        // unevenly apart, so that no lag in the range lines up two pairs the way the true one lines up all of them
        let gaps: Vec<i64> = CLICKS.windows(2).map(|w| ((w[1] - w[0]) * 1000.0).round() as i64).collect();
        assert!(gaps.iter().all(|&g| g as f64 > (LATEST - EARLIEST) * 1000.0 / 2.0), "{gaps:?}");
    }

    #[test]
    fn it_finds_how_late_the_clicks_come_back() {
        for late in [0.0, 0.0123, 0.044, 0.15, 0.31, 0.55] {
            for rate in [44_100, 48_000, 96_000] {
                within(&Room { late, ..Default::default() }, rate, 0.0005);
            }
        }
    }

    #[test]
    fn early_clicks_are_found_too() {
        // devices that report more latency than they have: a take would land early, and wants less
        for late in [-0.02, -0.08] {
            within(&Room { late, ..Default::default() }, 48_000, 0.0005);
        }
    }

    #[test]
    fn it_takes_the_sound_straight_from_the_speaker_over_a_louder_echo() {
        let room = Room { echoes: vec![(0.004, 1.8), (0.011, 1.2)], ..Default::default() };
        within(&room, 48_000, 0.0005);
    }

    #[test]
    fn quiet_clicks_in_a_noisy_room_with_hum_still_count() {
        let room = Room { gain: 0.02, hiss: 0.0015, hum: 0.2, tail: 0.2, ..Default::default() };
        within(&room, 48_000, 0.0005);
    }

    #[test]
    fn a_clipping_input_or_one_that_inverts_still_counts() {
        let (heard, truth) = Room::default().hear(&clicks(48_000).samples, 48_000.0);
        let clipped: Vec<f32> = heard.iter().map(|s| (s * 40.0).clamp(-1.0, 1.0)).collect();
        let inverted: Vec<f32> = heard.iter().map(|s| -s).collect();
        for x in [clipped, inverted] {
            assert!((lateness(&x, 48_000.0).unwrap() - truth).abs() <= 0.0005);
        }
    }

    #[test]
    fn missing_clicks_and_a_bang_in_between_do_not_fool_it() {
        let rate = 48_000.0;
        let (mut heard, truth) = Room::default().hear(&clicks(48_000).samples, rate);
        // the input drops two clicks, and something falls over between two others
        for t in [CLICKS[1], CLICKS[5]] {
            let at = ((t + 0.044) * rate) as usize;
            heard[at - 500..at + 3000].fill(0.0);
        }
        let mut rnd = noise(9);
        let bang = ((CLICKS[3] + 0.3) * rate) as usize;
        for (k, s) in heard[bang..bang + 4800].iter_mut().enumerate() {
            *s += rnd() * 0.8 * (-(k as f32) / 800.0).exp();
        }
        assert!((lateness(&heard, rate).unwrap() - truth).abs() <= 0.0005);
    }

    #[test]
    fn it_says_so_when_it_hears_no_clicks() {
        let rate = 48_000.0;
        let len = clicks(48_000).samples.len() + 14_400;
        let mut rnd = noise(3);
        let hiss: Vec<f32> = (0..len).map(|_| rnd() * 0.01).collect();
        for x in [vec![0.0; len], hiss, Vec::new()] {
            assert!(lateness(&x, rate).unwrap_err().contains("did not hear its clicks"));
        }
        // clicks lost in the noise
        let (got, _) = measure(&Room { gain: 0.003, hiss: 0.01, ..Default::default() }, 48_000);
        assert!(got.unwrap_err().contains("did not hear"));
        // or later than claudius looks for them
        let (got, _) = measure(&Room { late: 0.9, ..Default::default() }, 48_000);
        assert!(got.is_err());
    }

    #[test]
    fn it_says_so_when_the_clicks_come_back_at_different_times() {
        let rate = 48_000.0;
        let played = clicks(48_000).samples;
        // each click comes back up to 30 ms later than the one before it, in a room's faint hiss
        let mut rnd = noise(5);
        let mut heard: Vec<f32> = (0..played.len() + 48_000).map(|_| rnd() * 1e-4).collect();
        for (i, t) in CLICKS.iter().enumerate() {
            let src = (t * rate).round() as usize;
            let dst = src + (i as f64 * 0.007 * rate) as usize + 2000;
            for k in 0..200 {
                heard[dst + k] += played[src + k] * 0.3;
            }
        }
        let e = lateness(&heard, rate).unwrap_err();
        assert!(e.contains("different times") || e.contains("did not hear"), "{e}");
    }

    #[test]
    fn an_input_gated_to_silence_is_named_as_such() {
        // what a browser on a Mac gave: the clicks heard now and then, and silence to the bit in between
        let rate = 48_000.0;
        let (mut heard, _) = Room::default().hear(&clicks(48_000).samples, rate);
        for t in &CLICKS[2..] {
            let at = ((t + 0.044) * rate) as usize;
            heard[at - 4_800..at + 9_600].fill(0.0);
        }
        let e = lateness(&heard, rate).unwrap_err();
        assert!(e.contains("went silent in places") && e.contains("Mic Mode") && e.ends_with("measure again"), "{e}");
    }

    #[test]
    fn a_recording_cut_short_counts_the_clicks_it_holds() {
        let rate = 48_000.0;
        let (heard, truth) = Room::default().hear(&clicks(48_000).samples, rate);
        // the last two clicks never made it
        let short = &heard[..((CLICKS[5] + 0.03) * rate) as usize];
        assert!((lateness(short, rate).unwrap() - truth).abs() <= 0.0005);
        let shorter = &heard[..((CLICKS[3] + 0.03) * rate) as usize];
        assert!(lateness(shorter, rate).is_err(), "four clicks are not enough to be sure");
    }
}

//! What a take is, on either side of the studio: what to record and what plays meanwhile (`Plan`), what the output
//! plays (`fill`), what the input captured put together and cut where the take starts (`Captured`, `cut`), and the
//! take itself (`Take`, as a float WAV, or the notes a MIDI input played). The app records through cpal and midir
//! (`record.rs`), the browser through Web Audio (`web_record.rs`), and both go through these.

use std::io::Cursor;
use std::sync::Arc;

use super::Pcm;

/// what to record, and what plays meanwhile
pub struct Plan {
    /// the input, by name (the default input when there is none of that name)
    pub input: Option<String>,
    /// a MIDI input to record what is played on, instead of an audio input
    pub midi: Option<String>,
    /// the first of the input's channels to record, and whether the one after it too (a stereo take)
    pub channel: usize,
    pub stereo: bool,
    /// the song as rendered
    pub song: Arc<Pcm>,
    /// where the take starts in the song, and the count-in before it, in seconds
    pub start: f64,
    pub count_in: f64,
    /// seconds per beat and beats per bar, for the click
    pub beat: f64,
    pub bar: u32,
    /// the click goes on while the take records, not only through the count-in
    pub click: bool,
    /// seconds the setup adds that the devices do not report
    pub offset: f64,
}

impl Plan {
    /// a measurement of the latency (`Recorder::lateness`): the clicks of `latency::clicks` and nothing else, with no
    /// count-in and no offset, so that they come back as late as a take would land; heard on `channel` of `input`
    pub fn measure(input: Option<String>, channel: usize) -> Plan {
        Plan {
            input,
            midi: None,
            channel,
            stereo: false,
            song: Arc::new(super::latency::clicks(48_000)),
            start: 0.0,
            count_in: 0.0,
            beat: 0.5,
            bar: 4,
            click: false,
            offset: 0.0,
        }
    }
}

/// a recorded take
pub enum Take {
    /// from an audio input
    Audio {
        /// a 32-bit float WAV file at the input's rate
        wav: Vec<u8>,
        secs: f64,
        /// the loudest sample, 0..1
        peak: f32,
        /// how often a stream dropped audio while the take recorded (each a click or a gap in it)
        dropouts: usize,
        /// how often the input fell silent in it (`hushed`)
        hushed: usize,
    },
    /// from a MIDI input: the notes played (in the app: the browser records no MIDI yet)
    #[cfg(not(target_arch = "wasm32"))]
    Notes(Vec<Played>),
}

/// a note played on a MIDI input: when it started and ended, in seconds from the take's start, its pitch, how hard
#[cfg(any(not(target_arch = "wasm32"), test))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Played {
    pub start: f64,
    pub end: f64,
    pub pitch: u8,
    pub velocity: f32,
}

/// The notes in MIDI bytes, as a take hears them: each note on (its pitch and velocity) or off (velocity 0). The bytes
/// may hold several messages, the later ones in running status, with clock and other real-time bytes among them.
#[cfg(any(not(target_arch = "wasm32"), test))]
pub(crate) fn notes_of(bytes: &[u8]) -> Vec<(u8, f32)> {
    let mut out = Vec::new();
    let (mut status, mut data) = (0u8, Vec::with_capacity(2));
    for &b in bytes {
        match b {
            // real time (clock, start, stop, active sensing) may come between any two bytes
            0xF8..=0xFF => {}
            0x80..=0xF7 => (status, data) = (b, Vec::with_capacity(2)),
            _ => {
                data.push(b);
                let wanted = match status & 0xF0 {
                    0xC0 | 0xD0 => 1,
                    0xF0 => usize::MAX,
                    _ => 2,
                };
                if data.len() == wanted {
                    match status & 0xF0 {
                        0x90 => out.push((data[0], f32::from(data[1]) / 127.0)),
                        0x80 => out.push((data[0], 0.0)),
                        _ => {}
                    }
                    data.clear();
                }
            }
        }
    }
    out
}

/// The notes in what a MIDI input played (seconds since the recorder began, pitch, velocity), measured from `start`:
/// a note on a pitch already sounding ends the one before, and a note still held at `stop` ends there.
#[cfg(any(not(target_arch = "wasm32"), test))]
pub(crate) fn pair(events: &[(f64, u8, f32)], start: f64, stop: f64) -> Vec<Played> {
    let mut open: [Option<(f64, f32)>; 128] = [None; 128];
    let mut out = Vec::new();
    let end = |pitch: u8, (at, velocity): (f64, f32), until: f64, out: &mut Vec<Played>| {
        if until > start {
            out.push(Played { start: (at - start).max(0.0), end: until - start, pitch, velocity });
        }
    };
    for &(t, p, v) in events {
        if let Some(on) = open[p as usize].take() {
            end(p, on, t, &mut out);
        }
        if v > 0.0 {
            open[p as usize] = Some((t, v));
        }
    }
    for (p, on) in open.iter().enumerate() {
        if let Some(on) = on {
            end(p as u8, *on, stop, &mut out);
        }
    }
    out.sort_by(|a, b| a.start.total_cmp(&b.start).then(a.pitch.cmp(&b.pitch)));
    out
}

/// what the output plays: the song from `from` seconds on, and a click until `click_until`
#[derive(Clone)]
pub(crate) struct Mix {
    pub(crate) song: Arc<Pcm>,
    pub(crate) from: f64,
    pub(crate) start: f64,
    pub(crate) click_until: f64,
    pub(crate) beat: f64,
    pub(crate) bar: u32,
}

impl Mix {
    /// what the output plays for `plan`: the song from its count-in on, with a click through the count-in (and through
    /// the take too, when asked)
    pub(crate) fn of(plan: &Plan) -> Mix {
        Mix {
            song: plan.song.clone(),
            from: plan.start - plan.count_in,
            start: plan.start,
            click_until: if plan.click { f64::INFINITY } else { plan.start },
            beat: plan.beat,
            bar: plan.bar.max(1),
        }
    }
}

/// The output's frames from `first` on: the song from `mix.from` seconds (silence before its start and after its end),
/// and a click on every beat (counted from the take's start, the bar's first louder) until `mix.click_until`.
pub(crate) fn fill(out: &mut [f32], channels: usize, rate: f64, first: u64, mix: &Mix) {
    let ch = channels.max(1);
    let song_ch = mix.song.channels.max(1) as usize;
    let song_frames = mix.song.frames();
    for (i, frame) in out.chunks_exact_mut(ch).enumerate() {
        let t = mix.from + (first + i as u64) as f64 / rate;
        let (mut l, mut r) = (0f32, 0f32);
        let pos = t * mix.song.rate as f64;
        if pos >= 0.0 && (pos as usize) + 1 < song_frames {
            let (k, x) = (pos as usize, (pos - pos.floor()) as f32);
            let at = |frame: usize, c: usize| mix.song.samples[frame * song_ch + c.min(song_ch - 1)];
            l = at(k, 0) + (at(k + 1, 0) - at(k, 0)) * x;
            r = at(k, 1) + (at(k + 1, 1) - at(k, 1)) * x;
        }
        if t < mix.click_until && mix.beat > 0.0 {
            let beat = ((t - mix.start) / mix.beat).floor();
            let since = t - (mix.start + beat * mix.beat);
            if since < 0.03 {
                let accent = (beat as i64).rem_euclid(mix.bar as i64) == 0;
                let hz = if accent { 1760.0 } else { 1320.0 };
                let c = ((std::f64::consts::TAU * hz * since).sin() * (-since * 150.0).exp() * 0.45) as f32;
                l += c;
                r += c;
            }
        }
        frame[0] = l;
        if ch > 1 {
            frame[1] = r;
        }
        for s in frame.iter_mut().skip(2) {
            *s = 0.0;
        }
    }
}

/// the take: the captured samples from `at` seconds after the first was captured (silence first, when that was before
/// it), to the end
pub(crate) fn cut(taken: &[f32], channels: usize, rate: f64, at: f64) -> Vec<f32> {
    let skip = (at * rate).round() as i64 * channels as i64;
    if skip >= 0 {
        taken.get(skip as usize..).map(<[f32]>::to_vec).unwrap_or_default()
    } else {
        let mut out = vec![0.0; (-skip) as usize];
        out.extend_from_slice(taken);
        out
    }
}

/// How often a recording falls silent, to the bit, for 50 ms or more between sounds (interleaved, every channel at 0).
/// A microphone or a line in always hears some noise, so that is the input muted or gated: by the processing a system
/// or a browser does to a microphone for calls, say.
#[cfg(any(not(target_arch = "wasm32"), test))]
pub(crate) fn hushed(samples: &[f32], channels: usize, rate: f64) -> usize {
    hushed_but(samples, channels, rate, |_| false)
}

/// `hushed`, but frame `k` (from the first) is not heard at all when `missing(k)`, as frames a browser lost are not:
/// silence that runs into them is not counted
pub(crate) fn hushed_but(samples: &[f32], channels: usize, rate: f64, missing: impl Fn(usize) -> bool) -> usize {
    let ch = channels.max(1);
    let long = (0.05 * rate).ceil() as usize;
    let (mut runs, mut silent, mut heard) = (0, 0usize, false);
    for (k, frame) in samples.chunks_exact(ch).enumerate() {
        if missing(k) {
            (heard, silent) = (false, 0);
        } else if frame.iter().all(|s| *s == 0.0) {
            silent += 1;
        } else {
            if heard && silent >= long {
                runs += 1;
            }
            (heard, silent) = (true, 0);
        }
    }
    runs
}

/// what to say of a recording that fell silent in places (`hushed`)
pub(crate) const HUSHED: &str = "the input went silent in places, as it does when the system or the browser processes \
                                 the microphone for calls: on a Mac, choose Standard in Control Center's Mic Mode";

/// interleaved samples as a 32-bit float WAV file
pub(crate) fn wav(samples: &[f32], channels: u16, rate: u32) -> Vec<u8> {
    let spec =
        hound::WavSpec { channels, sample_rate: rate, bits_per_sample: 32, sample_format: hound::SampleFormat::Float };
    let mut out = Cursor::new(Vec::new());
    if let Ok(mut w) = hound::WavWriter::new(&mut out, spec) {
        for &s in samples {
            let _ = w.write_sample(s);
        }
        let _ = w.finalize();
    }
    out.into_inner()
}

/// What an input captured, from chunks that each say which frame their first sample is on the clock that captured it:
/// in order from the first chunk's frame on, with silence where frames went missing between chunks.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Debug, Default)]
pub(crate) struct Captured {
    channels: usize,
    /// the frame of the first sample held
    first: Option<u64>,
    /// interleaved, from `first` on
    samples: Vec<f32>,
    /// where frames went missing, each filled with silence: frames since `first`, and how many
    gaps: Vec<(u64, u64)>,
}

#[cfg(any(target_arch = "wasm32", test))]
impl Captured {
    pub(crate) fn new(channels: usize) -> Captured {
        Captured { channels: channels.max(1), ..Default::default() }
    }

    /// takes in a chunk of interleaved `samples` whose first frame is `frame`; a chunk from before what is held, or the
    /// part of one already held, is dropped
    pub(crate) fn add(&mut self, frame: u64, samples: &[f32]) {
        let ch = self.channels;
        let first = *self.first.get_or_insert(frame);
        let held = self.frames() as u64;
        let Some(at) = frame.checked_sub(first) else { return };
        if at > held {
            self.gaps.push((held, at - held));
            self.samples.resize(at as usize * ch, 0.0);
        }
        let whole = samples.len() - samples.len() % ch;
        let skip = (held.saturating_sub(at) as usize * ch).min(whole);
        self.samples.extend_from_slice(&samples[skip..whole]);
    }

    /// the frames held
    pub(crate) fn frames(&self) -> usize {
        self.samples.len() / self.channels
    }

    /// the frame of the first sample held, if any
    pub(crate) fn first(&self) -> Option<u64> {
        self.first
    }

    /// how many times frames went missing at or after `from` frames in
    pub(crate) fn gaps_from(&self, from: u64) -> usize {
        self.gaps.iter().filter(|(at, _)| *at >= from).count()
    }

    /// how often what was heard from `from` frames in fell silent (`hushed`), not counting the frames that went missing
    pub(crate) fn hushed_from(&self, from: u64, rate: f64) -> usize {
        let start = (from as usize * self.channels).min(self.samples.len());
        // the frames are asked after in order, and the gaps went in in order: one pass over both
        let next = std::cell::Cell::new(0);
        let missing = |k: usize| {
            let f = from + k as u64;
            let mut i = next.get();
            while self.gaps.get(i).is_some_and(|(at, n)| at + n <= f) {
                i += 1;
            }
            next.set(i);
            self.gaps.get(i).is_some_and(|(at, _)| f >= *at)
        };
        hushed_but(&self.samples[start..], self.channels, rate, missing)
    }

    pub(crate) fn samples(&self) -> &[f32] {
        &self.samples
    }
}

/// Where a take starts in what the input captured, in seconds from the first frame captured, when the output and the
/// input share one clock (a browser's audio context). The song's first frame was to play at `scheduled` and the take
/// starts `lead` into it (the count-in); what plays is heard `out` later, and what is heard reaches the input `input`
/// later; `offset` adds what neither reports; the first frame captured reached the input at `first`. All in seconds on
/// that clock.
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn take_starts(scheduled: f64, lead: f64, out: f64, input: f64, offset: f64, first: f64) -> f64 {
    scheduled + lead + out + input + offset - first
}

/// What the output plays for `plan` at `rate`, from the count-in to half a second past the song's end (or the take's
/// start, if that is later), as one buffer of a stereo pair (a browser's `AudioBuffer`): the left channel, then the
/// right.
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn render_mix(plan: &Plan, rate: f64) -> [Vec<f32>; 2] {
    let mix = Mix::of(plan);
    let frames = ((plan.song.seconds().max(plan.start) - mix.from).max(0.0) + 0.5) * rate;
    let mut both = vec![0.0; frames as usize * 2];
    fill(&mut both, 2, rate, 0, &mix);
    [both.iter().step_by(2).copied().collect(), both.iter().skip(1).step_by(2).copied().collect()]
}

/// the channels a take records, `first` and `count` of them, out of what an input gave one plane a channel: as
/// interleaved frames, as long as the shortest plane; or how many channels there were, when too few
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn interleave(planes: &[Vec<f32>], first: usize, count: usize) -> Result<Vec<f32>, usize> {
    let picked = planes.get(first..first + count.max(1)).ok_or(planes.len())?;
    let frames = picked.iter().map(Vec::len).min().unwrap_or(0);
    Ok((0..frames).flat_map(|i| picked.iter().map(move |p| p[i])).collect())
}

/// where in the song the person hears it at `now` on the output's clock: from `from` (the count-in's start) on, once
/// what was to play at `scheduled` has come out of the output, `out` later
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn heard_at(now: f64, scheduled: Option<f64>, out: f64, from: f64) -> f64 {
    scheduled.map_or(from, |at| from + (now - at - out).max(0.0))
}

/// what to say when a browser gives no input: its error's name and message, and the input's name if it was chosen
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn no_input(name: &str, message: &str, input: Option<&str>) -> String {
    let which = input.map_or_else(|| "the microphone".to_string(), |i| format!("'{i}'"));
    match name {
        "NotAllowedError" | "SecurityError" => format!(
            "the browser was not allowed to use {which}: allow the microphone for this page (in the address bar's site \
             settings) and try again"
        ),
        "NotFoundError" => "the browser finds no microphone or audio input".into(),
        "NotReadableError" | "AbortError" => {
            format!("{which} is busy: another app or tab may be using it, or the system has not allowed the browser it")
        }
        "OverconstrainedError" => format!("{which} is not there any more: choose another input"),
        "NotSupportedError" => "this browser cannot record here: open claudius on localhost or over https, in a \
                                browser that records (Chrome, Edge, Firefox or Safari)"
            .into(),
        "" => format!("the browser gave no input: {message}"),
        other => format!("the browser gave no input ({other}): {message}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_input_that_falls_silent_to_the_bit_is_told_from_a_quiet_one() {
        let rate = 1_000.0;
        let hiss = |n: usize| (0..n).map(|k| if k % 2 == 0 { 1e-6 } else { -1e-6 }).collect::<Vec<f32>>();
        let silence = |n: usize| vec![0.0f32; n];
        // quiet is not silent; silence before the first sound and after the last is the take's edges
        assert_eq!(hushed(&hiss(1_000), 1, rate), 0);
        assert_eq!(hushed(&[silence(400), hiss(100), silence(400)].concat(), 1, rate), 0);
        // 50 ms of silence between sounds counts, 49 ms does not, and each run counts once
        assert_eq!(hushed(&[hiss(10), silence(50), hiss(10)].concat(), 1, rate), 1);
        assert_eq!(hushed(&[hiss(10), silence(49), hiss(10)].concat(), 1, rate), 0);
        assert_eq!(hushed(&[hiss(10), silence(80), hiss(10), silence(200), hiss(1)].concat(), 1, rate), 2);
        // in stereo a frame is silent when both sides are
        let one_side: Vec<f32> = [hiss(20), silence(200), hiss(20)].concat().iter().flat_map(|s| [*s, 1e-6]).collect();
        assert_eq!(hushed(&one_side, 2, rate), 0);
        let both: Vec<f32> = [hiss(20), silence(200), hiss(20)].concat().iter().flat_map(|s| [*s, *s]).collect();
        assert_eq!(hushed(&both, 2, rate), 1);
        assert_eq!(hushed(&[], 2, rate), 0);
    }

    #[test]
    fn a_browser_plays_the_song_from_the_count_in_with_its_click() {
        let n = 48_000 * 4;
        let song = Arc::new(Pcm { rate: 48_000, channels: 2, samples: (0..n).flat_map(|_| [0.25, -0.25]).collect() });
        let plan = Plan {
            input: None,
            midi: None,
            channel: 0,
            stereo: false,
            song,
            start: 2.0,
            count_in: 1.0,
            beat: 0.5,
            bar: 4,
            click: false,
            offset: 0.0,
        };
        // at 44.1 kHz, as a browser may run: from 1 s into the song to half a second past its end
        let [l, r] = render_mix(&plan, 44_100.0);
        assert_eq!((l.len(), r.len()), (44_100 * 3 + 22_050, 44_100 * 3 + 22_050));
        let loud =
            |p: &[f32], t: f64| p[(t * 44_100.0) as usize..][..400].iter().fold(0f32, |m, s| m.max((s - 0.25).abs()));
        assert!(loud(&l, 0.0) > 0.2 && loud(&l, 0.5) > 0.2, "the count-in's two beats click");
        assert!(loud(&l, 1.0) < 1e-3 && loud(&l, 0.25) < 1e-3, "and nothing else does");
        assert!((r[44_100 / 4] + 0.25).abs() < 1e-6, "the right channel is the song's right");
        assert_eq!(l[l.len() - 1], 0.0, "silence after the song's end");
        let none = Plan { song: Arc::new(Pcm { rate: 48_000, channels: 2, samples: Vec::new() }), ..plan };
        assert_eq!(
            render_mix(&none, 48_000.0)[0].len(),
            72_000,
            "a song with nothing in it is the count-in, and the half second"
        );
    }

    #[test]
    fn the_channels_a_take_records_come_out_of_the_planes_interleaved() {
        let planes = vec![vec![1.0, 2.0, 3.0], vec![-1.0, -2.0, -3.0], vec![10.0, 20.0]];
        assert_eq!(interleave(&planes, 0, 1), Ok(vec![1.0, 2.0, 3.0]));
        assert_eq!(interleave(&planes, 1, 1), Ok(vec![-1.0, -2.0, -3.0]), "input 2 alone");
        assert_eq!(interleave(&planes, 0, 2), Ok(vec![1.0, -1.0, 2.0, -2.0, 3.0, -3.0]));
        assert_eq!(interleave(&planes, 1, 2), Ok(vec![-1.0, 10.0, -2.0, 20.0]), "as long as the shorter");
        assert_eq!(interleave(&planes, 2, 2), Err(3), "a pair past the last channel");
        assert_eq!(interleave(&[vec![0.5]], 1, 1), Err(1), "a mono input has no second channel");
        assert_eq!(interleave(&[], 0, 1), Err(0));
    }

    #[test]
    fn the_person_hears_the_song_once_it_is_out_of_the_output() {
        assert_eq!(heard_at(5.0, None, 0.02, 10.0), 10.0, "nothing has played yet: the count-in's start");
        assert_eq!(heard_at(1.0, Some(1.0), 0.02, 10.0), 10.0, "scheduled, but not out yet");
        assert!((heard_at(3.52, Some(1.0), 0.02, 10.0) - 12.5).abs() < 1e-12);
    }

    #[test]
    fn a_browser_that_gives_no_input_is_told_what_to_do() {
        let denied = no_input("NotAllowedError", "Permission denied", None);
        assert!(denied.contains("not allowed to use the microphone") && denied.contains("address bar"), "{denied}");
        assert!(no_input("NotAllowedError", "", Some("Scarlett 2i2")).contains("not allowed to use 'Scarlett 2i2'"));
        assert!(no_input("NotFoundError", "", None).contains("no microphone"));
        assert!(no_input("NotReadableError", "", None).contains("busy"));
        assert!(no_input("OverconstrainedError", "", Some("USB Mic")).contains("'USB Mic' is not there any more"));
        assert!(no_input("NotSupportedError", "Not supported", None).contains("cannot record here"));
        assert_eq!(no_input("TypeError", "bad", None), "the browser gave no input (TypeError): bad");
        assert_eq!(no_input("", "no mediaDevices", None), "the browser gave no input: no mediaDevices");
    }

    #[test]
    fn chunks_go_together_in_order_with_silence_where_frames_went_missing() {
        let mut c = Captured::new(2);
        assert_eq!((c.frames(), c.first()), (0, None));
        c.add(1000, &[1.0, -1.0, 2.0, -2.0]);
        c.add(1002, &[3.0, -3.0]);
        assert_eq!((c.frames(), c.first(), c.gaps_from(0)), (3, Some(1000), 0));
        // two frames went missing
        c.add(1005, &[6.0, -6.0]);
        assert_eq!(c.samples(), [1.0, -1.0, 2.0, -2.0, 3.0, -3.0, 0.0, 0.0, 0.0, 0.0, 6.0, -6.0]);
        assert_eq!((c.gaps_from(0), c.gaps_from(3), c.gaps_from(4)), (1, 1, 0), "the gap was at frame 3");
        // a chunk that repeats one frame held and brings one more; then one wholly held; then one from before
        c.add(1005, &[9.0, -9.0, 7.0, -7.0]);
        c.add(1001, &[9.0, 9.0]);
        c.add(990, &[9.0, 9.0]);
        assert_eq!(&c.samples()[10..], [6.0, -6.0, 7.0, -7.0]);
        assert_eq!((c.frames(), c.gaps_from(0)), (7, 1));
        // frames that went missing are not an input gone silent; silence the input gave is
        let rate = 100.0;
        let mut q = Captured::new(1);
        q.add(0, &[0.5; 10]);
        q.add(30, &[0.5; 10]);
        assert_eq!((q.gaps_from(0), q.hushed_from(0, rate)), (1, 0), "a gap of 20 frames, 0.2 s, filled");
        q.add(40, &[[0.0; 10], [0.5; 10]].concat());
        assert_eq!(q.hushed_from(0, rate), 1, "0.1 s of silence heard");
        q.add(60, &[0.0; 3]);
        q.add(70, &[vec![0.0; 4], vec![0.5; 2]].concat());
        assert_eq!(q.hushed_from(0, rate), 1, "silence that runs into a gap is not counted");
        assert_eq!(q.hushed_from(45, rate), 0, "nor what was before `from`");
        // an empty chunk changes nothing, and neither does half a frame
        c.add(1007, &[]);
        c.add(1007, &[8.0]);
        assert_eq!((c.frames(), c.samples().len()), (7, 14));
        c.add(1007, &[8.0, -8.0, 5.0]);
        assert_eq!(&c.samples()[14..], [8.0, -8.0], "the whole frames of a chunk that ends in half of one");
    }

    #[test]
    fn a_take_starts_when_its_downbeat_reached_the_input() {
        // scheduled 0.2 s in, a count-in of 2 s, heard 12.5 ms later, at the input 5 ms after that, 30 ms more
        // that nothing reports, and the input's first frame 0.1 s in
        let at = take_starts(0.2, 2.0, 0.0125, 0.005, 0.03, 0.1);
        assert!((at - 2.1475).abs() < 1e-12, "{at}");
        assert!((take_starts(1.0, 0.0, 0.0, 0.0, 0.0, 1.0)).abs() < 1e-12, "at once, when nothing is late");
        assert!(take_starts(0.0, 0.0, 0.0, 0.0, -0.05, 0.0) < 0.0, "before the first frame: silence first");
    }

    /// the browser's path, simulated: a song scheduled on the context's clock, heard `out` later, picked up by the
    /// microphone and in the worklet `input` (and `extra`, unreported) later, in 128-frame quanta from frame `begun`
    fn through_a_browser(
        song: &[f32],
        scheduled: u64,
        out: u64,
        input: u64,
        extra: u64,
        begun: u64,
        drop: Option<u64>,
    ) -> Captured {
        let mut c = Captured::new(1);
        let late = scheduled + out + input + extra;
        let end = late + song.len() as u64 + 48_000;
        for quantum in (begun..end).step_by(128) {
            if Some(quantum) == drop {
                continue;
            }
            let chunk: Vec<f32> = (quantum..quantum + 128)
                .map(|f| f.checked_sub(late).and_then(|k| song.get(k as usize)).copied().unwrap_or(0.0))
                .collect();
            c.add(quantum, &chunk);
        }
        c
    }

    #[test]
    fn a_take_recorded_in_a_browser_starts_on_its_downbeat() {
        let rate = 48_000.0;
        // a song whose every frame says which it is, so where a take starts shows to the frame
        let song: Vec<f32> = (0..rate as usize * 6).map(|k| k as f32 / 1e6).collect();
        let (scheduled, out, input, begun) = (9_600, 612, 256, 4_800);
        for extra in [0, 1_440] {
            // a quantum lost in the count-in must not move the take
            let c = through_a_browser(&song, scheduled, out, input, extra, begun, Some(begun + 128 * 40));
            let s = |f: u64| f as f64 / rate;
            let at = take_starts(s(scheduled), 2.0, s(out), s(input), s(extra), s(c.first().unwrap()));
            let take = cut(c.samples(), 1, rate, at);
            assert_eq!(take[0], song[96_000], "the downbeat, two seconds in, comes first (unreported {extra} frames)");
            assert_eq!(take[1_000], song[97_000]);
            assert_eq!(c.gaps_from(((at * rate).round()) as u64), 0, "the gap was before the take");
        }
    }

    #[test]
    fn a_measurement_in_a_browser_finds_what_the_browser_does_not_report() {
        let rate = 48_000.0;
        let clicks = super::super::latency::clicks(48_000).samples;
        let (scheduled, out, input) = (9_600, 612, 256);
        // a Bluetooth headset's 0.18 s, which nothing reports
        let extra = 8_640;
        let c = through_a_browser(&clicks, scheduled, out, input, extra, 4_800, None);
        let s = |f: u64| f as f64 / rate;
        let at = take_starts(s(scheduled), 0.0, s(out), s(input), 0.0, s(c.first().unwrap()));
        let heard = cut(c.samples(), 1, rate, at);
        let late = super::super::latency::lateness(&heard, rate).unwrap();
        assert!((late - 0.18).abs() < 0.0005, "{late}");
    }

    fn song(secs: f64) -> Arc<Pcm> {
        let n = (secs * 48_000.0) as usize;
        Arc::new(Pcm { rate: 48_000, channels: 2, samples: (0..n).flat_map(|i| [i as f32 / n as f32, -0.5]).collect() })
    }

    #[test]
    fn the_output_plays_the_song_from_the_count_in_and_clicks_on_its_beats() {
        let mix = Mix { song: song(10.0), from: 2.0, start: 4.0, click_until: 4.0, beat: 0.5, bar: 4 };
        let rate = 48_000.0;
        let mut out = vec![0.0; 3 * 48_000 * 3];
        fill(&mut out, 3, rate, 0, &mix);
        let at = |t: f64, c: usize| out[(t * rate) as usize * 3 + c];
        // between clicks: the song as it is at that point, on the first two channels only
        assert!((at(0.25, 0) - 2.25 / 10.0).abs() < 1e-3 && (at(0.25, 1) + 0.5).abs() < 1e-6);
        assert_eq!(at(0.25, 2), 0.0);
        // a click on each beat of the count-in, from the take's start back (2.0 s in is the count-in's first beat)
        let loud = |t: f64| {
            (0..240).map(|k| (at(t + k as f64 / rate, 0) - (t + 2.0) as f32 / 10.0).abs()).fold(0f32, f32::max)
        };
        for beat in [0.0, 0.5, 1.0, 1.5] {
            assert!(loud(beat) > 0.2, "a click at {beat} s");
        }
        assert!(loud(0.2) < 1e-3, "nothing between them");
        assert!(loud(2.0) < 1e-3 && loud(2.5) < 1e-3, "and none once the take starts");
        let mut before = vec![1.0; 4];
        fill(&mut before, 2, rate, 0, &Mix { from: -1.0, click_until: f64::NEG_INFINITY, ..mix });
        assert_eq!(before, [0.0; 4], "silence before the song");
    }

    #[test]
    fn a_take_is_cut_where_the_section_began_as_heard() {
        let taken: Vec<f32> = (0..100).map(|i| i as f32).collect();
        let rate = 10.0;
        // heard 0.3 s after the input began, a count-in of 2 s and 0.1 s more for the headphones: 2.4 s in
        let at = 0.3 + 2.0 + 0.1;
        assert_eq!(cut(&taken, 1, rate, at)[..3], [24.0, 25.0, 26.0]);
        assert_eq!(cut(&taken, 2, rate, at)[..2], [48.0, 49.0], "frames, not samples");
        assert_eq!(cut(&taken, 1, rate, -0.2)[..3], [0.0, 0.0, 0.0], "silence first when it began too late");
        assert!(cut(&taken, 1, rate, 20.0).is_empty());
    }

    #[test]
    fn a_keyboard_says_which_notes_start_and_end() {
        assert_eq!(notes_of(&[0x90, 60, 127]), [(60, 1.0)]);
        assert_eq!(notes_of(&[0x93, 64, 0]), [(64, 0.0)], "a note on at velocity 0 is a note off, on any channel");
        assert_eq!(notes_of(&[0x80, 60, 40]), [(60, 0.0)]);
        assert!(notes_of(&[0xB0, 64, 127]).is_empty(), "the sustain pedal plays no note");
        assert!(notes_of(&[0xF8]).is_empty(), "nor does the clock");
        // a chord in one packet, the later notes in running status, and the clock in the middle of one
        let chord = notes_of(&[0x90, 60, 127, 64, 127, 0xF8, 67, 127, 0x80, 60, 0]);
        assert_eq!(chord.iter().map(|n| n.0).collect::<Vec<_>>(), [60, 64, 67, 60]);
        assert!(notes_of(&[0xC0, 5, 0x90, 62, 64]).len() == 1, "a program change takes one byte");
        assert!(notes_of(&[0xF0, 1, 2, 3, 0xF7, 0x90, 62, 64]).len() == 1, "and a sysex its whole length");
    }

    #[test]
    fn what_was_played_is_notes_from_the_take_start() {
        // the take starts 2 s in: a note held across it starts with it; one held at the stop ends there
        let events = [(1.0, 60, 0.5), (1.5, 62, 0.5), (1.8, 62, 0.0), (2.5, 60, 0.0), (3.0, 64, 0.9), (3.5, 67, 1.0)];
        let notes = pair(&events, 2.0, 4.0);
        let got: Vec<(f64, f64, u8)> = notes.iter().map(|n| (n.start, n.end, n.pitch)).collect();
        assert_eq!(got, [(0.0, 0.5, 60), (1.0, 2.0, 64), (1.5, 2.0, 67)], "the note over before the take is gone");
        let again = pair(&[(0.0, 60, 0.7), (1.0, 60, 0.8), (2.0, 60, 0.0)], 0.0, 3.0);
        assert_eq!(again.iter().map(|n| (n.start, n.end)).collect::<Vec<_>>(), [(0.0, 1.0), (1.0, 2.0)]);
    }

    #[test]
    fn a_take_is_a_float_wav() {
        let wav = wav(&[0.25, -0.5, 1.5, 0.0], 2, 44_100);
        let pcm = Pcm::decode_wav(&wav).unwrap();
        assert_eq!((pcm.rate, pcm.channels, pcm.samples), (44_100, 2, vec![0.25, -0.5, 1.5, 0.0]));
    }
}

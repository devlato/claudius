//! How a window of the live mix (design 0023) goes into the page's buffer over the render it changes (the page's player,
//! `web.rs`): faded in and out against what the buffer holds there, as the desktop player plays one, but joined to the
//! window before it when it carries on from it, so that a run of windows of one change plays through as one, with no fade
//! between them, as the desktop player joins them. Apart from the player, so that it is tested where the page's player is
//! not built.

/// audio held channel by channel, which a window is read from and written into: the page's buffer, or plain samples
pub(super) trait Channels {
    fn channels(&self) -> usize;
    fn frames(&self) -> usize;
    /// channel `ch` from frame `at` into `into`; false when it could not be read
    fn read(&self, ch: usize, at: usize, into: &mut [f32]) -> bool;
    /// `from` into channel `ch` from frame `at`; false when it could not be written
    fn write(&mut self, ch: usize, at: usize, from: &[f32]) -> bool;
}

/// where the last window written ends, and what it was: of which version of the song, the frame after its last, and
/// channel by channel its last frames as they came, before they faded out
pub(super) struct Seam {
    version: u64,
    end: usize,
    tail: Vec<Vec<f32>>,
}

/// `window` (interleaved, `width` channels, at the rate of `into`) of the song at `version` written into `into` from
/// frame `first`: faded in over `fade` frames against what `into` holds and out over `fade` frames at its end. A window
/// that carries on from the last one written (`seam`: of the same version, from where it ended, to a frame) starts where
/// that one ended, with that one's end put back as it came and no fade between the two. `seam` is left at the window's
/// end for the next. Whether anything was written.
pub(super) fn put(
    into: &mut impl Channels,
    seam: &mut Option<Seam>,
    version: u64,
    first: usize,
    window: &[f32],
    width: usize,
    fade: usize,
) -> bool {
    let width = width.max(1);
    let joined = seam.take().filter(|s| s.version == version && s.end.abs_diff(first) <= 1);
    let first = joined.as_ref().map_or(first, |s| s.end);
    let frames = (window.len() / width).min(into.frames().saturating_sub(first));
    if frames == 0 {
        *seam = joined;
        return false;
    }
    let fade = fade.clamp(1, frames / 2 + 1);
    let mut tail = Vec::with_capacity(into.channels());
    for ch in 0..into.channels() {
        let came: Vec<f32> = (0..frames).map(|i| window[i * width + ch.min(width - 1)]).collect();
        // the window before, which faded out into this one's start, put back as it came
        if let Some(t) = joined.as_ref().and_then(|s| s.tail.get(ch))
            && !into.write(ch, first - t.len(), t)
        {
            return false;
        }
        let mut held = vec![0.0f32; frames];
        if !into.read(ch, first, &mut held) {
            return false;
        }
        for (i, h) in held.iter_mut().enumerate() {
            let from_start = if joined.is_some() { fade } else { i + 1 };
            let k = (from_start.min(frames - i) as f32 / fade as f32).min(1.0);
            *h += (came[i] - *h) * k;
        }
        if !into.write(ch, first, &held) {
            return false;
        }
        // what fades out at its end, as it came, for a window that carries on from it; never what faded in
        let kept = frames.saturating_sub(fade).max(if joined.is_some() { 0 } else { fade.min(frames) });
        tail.push(came[kept..].to_vec());
    }
    *seam = Some(Seam { version, end: first + frames, tail });
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// plain samples, channel by channel
    struct Plain(Vec<Vec<f32>>);

    impl Channels for Plain {
        fn channels(&self) -> usize {
            self.0.len()
        }
        fn frames(&self) -> usize {
            self.0[0].len()
        }
        fn read(&self, ch: usize, at: usize, into: &mut [f32]) -> bool {
            into.copy_from_slice(&self.0[ch][at..at + into.len()]);
            true
        }
        fn write(&mut self, ch: usize, at: usize, from: &[f32]) -> bool {
            self.0[ch][at..at + from.len()].copy_from_slice(from);
            true
        }
    }

    /// a render of silence, two channels of `frames`
    fn render(frames: usize) -> Plain {
        Plain(vec![vec![0.0; frames]; 2])
    }

    /// a window of `frames` stereo frames, every sample 1
    fn window(frames: usize) -> Vec<f32> {
        vec![1.0; frames * 2]
    }

    #[test]
    fn a_window_fades_in_and_out_against_the_render_it_goes_over() {
        let mut b = render(1000);
        let mut seam = None;
        assert!(put(&mut b, &mut seam, 1, 100, &window(300), 2, 10));
        for ch in &b.0 {
            assert!(ch[..100].iter().all(|&x| x == 0.0));
            assert!(ch[100] > 0.0 && ch[100] < 0.2, "fades in: {}", ch[100]);
            assert!(ch[110..390].iter().all(|&x| x == 1.0));
            assert!(ch[399] > 0.0 && ch[399] < 0.2, "fades out: {}", ch[399]);
            assert!(ch[400..].iter().all(|&x| x == 0.0));
        }
    }

    #[test]
    fn a_run_of_windows_of_one_change_plays_through_with_no_dip_at_their_seams() {
        let mut b = render(1000);
        let mut seam = None;
        for k in 0..3 {
            assert!(put(&mut b, &mut seam, 7, 100 + k * 200, &window(200), 2, 10));
        }
        // one run from 100 to 700, faded in at its start and out at its end, whole between
        for ch in &b.0 {
            assert!(ch[110..690].iter().all(|&x| x == 1.0), "a dip at a seam: {:?}", &ch[295..305]);
            assert!(ch[100] < 0.2 && ch[699] < 0.2);
            assert!(ch[700..].iter().all(|&x| x == 0.0));
        }
    }

    #[test]
    fn a_window_a_frame_off_where_the_last_ended_still_joins_it() {
        for off in [-1i64, 1] {
            let mut b = render(1000);
            let mut seam = None;
            put(&mut b, &mut seam, 3, 100, &window(200), 2, 10);
            put(&mut b, &mut seam, 3, (300 + off) as usize, &window(200), 2, 10);
            for ch in &b.0 {
                assert!(ch[110..490].iter().all(|&x| x == 1.0), "{off}: {:?}", &ch[295..305]);
            }
        }
    }

    #[test]
    fn a_window_of_another_change_or_from_elsewhere_fades_in_against_what_is_there() {
        // of another version, from where the last ended: the mix changes there, and the window fades in over it
        let mut b = render(1000);
        let mut seam = None;
        put(&mut b, &mut seam, 1, 100, &window(200), 2, 10);
        put(&mut b, &mut seam, 2, 300, &window(200), 2, 10);
        assert!(b.0[0][300] < 0.2);
        // of the same version, but not from where the last ended (the song was sought): faded in where it starts
        let mut b = render(1000);
        let mut seam = None;
        put(&mut b, &mut seam, 1, 100, &window(200), 2, 10);
        put(&mut b, &mut seam, 1, 600, &window(200), 2, 10);
        assert!(b.0[0][600] < 0.2 && b.0[0][299] < 0.2);
        assert!(b.0[0][300..600].iter().all(|&x| x == 0.0));
    }

    #[test]
    fn a_window_past_the_render_writes_nothing_and_keeps_the_seam() {
        let mut b = render(1000);
        let mut seam = None;
        put(&mut b, &mut seam, 1, 800, &window(200), 2, 10);
        assert!(!put(&mut b, &mut seam, 1, 1000, &window(200), 2, 10));
        assert_eq!(seam.as_ref().map(|s| s.end), Some(1000));
        // a window running past the render's end is cut there, faded out at the cut
        let mut b = render(1000);
        let mut seam = None;
        assert!(put(&mut b, &mut seam, 1, 900, &window(200), 2, 10));
        assert!(b.0[1][999] < 0.2 && b.0[1][950] == 1.0);
    }

    #[test]
    fn a_mono_window_goes_into_every_channel() {
        let mut b = render(100);
        let mut seam = None;
        put(&mut b, &mut seam, 1, 0, &[0.5; 50], 1, 5);
        assert_eq!(b.0[0][20], 0.5);
        assert_eq!(b.0[1][20], 0.5);
    }
}

//! The live mix's first part (design 0023): after an edit of the mix while the song plays, windows of the song as it is
//! now, from just after the playhead, played in place of the render until the next render comes, so that a fader or a
//! send is heard as it moves rather than when the whole song has rendered again.

/// how far after the playhead the first window starts, in seconds: about what one takes to come
pub const LEAD: f64 = 0.15;
/// how long each window is, in seconds
pub const WINDOW: f64 = 1.0;
/// how far ahead of the playhead windows are asked for, in seconds
pub const AHEAD: f64 = 4.0;

/// Which windows to ask for, one at a time.
#[derive(Debug, Default)]
pub struct Live {
    /// the version of the song the windows asked for are of, and where the next one starts, in seconds
    version: u64,
    next: f64,
    /// a window asked for that has not come
    waiting: bool,
    /// the version of the song the server or the demo's engine had no windows of (an edit of a track's own sound): none
    /// are asked for it
    refused: Option<u64>,
    /// windows of this version that failed to come, one after another (a timeout, a server error): asked for again,
    /// up to `TRIES` of them
    failures: u32,
}

/// how many windows of a version may fail to come, one after another, before none more are asked for it
pub const TRIES: u32 = 3;

impl Live {
    /// The window to ask for now, `from` and `secs`, if any: while the song plays (`playing`, its playhead at `at`)
    /// and is newer (`song`) than the render playing (`rendered`), one window after another from just after the
    /// playhead, up to `AHEAD` of it; again from the playhead when the song changes again.
    pub fn want(&mut self, song: u64, rendered: u64, at: f64, playing: bool) -> Option<(f64, f64)> {
        if !playing || song <= rendered || self.waiting || self.refused == Some(song) {
            return None;
        }
        if song != self.version {
            (self.version, self.next, self.failures) = (song, at + LEAD, 0);
        }
        // a window that came late, or a seek: on from where the playhead will be
        if self.next < at + LEAD || self.next > at + AHEAD + WINDOW {
            self.next = at + LEAD;
        }
        if self.next > at + AHEAD {
            return None;
        }
        self.waiting = true;
        Some((self.next, WINDOW))
    }

    /// A window came, of the song at `version`, from `start` seconds and `secs` long: the next goes on from its end
    /// when it is of the version asked for.
    pub fn came(&mut self, version: u64, start: f64, secs: f64) {
        self.waiting = false;
        if version == self.version {
            (self.next, self.failures) = (start + secs, 0);
        }
    }

    /// A window failed to come (a timeout, a server error, a body cut short): it is asked for again, unless `TRIES` of
    /// this version's have failed one after another, as they do where no window can come.
    pub fn failed(&mut self) {
        self.waiting = false;
        self.failures += 1;
        if self.failures >= TRIES {
            self.refused = Some(self.version);
        }
    }

    /// The server has no windows of the song as it is (the next render plays it): none are asked for until it
    /// changes again.
    pub fn refused(&mut self) {
        self.waiting = false;
        self.refused = Some(self.version);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_window_that_failed_to_come_is_asked_for_again_and_one_refused_is_not() {
        let mut live = Live::default();
        let first = live.want(5, 4, 10.0, true);
        assert!(first.is_some());
        // a timeout: the same window again, and so up to TRIES
        live.failed();
        assert_eq!(live.want(5, 4, 10.0, true), first);
        live.failed();
        assert_eq!(live.want(5, 4, 10.0, true), first);
        live.failed();
        assert_eq!(live.want(5, 4, 10.0, true), None, "no window of this version comes");
        // the next edit asks again; one that comes clears what failed before it
        assert!(live.want(6, 4, 10.0, true).is_some());
        live.failed();
        let (from, secs) = live.want(6, 4, 10.0, true).unwrap();
        live.came(6, from, secs);
        live.failed();
        live.failed();
        assert!(live.want(6, 4, 10.0, true).is_some(), "two failures since the last window came");
        // a refusal (409) holds until the song changes
        live.refused();
        assert_eq!(live.want(6, 4, 10.0, true), None);
        assert!(live.want(7, 4, 10.0, true).is_some());
    }

    #[test]
    fn windows_are_asked_for_one_after_another_ahead_of_the_playhead_while_the_song_is_newer_than_its_render() {
        let mut live = Live::default();
        // nothing while paused, or when the render is of the song as it is
        assert_eq!(live.want(5, 4, 10.0, false), None);
        assert_eq!(live.want(5, 5, 10.0, true), None);
        // an edit while it plays: from just after the playhead, one window at a time
        assert_eq!(live.want(5, 4, 10.0, true), Some((10.0 + LEAD, WINDOW)));
        assert_eq!(live.want(5, 4, 10.0, true), None, "one at a time");
        live.came(5, 10.0 + LEAD, WINDOW);
        assert_eq!(live.want(5, 4, 10.1, true), Some((10.0 + LEAD + WINDOW, WINDOW)));
        live.came(5, 10.0 + LEAD + WINDOW, WINDOW);
        // up to AHEAD of the playhead, then again as it moves on
        let mut asked = 2;
        while let Some((from, secs)) = live.want(5, 4, 10.2, true) {
            live.came(5, from, secs);
            asked += 1;
        }
        assert_eq!(asked, 5, "from 10.15 s to 15.15 s, four seconds ahead of the playhead at 10.2 s");
        let (from, secs) = live.want(5, 4, 12.5, true).unwrap();
        assert!((from - 15.15).abs() < 1e-9 && secs == WINDOW, "{from}");
        live.came(5, 15.15, WINDOW);
        // the mix changes again: from the playhead, for the new version; a window of the old one that comes late does
        // not move where the new ones go
        assert_eq!(live.want(6, 4, 13.0, true), Some((13.0 + LEAD, WINDOW)));
        live.came(5, 20.0, WINDOW);
        assert_eq!(live.want(6, 4, 13.0, true), Some((13.0 + LEAD, WINDOW)));
        // a seek far ahead: from the playhead again
        live.came(6, 13.0 + LEAD, WINDOW);
        assert_eq!(live.want(6, 4, 60.0, true), Some((60.0 + LEAD, WINDOW)));
        // refused (a track's own sound changed): none until the song changes again
        live.refused();
        assert_eq!(live.want(6, 4, 60.0, true), None);
        assert_eq!(live.want(7, 4, 60.0, true), Some((60.0 + LEAD, WINDOW)));
        // the render caught up: nothing more
        live.came(7, 60.0 + LEAD, WINDOW);
        assert_eq!(live.want(7, 7, 61.0, true), None);
    }
}

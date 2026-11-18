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
    /// the version of the song the server had no windows of (an edit of a track's own sound): none are asked for it
    refused: Option<u64>,
}

impl Live {
    /// The window to ask for now, `from` and `secs`, if any: while the song plays (`playing`, its playhead at `at`)
    /// and is newer (`song`) than the render playing (`rendered`), one window after another from just after the
    /// playhead, up to `AHEAD` of it; again from the playhead when the song changes again.
    pub fn want(&mut self, song: u64, rendered: u64, at: f64, playing: bool) -> Option<(f64, f64)> {
        if !playing || song <= rendered || self.waiting || self.refused == Some(song) {
            return None;
        }
        if song != self.version {
            (self.version, self.next) = (song, at + LEAD);
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
            self.next = start + secs;
        }
    }

    /// The server has no windows of the song as it is (the next render plays it): none are asked for until it
    /// changes again.
    pub fn refused(&mut self) {
        self.waiting = false;
        self.refused = Some(self.version);
    }

    /// none was asked for after all (a backend that makes no windows)
    pub fn none(&mut self) {
        self.waiting = false;
        self.refused = Some(self.version);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

//! A play pressed while the page's next audio is on its way (the page's player, `web.rs`): kept for that audio as the
//! desktop player keeps one, across another render of the same song, until a pause or a stop takes it back; dropped for
//! another song, and once the song has played to its end by itself. Apart from the player, so that it is tested where
//! the page's player is not built.

/// whether the audio on its way plays once it is here
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Pending {
    play: bool,
}

impl Pending {
    /// A load begins: the song playing plays the new audio once it is here, and a play pressed before it holds for a
    /// render of the same song (`keep_position`), not for another song.
    pub(super) fn load(&mut self, was_playing: bool, keep_position: bool) {
        self.play = was_playing || (keep_position && self.play);
    }

    /// play pressed while audio is on its way
    pub(super) fn play(&mut self) {
        self.play = true;
    }

    /// a pause or a stop pressed
    pub(super) fn pause(&mut self) {
        self.play = false;
    }

    /// The song played to its end by itself: played through, it waits for a press again, as it would have without
    /// audio on its way.
    pub(super) fn ended(&mut self) {
        self.play = false;
    }

    /// the song is gone, or its audio could not be read
    pub(super) fn forget(&mut self) {
        self.play = false;
    }

    /// the audio is here: whether it plays, said once
    pub(super) fn arrived(&mut self) -> bool {
        std::mem::take(&mut self.play)
    }
}

#[cfg(test)]
mod tests {
    use super::Pending;

    #[test]
    fn a_play_pressed_while_the_first_render_is_on_its_way_holds_across_the_next_render() {
        let mut p = Pending::default();
        p.load(false, false);
        p.play();
        // the song changed again before any audio played: nothing was playing, so `was_playing` is false
        p.load(false, true);
        assert!(p.arrived(), "the press holds");
        assert!(!p.arrived(), "said once");
    }

    #[test]
    fn a_song_played_to_its_end_stays_there_when_its_next_render_arrives() {
        let mut p = Pending::default();
        // playing as the next render began
        p.load(true, true);
        p.ended();
        assert!(!p.arrived());
    }

    #[test]
    fn a_pause_or_a_stop_takes_a_pending_play_back_and_another_song_drops_it() {
        let mut p = Pending::default();
        p.play();
        p.pause();
        p.load(false, true);
        assert!(!p.arrived());
        p.play();
        p.load(false, false);
        assert!(!p.arrived(), "another song");
    }

    #[test]
    fn the_song_playing_plays_its_next_render_and_so_does_another_song_after_it() {
        let mut p = Pending::default();
        p.load(true, true);
        assert!(p.arrived());
        p.load(true, false);
        assert!(p.arrived());
        p.play();
        p.forget();
        assert!(!p.arrived());
    }
}

//! When the demo's next render comes (`worker::schedule`): one timer at a time, set for the soonest render a change
//! asks for. A change only to the mix asks for one after a wait (`Host::waits`, up to two and a half seconds, while the
//! studio hears it by windows); a change of another kind, a track's sound or another song, asks for one at once, and
//! takes the place of the wait set before it rather than waiting it out. Apart from the worker, so that it is tested
//! where the worker is not built.

/// the time the render on its way comes, in milliseconds of the page's clock, if one is
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub(crate) struct Due {
    at: Option<f64>,
}

impl Due {
    /// A render wanted `ms` after `now`: true when a timer is to be set for it, the one before it (due later) cleared;
    /// false when the render on its way comes as soon or sooner, and stands.
    pub(crate) fn want(&mut self, now: f64, ms: f64) -> bool {
        let at = now + ms;
        match self.at {
            Some(due) if due <= at => false,
            _ => {
                self.at = Some(at);
                true
            }
        }
    }

    /// the render came: none is on its way
    pub(crate) fn fired(&mut self) {
        self.at = None;
    }
}

#[cfg(test)]
mod tests {
    use super::Due;

    #[test]
    fn a_change_that_renders_at_once_takes_the_place_of_a_mix_change_s_wait() {
        let mut due = Due::default();
        // a level moved while the song plays: a render after the wait
        assert!(due.want(0.0, 2501.0));
        // a track's sound changed a moment later: at once, the wait's timer cleared
        assert!(due.want(400.0, 30.0));
        assert_eq!(due.at, Some(430.0));
    }

    #[test]
    fn a_render_on_its_way_stands_against_one_wanted_later() {
        let mut due = Due::default();
        assert!(due.want(0.0, 30.0));
        // a change to the mix right after: the render already coming carries it
        assert!(!due.want(10.0, 2501.0));
        assert!(!due.want(20.0, 10.0), "as soon is not sooner");
        assert_eq!(due.at, Some(30.0));
    }

    #[test]
    fn once_the_render_came_the_next_change_sets_a_timer_of_its_own() {
        let mut due = Due::default();
        assert!(due.want(0.0, 30.0));
        due.fired();
        assert!(due.want(100.0, 2501.0));
        assert_eq!(due.at, Some(2601.0));
    }
}

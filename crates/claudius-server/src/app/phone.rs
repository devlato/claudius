//! What the session keeps of a phone playing along: the studio's transport, which the phone follows, and the page
//! while it is served (`crate::phone` serves it).

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use claudius::take::{self, Pads, Played, Struck};
use claudius::{scratch, theory::Note};
use claudius_api::{PhoneView, PlayAsk, Reply, Transport};

use super::App;

/// how long a phone counts as there after it last asked for the time
const SEEN: Duration = Duration::from_secs(10);

/// the page being served: where, the phones that have it open, and how to stop it
pub(crate) struct Running {
    pub url: String,
    /// dropping it stops the page's server
    pub _stop: tokio::sync::oneshot::Sender<()>,
    pub seen: HashMap<IpAddr, Instant>,
    /// the song played or stopped from a phone, last (`ask_play`)
    pub play: Option<PlayAsk>,
}

/// a take played on a phone, as the edit it makes: what a phone may change, and nothing else a command could do
pub(crate) enum Take {
    Hits { from: String, struck: Vec<Struck>, pads: Pads },
    Notes { from: String, notes: Vec<Note>, played: Played },
    Scratch { from: String, scratch: scratch::Take, name: Option<String>, role: Option<String> },
}

impl App {
    /// a take played on a phone, into the song
    pub(crate) fn take(self: &Arc<Self>, t: Take) -> Reply {
        match t {
            Take::Hits { from, struck, pads } => self.edit_with(|bp| take::hits(bp, &from, &struck, &pads)),
            Take::Notes { from, notes, played } => self.edit_with(|bp| take::played(bp, &from, &notes, &played)),
            Take::Scratch { from, scratch, name, role } => {
                self.add_scratch(&from, &scratch, name.as_deref(), role.as_deref())
            }
        }
    }

    /// where the studio's song is, as the studio says (`POST /api/transport`)
    pub fn set_transport(&self, t: Transport) {
        *self.transport.lock().unwrap_or_else(|e| e.into_inner()) = (t, Instant::now());
    }

    /// the studio's transport, and when it was said
    pub(crate) fn transport(&self) -> (Transport, Instant) {
        *self.transport.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn phone_view(&self) -> Option<PhoneView> {
        let p = self.phone.lock().unwrap_or_else(|e| e.into_inner());
        p.as_ref().map(|r| PhoneView { url: r.url.clone(), phones: phones(r), play: r.play.clone() })
    }

    /// a phone asks the studio to play the song from `at` seconds (from where it is, when none), or to stop it: an ask
    /// of its own, which the studio sees in the state and does
    pub(crate) fn ask_play(&self, playing: bool, at: Option<f64>) {
        {
            let mut p = self.phone.lock().unwrap_or_else(|e| e.into_inner());
            let Some(r) = p.as_mut() else { return };
            let id = r.play.as_ref().map_or(1, |a| a.id + 1);
            r.play = Some(PlayAsk { id, playing, at: at.filter(|t| t.is_finite() && *t >= 0.0) });
        }
        self.update(|_| ());
    }

    /// the page served from now on (or none: the one served stops)
    pub(crate) fn set_phone(&self, r: Option<Running>) {
        let old = std::mem::replace(&mut *self.phone.lock().unwrap_or_else(|e| e.into_inner()), r);
        drop(old);
        self.update(|_| ());
    }

    /// a phone at `ip` asked for the time; the studio hears when one comes or goes
    pub(crate) fn phone_seen(&self, ip: IpAddr) {
        let changed = {
            let mut p = self.phone.lock().unwrap_or_else(|e| e.into_inner());
            let Some(r) = p.as_mut() else { return };
            let before = phones(r);
            r.seen.insert(ip, Instant::now());
            r.seen.retain(|_, t| t.elapsed() < SEEN);
            phones(r) != before
        };
        if changed {
            self.update(|_| ());
        }
    }
}

fn phones(r: &Running) -> u32 {
    r.seen.values().filter(|t| t.elapsed() < SEEN).count() as u32
}

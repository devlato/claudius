//! The studio's window as the system's other apps are there: its title says the song; the system's media keys and its
//! Now Playing (Control Center on a Mac, the media overlay on Windows, MPRIS on Linux) play and pause it, move it
//! through its sections and seek in it; and when something the studio did in the background for a while is done, or
//! an assistant asks for a take, the system says so (a notification, and the Dock or the taskbar asking for attention).

use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use claudius_api::{State, TakeStatus};
use eframe::egui;
use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig};

use crate::audio::Player;

/// how long the studio works on something before its end is worth a notification: a render after an edit takes a few
/// seconds, an evolution, a remake or an analysis minutes
pub const LONG: f64 = 20.0;

/// how far a seek without an amount goes, in seconds
const SEEK: f64 = 5.0;

/// what the app tells the system, and what the system asks of it
pub struct Desktop {
    media: Option<MediaControls>,
    events: Receiver<MediaControlEvent>,
    /// the window's title as told
    title: String,
    /// the song as Now Playing was told it last
    told: Option<Told>,
    /// what the studio is busy with
    busy: Option<Job>,
    /// the take request said last
    asked: Option<u64>,
}

/// the song as Now Playing was told it: its title and length, whether it played, and where, at `when` on the studio's
/// clock
#[derive(Clone, Debug, PartialEq)]
struct Told {
    song: String,
    length: f64,
    playing: bool,
    at: f64,
    when: f64,
}

impl Told {
    /// whether the song is at `at`, `now`, somewhere else than where Now Playing has it from what it was told: a
    /// seek, a section gone to, the song started again
    fn moved(&self, at: f64, now: f64) -> bool {
        let gone_on = if self.playing { now - self.when } else { 0.0 };
        (self.at + gone_on - at).abs() > 1.0
    }
}

/// a job the studio is busy with: what it is (what its steps say before their colon), since when on the studio's
/// clock, and whether the window was ever away while it ran
#[derive(Clone, Debug, PartialEq)]
pub struct Job {
    name: String,
    since: f64,
    away: bool,
}

impl Desktop {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Desktop {
        let (tx, events) = channel();
        let ctx = cc.egui_ctx.clone();
        let config =
            PlatformConfig { display_name: "Claudius", dbus_name: "io.github.devlato.claudius", hwnd: window(cc) };
        let media = MediaControls::new(config).ok().and_then(|mut m| {
            let attached = m.attach(move |e| {
                let _ = tx.send(e);
                ctx.request_repaint();
            });
            attached.ok().map(|_| m)
        });
        #[cfg(target_os = "macos")]
        let _ = notify_rust::set_application("io.github.devlato.claudius");
        Desktop { media, events, title: String::new(), told: None, busy: None, asked: None }
    }

    /// What the system asked of the song since the last frame, done; the song as the system shows it, told; and what
    /// was done in the background while the window was not in front, said. `now` is the studio's clock.
    pub fn tick(&mut self, ctx: &egui::Context, state: &State, player: &mut Player, now: f64) {
        let bpm = state.render.as_ref().map(|r| r.bpm).filter(|b| *b > 0.0).or(state.song.as_ref().map(|s| s.bpm));
        let starts: Vec<f64> = match (&state.song, bpm) {
            (Some(s), Some(bpm)) => s.sections.iter().map(|x| x.start_bar as f64 * 240.0 / bpm).collect(),
            _ => Vec::new(),
        };
        while let Ok(e) = self.events.try_recv() {
            let at = player.position();
            match e {
                MediaControlEvent::Play => player.play(),
                MediaControlEvent::Pause => player.pause(),
                MediaControlEvent::Stop => player.stop(),
                MediaControlEvent::Toggle if player.is_playing() => player.pause(),
                MediaControlEvent::Toggle => player.play(),
                MediaControlEvent::Next => player.seek(next(&starts, at).unwrap_or(at)),
                MediaControlEvent::Previous => player.seek(previous(&starts, at)),
                MediaControlEvent::Seek(d) => player.seek((at + sign(d) * SEEK).max(0.0)),
                MediaControlEvent::SeekBy(d, by) => player.seek((at + sign(d) * by.as_secs_f64()).max(0.0)),
                MediaControlEvent::SetPosition(MediaPosition(to)) => player.seek(to.as_secs_f64()),
                _ => {}
            }
        }
        let title = title(state.song.as_ref().map(|s| s.title.as_str()));
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
        if let Some(media) = self.media.as_mut() {
            let song = state.song.as_ref().map_or_else(String::new, |s| s.title.clone());
            let (length, at, playing) = (player.duration().max(0.0), player.position().max(0.0), player.is_playing());
            // the length is known once the render has loaded, after the title
            let renamed = self.told.as_ref().is_none_or(|t| t.song != song || (t.length - length).abs() > 0.5);
            if renamed {
                let genre = state.song.as_ref().map(|s| s.genre.replace('_', " "));
                let meta = MediaMetadata {
                    title: Some(&song).filter(|s| !s.is_empty()).map(String::as_str),
                    artist: Some("Claudius"),
                    album: genre.as_deref(),
                    duration: (length > 0.0).then(|| Duration::from_secs_f64(length)),
                    ..Default::default()
                };
                let _ = media.set_metadata(meta);
            }
            if renamed || self.told.as_ref().is_none_or(|t| t.playing != playing || t.moved(at, now)) {
                let progress = Some(MediaPosition(Duration::from_secs_f64(at)));
                let _ = media.set_playback(match playing {
                    true => MediaPlayback::Playing { progress },
                    false => MediaPlayback::Paused { progress },
                });
                self.told = Some(Told { song, length, playing, at, when: now });
            }
        }
        let away = !ctx.input(|i| i.viewport().focused.unwrap_or(true));
        let said = state.log.last().map(String::as_str);
        let (busy, done) = step(self.busy.take(), state.busy.as_deref(), away, now, said);
        self.busy = busy;
        if let Some((summary, body)) = done {
            tell(ctx, &summary, &body);
        }
        let waiting = state.take_request.as_ref().filter(|r| r.status == TakeStatus::Waiting);
        if let Some(r) = waiting.filter(|r| self.asked != Some(r.id)) {
            self.asked = Some(r.id);
            if away {
                let what = if r.note.is_empty() { format!("from {}", r.from) } else { r.note.clone() };
                tell(ctx, "An assistant asks for a take", &what);
            }
        }
    }
}

/// a notification, and the Dock or the taskbar asking for attention
fn tell(ctx: &egui::Context, summary: &str, body: &str) {
    ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(egui::UserAttentionType::Informational));
    let (summary, body) = (summary.to_string(), body.to_string());
    // a notification server can be slow to answer: the frame does not wait for it
    std::thread::spawn(move || {
        let _ = notify_rust::Notification::new().appname("Claudius").summary(&summary).body(&body).show();
    });
}

/// the window's title: the song's, and the app's
pub fn title(song: Option<&str>) -> String {
    match song.filter(|s| !s.trim().is_empty()) {
        Some(s) => format!("{s} — Claudius"),
        None => "Claudius".to_string(),
    }
}

/// The job the studio is busy with after a frame, from `was`, the one before it, and `busy`, what the state says it
/// is busy with, the window `away` or not, `now`; and what to say of a job that ended: its name, capitalised, done,
/// with what the log `said` last, for one that took `LONG` or more while the window was away at some time. A job's
/// steps are said after a colon ("evolving: generation 2 of 3"), and are the same job; another job after it is one of
/// its own.
pub fn step(
    was: Option<Job>,
    busy: Option<&str>,
    away: bool,
    now: f64,
    said: Option<&str>,
) -> (Option<Job>, Option<(String, String)>) {
    let name = busy.map(|b| b.split(':').next().unwrap_or(b).trim().to_string());
    match (was, name) {
        (Some(mut job), Some(name)) if job.name == name => {
            job.away |= away;
            (Some(job), None)
        }
        (was, name) => {
            let ended = was.filter(|j| (j.away || away) && now - j.since >= LONG).map(|j| {
                let mut c = j.name.chars();
                let name: String = c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default();
                (format!("{name} is done"), said.unwrap_or_default().to_string())
            });
            (name.map(|name| Job { name, since: now, away }), ended)
        }
    }
}

/// where the section after the one playing at `at` starts, in seconds; none in the last
pub fn next(starts: &[f64], at: f64) -> Option<f64> {
    starts.iter().copied().find(|&s| s > at + 0.05)
}

/// where the section playing at `at` starts, or the one before it within its first two seconds, as a player's
/// Previous goes
pub fn previous(starts: &[f64], at: f64) -> f64 {
    let current = starts.iter().copied().rfind(|&s| s <= at + 0.05).unwrap_or(0.0);
    match at - current < 2.0 {
        true => starts.iter().copied().rfind(|&s| s < current - 0.05).unwrap_or(0.0),
        false => current,
    }
}

fn sign(d: souvlaki::SeekDirection) -> f64 {
    match d {
        souvlaki::SeekDirection::Forward => 1.0,
        souvlaki::SeekDirection::Backward => -1.0,
    }
}

/// the window's handle, which Windows' media controls need
#[cfg(target_os = "windows")]
fn window(cc: &eframe::CreationContext<'_>) -> Option<*mut std::ffi::c_void> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match cc.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(h.hwnd.get() as *mut std::ffi::c_void),
        _ => None,
    }
}

#[cfg(not(target_os = "windows"))]
fn window(_: &eframe::CreationContext<'_>) -> Option<*mut std::ffi::c_void> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_title_says_the_song() {
        assert_eq!(title(Some("Glass Garden")), "Glass Garden — Claudius");
        assert_eq!((title(None), title(Some("  "))), ("Claudius".to_string(), "Claudius".to_string()));
    }

    #[test]
    fn a_long_job_s_end_is_said_when_the_window_was_away_while_it_ran() {
        let (job, said) = step(None, Some("evolving: generation 1 of 3"), false, 0.0, None);
        assert_eq!(said, None);
        let (job, said) = step(job, Some("evolving: generation 2 of 3"), true, 30.0, None);
        assert_eq!(said, None, "a step of the same job");
        let (job, said) = step(job, None, false, 90.0, Some("evolved 3 generations: 91/100"));
        let evolved = ("Evolving is done".to_string(), "evolved 3 generations: 91/100".to_string());
        assert_eq!((job, said), (None, Some(evolved)), "away while it ran, though in front as it ended");
        let (job, _) = step(None, Some("rendering"), true, 50.0, None);
        assert_eq!(step(job, None, true, 54.0, None).1, None, "a render after an edit");
        let (job, _) = step(None, Some("remaking"), false, 0.0, None);
        assert_eq!(step(job, None, false, 90.0, None).1, None, "the window in front all along");
    }

    #[test]
    fn a_job_after_another_is_one_of_its_own() {
        let (job, _) = step(None, Some("evolving: generation 3 of 3"), true, 0.0, None);
        let (job, said) = step(job, Some("rendering"), true, 60.0, Some("evolved 3 generations"));
        assert_eq!(said, Some(("Evolving is done".into(), "evolved 3 generations".into())), "under its own name");
        assert_eq!(step(job, None, true, 64.0, None).1, None, "the render after it took seconds, from when it began");
    }

    #[test]
    fn now_playing_hears_of_a_seek_but_not_of_the_song_playing_on() {
        let told = Told { song: "Glass Garden".into(), length: 120.0, playing: true, at: 10.0, when: 100.0 };
        assert!(!told.moved(15.0, 105.0), "it plays on");
        assert!(told.moved(48.0, 105.0), "Next went to a section");
        let paused = Told { playing: false, ..told };
        assert!(!paused.moved(10.0, 200.0) && paused.moved(0.0, 200.0), "a seek while it was paused");
    }

    #[test]
    fn next_and_previous_go_by_the_sections() {
        let starts = [0.0, 16.0, 48.0];
        assert_eq!((next(&starts, 3.0), next(&starts, 16.0), next(&starts, 50.0)), (Some(16.0), Some(48.0), None));
        assert_eq!(previous(&starts, 30.0), 16.0, "to the start of the section playing");
        assert_eq!(previous(&starts, 17.0), 0.0, "within its first two seconds, to the one before");
        assert_eq!(previous(&starts, 1.0), 0.0);
    }
}

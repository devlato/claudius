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
    /// the window's title as told, the song as Now Playing shows it and whether it plays
    title: String,
    playing: Option<(String, bool)>,
    /// what the studio was busy with, and since when (the studio's clock)
    busy: Option<(String, f64)>,
    /// the take request said last
    asked: Option<u64>,
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
        Desktop { media, events, title: String::new(), playing: None, busy: None, asked: None }
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
                MediaControlEvent::Pause | MediaControlEvent::Stop => player.pause(),
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
            let playing = (song.clone(), player.is_playing());
            if self.playing.as_ref().is_none_or(|(s, _)| *s != song) {
                let genre = state.song.as_ref().map(|s| s.genre.replace('_', " "));
                let length = Duration::from_secs_f64(player.duration().max(0.0));
                let meta = MediaMetadata {
                    title: Some(&song).filter(|s| !s.is_empty()).map(String::as_str),
                    artist: Some("Claudius"),
                    album: genre.as_deref(),
                    duration: (length > Duration::ZERO).then_some(length),
                    ..Default::default()
                };
                let _ = media.set_metadata(meta);
            }
            if self.playing.as_ref() != Some(&playing) {
                let progress = Some(MediaPosition(Duration::from_secs_f64(player.position().max(0.0))));
                let _ = media.set_playback(match playing.1 {
                    true => MediaPlayback::Playing { progress },
                    false => MediaPlayback::Paused { progress },
                });
                self.playing = Some(playing);
            }
        }
        let away = !ctx.input(|i| i.viewport().focused.unwrap_or(true));
        if let Some(said) = done(&self.busy, state.busy.as_deref(), now, state.log.last().map(String::as_str))
            && away
        {
            tell(ctx, &said.0, &said.1);
        }
        self.busy = match (&self.busy, &state.busy) {
            (Some((was, since)), Some(b)) if was == b => Some((was.clone(), *since)),
            (_, Some(b)) => Some((b.clone(), self.busy.as_ref().map_or(now, |(_, since)| *since))),
            (_, None) => None,
        };
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

/// What to say once the studio is no longer busy (`busy` none) with what it was busy with since `since`, as `now` is:
/// the job, its first word capitalised, done, with what the log said last; nothing for one that took less than `LONG`.
pub fn done(was: &Option<(String, f64)>, busy: Option<&str>, now: f64, said: Option<&str>) -> Option<(String, String)> {
    let (job, since) = was.as_ref()?;
    if busy.is_some() || now - since < LONG {
        return None;
    }
    // a job's steps are said after a colon ("evolving: generation 2 of 3 done"): the job is what comes before
    let job = job.split(':').next().unwrap_or(job).trim();
    let mut c = job.chars();
    let job: String = c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default();
    Some((format!("{job} is done"), said.unwrap_or_default().to_string()))
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
    fn only_a_long_job_s_end_is_said() {
        let job = Some(("evolving: generation 1 of 3".to_string(), 0.0));
        assert_eq!(done(&job, Some("evolving: generation 2 of 3"), 60.0, None), None, "still at it");
        assert_eq!(done(&Some(("rendering".into(), 50.0)), None, 54.0, None), None, "a render after an edit");
        let said = done(&job, None, 90.0, Some("evolved 3 generations: 91/100"));
        assert_eq!(said, Some(("Evolving is done".into(), "evolved 3 generations: 91/100".into())));
        assert_eq!(done(&None, None, 90.0, None), None);
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

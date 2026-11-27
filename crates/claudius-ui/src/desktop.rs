//! The studio's window as the system's other apps are there: its title says the song; the system's media keys and its
//! Now Playing (Control Center on a Mac, the media overlay on Windows, MPRIS on Linux) play and pause it, move it
//! through its sections and seek in it; and when something the studio did in the background for a while is done, or
//! an assistant asks for a take, the system says so (a notification, and the Dock or the taskbar asking for attention).

use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use claudius_api::{ChatMessage, ChatView, State, TakeStatus};
use eframe::egui;
use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig};

use crate::audio::Player;
use crate::panels::chat::unread;

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
    /// when the app started: the clock its jobs are timed on, which goes on while the window is hidden, as the
    /// studio's input time does not
    born: Instant,
    /// the chat's last message seen: what an assistant writes after it is said, not what was there at the start
    read: Option<u64>,
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
/// clock, whether the window was ever away while it ran, the song's score and version as it began, which what it
/// came to is told against, the log's last line as it began (the lines after it are the job's), and when the studio
/// stopped being busy with it, while its end settles (`SETTLE`)
#[derive(Clone, Debug, PartialEq)]
pub struct Job {
    name: String,
    since: f64,
    away: bool,
    score: Option<f64>,
    version: u64,
    mark: Option<String>,
    ended: Option<f64>,
}

impl Job {
    /// whether the studio stopped being busy with it and its end is settling
    pub fn settling(&self) -> bool {
        self.ended.is_some()
    }
}

/// What the studio is at in a frame, as a job's end is told from it: what it is busy with, whether the window is
/// away, the time on the studio's clock, the log, and the song's title, score and version.
#[derive(Clone, Copy, Debug, Default)]
pub struct At<'a> {
    pub busy: Option<&'a str>,
    pub away: bool,
    pub now: f64,
    pub log: &'a [String],
    pub title: Option<&'a str>,
    pub score: Option<f64>,
    pub version: u64,
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
        let born = Instant::now();
        Desktop { media, events, title: String::new(), told: None, busy: None, asked: None, born, read: None }
    }

    /// What the system asked of the song since the last frame, done; the song as the system shows it, told; and what
    /// was done in the background while the window was not in front, said. The app calls it from its logic, which
    /// eframe runs while the window is hidden too, and its `ui` not.
    pub fn tick(&mut self, ctx: &egui::Context, state: &State, player: &mut Player) {
        let now = self.born.elapsed().as_secs_f64();
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
        let away = ctx.input(|i| {
            let v = i.viewport();
            !v.focused.unwrap_or(true) || v.minimized == Some(true) || v.occluded == Some(true)
        });
        let at = At {
            busy: state.busy.as_deref(),
            away,
            now,
            log: &state.log,
            title: state.song.as_ref().map(|s| s.title.as_str()),
            score: state.verdict.as_ref().map(|v| v.total),
            version: state.song_version,
        };
        let (busy, done) = step(self.busy.take(), &at);
        // a job whose end is settling is told once it has settled, though nothing else wakes the window by then
        if busy.as_ref().is_some_and(Job::settling) {
            ctx.request_repaint_after(Duration::from_secs_f64(SETTLE));
        }
        self.busy = busy;
        if let Some((summary, body)) = done {
            tell(ctx, &summary, &body);
        }
        // none until the first state has come from claudius: the studio starts with an empty one
        if state.version > 0 {
            let (new, read) = to_say(&state.chat, self.read);
            for m in new.into_iter().filter(|_| away) {
                if let Some(who) = m.from.as_deref() {
                    tell(ctx, &format!("{who} writes"), &m.text);
                }
            }
            self.read = Some(read);
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

/// The chat's messages to say, after `read`, the last one said (none yet in this run), and the last one said after
/// them. An agent's answer is said once it is whole, and what it does (a note) not at all; what was written after an
/// answer still being written waits for it.
fn to_say(chat: &ChatView, read: Option<u64>) -> (Vec<&ChatMessage>, u64) {
    let latest = chat.messages.last().map_or(0, |m| m.id);
    let last = chat.writing.map_or(latest, |w| latest.min(w.saturating_sub(1)));
    let after = unread(read, last);
    (chat.messages.iter().filter(|m| m.id > after && m.id <= last && !m.note).collect(), last)
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

/// The job the studio is busy with after a frame, from `was`, the one before it, and what the studio is `at`; and what
/// to tell of a job that ended (`notice`), one that took `LONG` or more while the window was away at some time. A
/// job's steps are said after a colon ("evolving: generation 2 of 3"), and are the same job, and so are the steps that
/// go on with it (`goes_on`): a render and the critic's listening to it, the packs one after another, one recording
/// taken apart and then listened to. A job's end settles for `SETTLE` before it is told, so that a step that goes on
/// with it after the studio was idle a moment is the same job, and its last lines reach the log; another job after it
/// is one of its own, and ends the one before at once.
pub fn step(was: Option<Job>, at: &At) -> (Option<Job>, Option<(String, String)>) {
    let name = at.busy.map(|b| b.split(':').next().unwrap_or(b).trim().to_string());
    match (was, name) {
        (Some(mut job), Some(name))
            if (job.name == name || goes_on(&job.name, &name)) && job.ended.is_none_or(|end| at.now - end < SETTLE) =>
        {
            job.away |= at.away;
            job.ended = None;
            // the critic's listening to a render is the render's last step: the job stays the render it is
            if !(job.name == "rendering" && name == "listening") {
                job.name = name;
            }
            (Some(job), None)
        }
        (Some(mut job), None) => match job.ended {
            Some(end) if at.now - end >= SETTLE => (None, told(&job, end, at)),
            ended => {
                job.away |= at.away;
                job.ended = Some(ended.unwrap_or(at.now));
                (Some(job), None)
            }
        },
        (was, name) => {
            let ended = was.and_then(|j| told(&j, j.ended.unwrap_or(at.now), at));
            let job = name.map(|name| Job {
                name,
                since: at.now,
                away: at.away,
                score: at.score,
                version: at.version,
                mark: at.log.last().cloned(),
                ended: None,
            });
            (job, ended)
        }
    }
}

/// how long a job's end settles before it is told, in seconds
pub const SETTLE: f64 = 3.0;

/// what to tell of `job`, which ended at `end`: its notice where it took `LONG` or more and the window was away at
/// some time while it ran, or is as it is told
fn told(job: &Job, end: f64, at: &At) -> Option<(String, String)> {
    ((job.away || at.away) && end - job.since >= LONG).then(|| notice(job, at))
}

/// whether the step `next` goes on with the job `was`: the critic's listening to a render, a pack's download after
/// another's, and the recording `was` was at taken apart, then listened to
fn goes_on(was: &str, next: &str) -> bool {
    (was == "rendering" && next == "listening")
        || (was.starts_with("downloading") && next.starts_with("downloading"))
        || matches!((recording(was), recording(next)), (Some(a), Some(b)) if a == b)
}

/// the log's lines written since `job` began: those after the line that was last as it began, or all of them once
/// that line has rolled out of the log
fn written<'a>(job: &Job, at: &At<'a>) -> &'a [String] {
    match job.mark.as_ref().and_then(|m| at.log.iter().rposition(|l| l == m)) {
        Some(i) => &at.log[i + 1..],
        None => at.log,
    }
}

/// the recording a job listens to, takes apart or remakes, by the words its name puts it in
fn recording(job: &str) -> Option<&str> {
    let after = |prefix: &str| job.strip_prefix(prefix);
    after("taking the groove of ")
        .or_else(|| after("taking ").and_then(|r| r.strip_suffix(" apart into its voice, drums, bass and the rest")))
        .or_else(|| after("listening to "))
        .or_else(|| after("sampling the voice of "))
        .or_else(|| after("remaking ").map(|r| r.rsplit_once(" as ").map_or(r, |(x, _)| x)))
}

/// What to tell of a job that ended, as a person would say it: what is ready, and what came of it in a sentence; never
/// the log's line, which says a render's seconds and loudness, or a file's path. A job that did not finish says so,
/// and that the Log says why.
fn notice(job: &Job, at: &At) -> (String, String) {
    let song = match at.title.map(str::trim).filter(|t| !t.is_empty()) {
        Some(t) => format!("\u{201c}{t}\u{201d}"),
        None => "The song".to_string(),
    };
    let name = job.name.as_str();
    if written(job, at).iter().any(|l| went_wrong(l)) {
        return (format!("Claudius could not finish {}", doing(name)), "The Log in the studio says why.".into());
    }
    let scores = at.score.map_or_else(String::new, |s| format!("It scores {s:.0} out of 100."));
    match name {
        "rendering" => (format!("{song} is ready to play"), scores),
        "saving" => (format!("{song} is saved"), String::new()),
        "mutating" => ("The variations are done".into(), better(job, at)),
        "evolving" => (format!("Improving {} is done", song_in_words(&song)), better(job, at)),
        n if n.starts_with("downloading") => ("The sample packs are downloaded".into(), "New songs play them.".into()),
        n if n.starts_with("remaking ") => match n.trim_start_matches("remaking ").rsplit_once(" as ") {
            Some((x, genre)) => (format!("{} is remade as {genre}", shown(x)), String::new()),
            None => ("The remake is ready".into(), String::new()),
        },
        n if n.starts_with("taking the groove of ") => {
            (format!("The groove of {} is in the song", shown(&n["taking the groove of ".len()..])), String::new())
        }
        n if n.starts_with("sampling the voice of ") => {
            (format!("The voice of {} is sampled", shown(&n["sampling the voice of ".len()..])), String::new())
        }
        n if n.starts_with("listening to ") || n.starts_with("taking ") => {
            (format!("Claudius has listened to {}", shown(recording(n).unwrap_or(n))), String::new())
        }
        n if n.starts_with("sampling") => ("The samples are in the song".into(), String::new()),
        n => (format!("{} is done", capitalized(&shown(n))), String::new()),
    }
}

/// "The song" as it reads inside a sentence, a title as it is
fn song_in_words(song: &str) -> String {
    if song == "The song" { "the song".into() } else { song.into() }
}

/// what came of trying variations or improving the song, against its score as the job began
fn better(job: &Job, at: &At) -> String {
    match (job.score, at.score) {
        (Some(was), Some(now)) if at.version != job.version && now > was + 0.5 => {
            format!("It scores {now:.0} now, up from {was:.0}.")
        }
        (Some(_), _) => "None of them beat it, so it stays as it was.".into(),
        _ => String::new(),
    }
}

/// whether a line of the log says the job did not finish: it failed, could not, was stopped or cancelled
fn went_wrong(said: &str) -> bool {
    let s = said.to_lowercase();
    s.contains("failed")
        || s.starts_with("cannot ")
        || s.starts_with("could not ")
        || s.contains(" did not ")
        || s.starts_with("stopped")
        || s.starts_with("cancelled")
        || s.starts_with("canceled")
}

/// a job as a person says what claudius could not finish
fn doing(job: &str) -> String {
    match job {
        "rendering" => "rendering the song".into(),
        "saving" => "saving the song".into(),
        "mutating" => "trying variations".into(),
        "evolving" => "improving the song".into(),
        other => shown(other),
    }
}

/// a job's words with the recording in them as a person knows it: a file's name without its folder or its kind
/// ("listening to /Users/me/Downloads/Song.mp3", "listening to C:\Music\Song.mp3": "listening to Song")
fn shown(words: &str) -> String {
    let mut s = words.to_string();
    // a path's folders: from the word its first slash (or a Windows backslash) is in to its last slash
    let slash = |c: char| c == '/' || c == '\\';
    if let (Some(first), Some(last)) = (s.find(slash), s.rfind(slash)) {
        let from = s[..first].rfind(' ').map_or(0, |i| i + 1);
        s.replace_range(from..=last, "");
    }
    for kind in [".mp3", ".wav", ".flac", ".m4a", ".aiff", ".aif", ".ogg", ".opus", ".aac", ".caf"] {
        s = s.replace(&format!("{kind} "), " ");
        if let Some(stripped) = s.strip_suffix(kind) {
            s = stripped.to_string();
        }
    }
    s
}

/// `s` with its first letter a capital
fn capitalized(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
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

    fn at<'a>(busy: Option<&'a str>, away: bool, now: f64) -> At<'a> {
        At { busy, away, now, title: Some("Glass Garden"), score: Some(74.0), version: 3, ..At::default() }
    }

    /// the log as a slice of lines, as the state holds it
    fn log(lines: &[&str]) -> Vec<String> {
        lines.iter().map(|l| l.to_string()).collect()
    }

    /// what is told of `job` as the studio stops being busy at `now`: nothing yet, then its notice once it has settled
    fn ends(job: Option<Job>, at: At) -> Option<(String, String)> {
        let (job, said) = step(job, &at);
        assert_eq!(said, None, "not while its end settles");
        assert!(job.as_ref().is_none_or(Job::settling), "it settles");
        let settled = At { now: at.now + SETTLE, ..at };
        let (job, said) = step(job, &settled);
        assert_eq!(job, None, "told and gone once settled");
        said
    }

    #[test]
    fn a_long_job_s_end_is_said_when_the_window_was_away_while_it_ran() {
        let (job, said) = step(None, &at(Some("evolving: generation 1 of 3"), false, 0.0));
        assert_eq!(said, None);
        let (job, said) = step(job, &at(Some("evolving: generation 2 of 3"), true, 30.0));
        assert_eq!(said, None, "a step of the same job");
        let lines = log(&["evolved 3 generations: 81.4/100"]);
        let ended = At { score: Some(81.4), version: 9, log: &lines, ..at(None, false, 90.0) };
        let improved =
            ("Improving \u{201c}Glass Garden\u{201d} is done".to_string(), "It scores 81 now, up from 74.".to_string());
        assert_eq!(ends(job, ended), Some(improved), "away while it ran, though in front as it ended");
        let (job, _) = step(None, &at(Some("rendering"), true, 50.0));
        assert_eq!(ends(job, at(None, true, 54.0)), None, "a render after an edit");
        let (job, _) = step(None, &at(Some("remaking a.mp3 as trip hop"), false, 0.0));
        assert_eq!(ends(job, at(None, false, 90.0)), None, "the window in front all along");
    }

    #[test]
    fn a_job_after_another_is_one_of_its_own() {
        let (job, _) = step(None, &at(Some("evolving: generation 3 of 3"), true, 0.0));
        let lines = log(&["no variation beat the current song (74.0; best 71.2)"]);
        let (job, said) = step(job, &At { log: &lines, ..at(Some("rendering"), true, 60.0) });
        let kept = (
            "Improving \u{201c}Glass Garden\u{201d} is done".into(),
            "None of them beat it, so it stays as it was.".into(),
        );
        assert_eq!(said, Some(kept), "under its own name, at once: another job began");
        assert_eq!(ends(job, at(None, true, 64.0)), None, "the render after it took seconds, from when it began");
    }

    #[test]
    fn a_job_s_end_says_what_is_ready_in_words_never_the_log_s_line() {
        let (job, _) = step(None, &at(Some("rendering"), true, 0.0));
        let lines = log(&["rendered in 31.2 s: -9.1 LUFS, verdict 72.4 \u{2014} a strong groove"]);
        assert_eq!(
            ends(job, At { log: &lines, score: Some(72.4), ..at(None, true, 40.0) }),
            Some(("\u{201c}Glass Garden\u{201d} is ready to play".into(), "It scores 72 out of 100.".into()))
        );
        let (job, _) = step(None, &at(Some("rendering"), true, 0.0));
        let failed = log(&["render failed: out of memory"]);
        assert_eq!(
            ends(job, At { log: &failed, ..at(None, true, 40.0) }),
            Some(("Claudius could not finish rendering the song".into(), "The Log in the studio says why.".into()))
        );
        let (job, _) = step(None, &At { title: None, ..at(Some("rendering"), true, 0.0) });
        assert_eq!(
            ends(job, At { title: None, ..at(None, true, 40.0) }).map(|s| s.0),
            Some("The song is ready to play".into())
        );
    }

    #[test]
    fn a_render_is_ready_once_the_critic_has_listened_to_it() {
        let (job, _) = step(None, &at(Some("rendering"), true, 0.0));
        let (job, said) = step(job, &at(Some("listening"), true, 30.0));
        assert_eq!(said, None, "the critic's listening is the render's last step");
        let lines = log(&["rendered in 33.0 s: -9.4 LUFS, verdict 77.0"]);
        let ended = At { log: &lines, score: Some(77.0), version: 4, ..at(None, true, 45.0) };
        assert_eq!(
            ends(job, ended),
            Some(("\u{201c}Glass Garden\u{201d} is ready to play".into(), "It scores 77 out of 100.".into())),
            "told once, as a render, with the score the critic gave it"
        );
        let (job, _) = step(None, &at(Some("downloading packs"), true, 0.0));
        let (job, said) = step(job, &at(Some("downloading Virtuosity Drums"), true, 10.0));
        assert_eq!(said, None, "the packs one after another are one download");
        assert_eq!(ends(job, at(None, true, 60.0)).map(|s| s.0), Some("The sample packs are downloaded".into()));
    }

    #[test]
    fn a_job_that_did_not_finish_says_so_by_its_own_lines() {
        let before = log(&["render failed: out of memory"]);
        let (job, _) = step(None, &At { log: &before, ..at(Some("rendering"), true, 0.0) });
        let after = log(&["render failed: out of memory", "rendered in 30.1 s: -9.0 LUFS, verdict 70.0"]);
        assert_eq!(
            ends(job, At { log: &after, ..at(None, true, 40.0) }).map(|s| s.0),
            Some("\u{201c}Glass Garden\u{201d} is ready to play".into()),
            "a failure written before the job began is another job's"
        );
        let (job, _) = step(None, &at(Some("remaking Down Under.mp3 as trip hop"), true, 0.0));
        let stopped = log(&["stopped: Down Under.mp3 was not remade"]);
        assert_eq!(
            ends(job, At { log: &stopped, ..at(None, true, 60.0) }).map(|s| s.0),
            Some("Claudius could not finish remaking Down Under as trip hop".into()),
            "a stopped remake is not remade"
        );
        // the line saying it failed reaches the log only after the studio stopped being busy
        let (job, _) = step(None, &at(Some("taking the groove of beat.wav"), true, 0.0));
        let (job, said) = step(job, &at(None, true, 30.0));
        assert_eq!(said, None);
        let late = log(&["could not take the groove: no drums heard"]);
        let (job, said) = step(job, &At { log: &late, ..at(None, true, 30.0 + SETTLE) });
        assert_eq!(job, None);
        assert_eq!(said.map(|s| s.0), Some("Claudius could not finish taking the groove of beat".into()));
    }

    #[test]
    fn taking_a_recording_apart_and_listening_to_it_is_one_job_told_by_the_file_s_name() {
        let file = "/Users/me/Downloads/Telenova - Paralysis ghosts.mp3";
        let apart = format!("taking {file} apart into its voice, drums, bass and the rest");
        let (job, _) = step(None, &at(Some(&apart), true, 0.0));
        let listening = format!("listening to {file}");
        let (job, said) = step(job, &at(Some(&listening), true, 50.0));
        assert_eq!(said, None, "the same recording: the work goes on");
        let lines = log(&["analyzed it"]);
        assert_eq!(
            ends(job, At { log: &lines, ..at(None, true, 70.0) }),
            Some(("Claudius has listened to Telenova - Paralysis ghosts".into(), String::new()))
        );
        // with the studio idle a moment between taking it apart and listening to it
        let (job, _) = step(None, &at(Some(&apart), true, 0.0));
        let (job, said) = step(job, &at(None, true, 40.0));
        assert_eq!(said, None, "not told while its end settles");
        let (job, said) = step(job, &at(Some(&listening), true, 41.0));
        assert_eq!(said, None, "the listening goes on with it");
        assert_eq!(
            ends(job, at(None, true, 70.0)).map(|s| s.0),
            Some("Claudius has listened to Telenova - Paralysis ghosts".into()),
            "told once, at the end"
        );
        let (job, _) = step(None, &at(Some("remaking Down Under.mp3 as trip hop"), true, 0.0));
        assert_eq!(ends(job, at(None, true, 60.0)).map(|s| s.0), Some("Down Under is remade as trip hop".into()));
    }

    #[test]
    fn a_recording_is_told_by_its_name_from_a_windows_path_too() {
        assert_eq!(shown("listening to C:\\Users\\me\\Music\\Down Under.mp3"), "listening to Down Under");
        assert_eq!(shown("listening to /Users/me/Music/Down Under.mp3"), "listening to Down Under");
        assert_eq!(shown("remaking D:\\beats\\loop one.wav as house"), "remaking loop one as house");
    }

    #[test]
    fn an_answer_is_said_once_it_is_whole_though_the_person_wrote_after_it_began() {
        let said = |id: u64, from: Option<&str>, text: &str| ChatMessage {
            id,
            from: from.map(str::to_string),
            text: text.into(),
            at: 0,
            reply_to: None,
            note: false,
        };
        let mut chat = ChatView {
            messages: vec![said(1, None, "make the drop hit harder"), said(2, Some("Claude Code"), "A louder")],
            writing: Some(2),
            ..ChatView::default()
        };
        chat.messages.push(said(3, None, "and the snare?"));
        let (new, read) = to_say(&chat, Some(1));
        assert_eq!((new.len(), read), (0, 1), "not while it is written, nor what came after it");
        chat.messages[1].text = "A louder kick, and a crash on the one.".into();
        chat.writing = None;
        let (new, read) = to_say(&chat, Some(read));
        let texts: Vec<&str> = new.iter().filter(|m| m.from.is_some()).map(|m| m.text.as_str()).collect();
        assert_eq!((texts, read), (vec!["A louder kick, and a crash on the one."], 3), "whole");
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

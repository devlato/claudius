//! How fast the engine runs, as it runs: each render, each listening of the ears and each recording decoded reports how
//! many seconds of audio it went through and how long it took (`observe`). `claudius serve` shows the totals at
//! `/api/metrics` (`prometheus`), and the usage statistics, once someone has turned them on, send each kind's
//! speed at most once a day (`set_sink`). Nothing here leaves the computer by itself.

use std::sync::{Mutex, OnceLock};

/// what took the time
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Metric {
    /// a song rendered: its tracks and its master
    Render,
    /// the listening models' reading of a render (the ears)
    Ears,
    /// a recording decoded and brought to the engine's rate
    Decode,
}

impl Metric {
    pub const ALL: [Metric; 3] = [Metric::Render, Metric::Ears, Metric::Decode];

    pub fn name(self) -> &'static str {
        match self {
            Metric::Render => "render",
            Metric::Ears => "ears",
            Metric::Decode => "decode",
        }
    }

    fn index(self) -> usize {
        match self {
            Metric::Render => 0,
            Metric::Ears => 1,
            Metric::Decode => 2,
        }
    }
}

/// A kind's totals since the program started: how many times, the seconds of audio, the seconds they took, and the
/// speed of the last one (seconds of audio a second).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Totals {
    pub count: u64,
    pub audio: f64,
    pub took: f64,
    pub last: f64,
}

impl Totals {
    /// seconds of audio a second over all of them
    pub fn speed(&self) -> f64 {
        if self.took > 0.0 { self.audio / self.took } else { 0.0 }
    }
}

static TOTALS: Mutex<[Totals; 3]> = Mutex::new([Totals { count: 0, audio: 0.0, took: 0.0, last: 0.0 }; 3]);

type Sink = Box<dyn Fn(Metric, f64) + Send + Sync>;
static SINK: OnceLock<Sink> = OnceLock::new();

/// Where each measurement's speed goes besides the totals: the usage statistics, in claudius' command (which sends
/// one of each kind a day, when they are on). Set once; later calls do nothing.
pub fn set_sink(f: impl Fn(Metric, f64) + Send + Sync + 'static) {
    let _ = SINK.set(Box::new(f));
}

/// `audio` seconds of audio went through `m` in `took` seconds
pub fn observe(m: Metric, audio: f64, took: f64) {
    if !(audio > 0.0 && took > 0.0 && audio.is_finite() && took.is_finite()) {
        return;
    }
    let speed = audio / took;
    {
        let mut all = TOTALS.lock().unwrap_or_else(|e| e.into_inner());
        let t = &mut all[m.index()];
        t.count += 1;
        t.audio += audio;
        t.took += took;
        t.last = speed;
    }
    if let Some(sink) = SINK.get() {
        sink(m, speed);
    }
}

/// times `work`, which goes through the seconds of audio `audio` says of its result, as `m`
pub fn timed<T>(m: Metric, audio: impl Fn(&T) -> f64, work: impl FnOnce() -> T) -> T {
    let t = crate::clock::Instant::now();
    let out = work();
    observe(m, audio(&out), t.elapsed().as_secs_f64());
    out
}

/// each kind's totals so far
pub fn totals() -> [(Metric, Totals); 3] {
    let all = *TOTALS.lock().unwrap_or_else(|e| e.into_inner());
    Metric::ALL.map(|m| (m, all[m.index()]))
}

/// The totals in Prometheus' text format, after `extra`'s lines (each a metric's name, labels and all, its help, its
/// type and its value): what `/api/metrics` answers.
pub fn prometheus(extra: &[(&str, &str, &str, f64)]) -> String {
    let mut out = String::new();
    let mut line = |name: &str, help: &str, kind: &str, value: f64| {
        let bare = name.split('{').next().unwrap_or(name);
        out.push_str(&format!("# HELP {bare} {help}\n# TYPE {bare} {kind}\n{name} {value}\n"));
    };
    for &(name, help, kind, value) in extra {
        line(name, help, kind, value);
    }
    for (m, t) in totals() {
        let n = m.name();
        let what = match m {
            Metric::Render => "rendered",
            Metric::Ears => "listened to by the ears",
            Metric::Decode => "decoded",
        };
        line(&format!("claudius_{n}_total"), &format!("times audio was {what}"), "counter", t.count as f64);
        line(&format!("claudius_{n}_audio_seconds_total"), &format!("seconds of audio {what}"), "counter", t.audio);
        line(&format!("claudius_{n}_seconds_total"), &format!("seconds spent on audio {what}"), "counter", t.took);
        line(
            &format!("claudius_{n}_speed"),
            &format!("seconds of audio {what} a second, the last time"),
            "gauge",
            t.last,
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_measurement_adds_to_its_kinds_totals_and_shows_in_prometheus_text() {
        let before = totals()[1].1;
        observe(Metric::Ears, 30.0, 1.5);
        observe(Metric::Ears, 0.0, 1.0);
        observe(Metric::Ears, 10.0, f64::NAN);
        let after = totals()[1].1;
        assert_eq!(after.count, before.count + 1, "a measurement of nothing, or of no time, is none");
        assert!((after.audio - before.audio - 30.0).abs() < 1e-9 && (after.last - 20.0).abs() < 1e-9);
        let text = prometheus(&[("claudius_build_info{version=\"1.2.3\"}", "its version", "gauge", 1.0)]);
        assert!(text.starts_with(
            "# HELP claudius_build_info its version\n# TYPE claudius_build_info gauge\nclaudius_build_info{version=\"1.2.3\"} 1\n"
        ));
        assert!(text.contains("# TYPE claudius_ears_audio_seconds_total counter\n"), "{text}");
        assert!(text.contains("\nclaudius_ears_speed 20\n"), "{text}");
    }
}

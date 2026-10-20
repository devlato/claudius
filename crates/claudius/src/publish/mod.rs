//! Publishing a song. What stores and distributors ask about a release, kept in the song's folder as release.json;
//! the checks they run on its artwork, its audio, the licences of its sounds and its details; and a release kit:
//! the files a distributor takes and the answers its upload form asks for.

mod artwork;
mod details;
mod rights;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

pub use artwork::{Artwork, Colour};

use crate::audio::Stereo;
use crate::export::{self, Spec};
use crate::model::{Blueprint, Project};

/// A song's release details, in its folder.
pub const FILE: &str = "release.json";

/// How a song was made. Distributors and stores ask whether music is made with AI, and label it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Made {
    /// the person wrote it, with claudius as the instrument
    Written,
    /// an assistant or claudius' generator wrote some of it, and the person shaped it
    Assisted,
    /// an assistant or claudius' generator wrote it
    Generated,
}

impl Made {
    pub const ALL: [Made; 3] = [Made::Written, Made::Assisted, Made::Generated];

    pub fn name(self) -> &'static str {
        match self {
            Made::Written => "written",
            Made::Assisted => "assisted",
            Made::Generated => "generated",
        }
    }

    pub fn describe(self) -> &'static str {
        match self {
            Made::Written => "you wrote it: the notes, the parts and the arrangement, with claudius as the instrument",
            Made::Assisted => {
                "written with help: an assistant or claudius' generator wrote some of it, and you shaped it"
            }
            Made::Generated => "an assistant or claudius' generator wrote it",
        }
    }

    /// what to tell a distributor
    pub fn disclosure(self) -> String {
        let sound = "claudius makes its sounds by synthesis and from recordings, and makes no audio, vocals or lyrics \
                     with AI.";
        match self {
            Made::Written => format!(
                "No AI: the artist wrote the music. {sound} Where a distributor asks whether the song includes \
                 AI-generated music, vocals or lyrics, the answer is no."
            ),
            Made::Assisted => format!(
                "AI-assisted composition: software (an AI assistant or claudius' generator) wrote some of the music, \
                 and the artist shaped it. {sound} Where a distributor asks whether the song includes AI-generated \
                 music, the answer is yes; where it asks for AI credits, as Spotify's do, credit AI-assisted \
                 composition."
            ),
            Made::Generated => format!(
                "AI-generated composition: software (an AI assistant or claudius' generator) wrote the music. {sound} \
                 Where a distributor asks whether the song includes AI-generated music, the answer is yes; where it \
                 asks for AI credits, as Spotify's do, credit AI-generated composition."
            ),
        }
    }
}

impl std::str::FromStr for Made {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Made> {
        Made::ALL
            .into_iter()
            .find(|m| m.name() == s.trim().to_ascii_lowercase())
            .with_context(|| format!("made how? '{s}' is not written, assisted or generated"))
    }
}

/// What stores and distributors ask about a release.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Release {
    /// the song's title when empty
    pub title: String,
    /// a version of the song, such as "Extended Mix" or "Radio Edit"
    pub version: String,
    pub artist: String,
    pub featuring: Vec<String>,
    /// the album or EP the song is on; a single when empty
    pub album: String,
    pub genre: String,
    pub subgenre: String,
    /// YYYY-MM-DD
    pub date: String,
    /// the record label: the artist when empty
    pub label: String,
    /// the songwriters' full legal names
    pub writers: Vec<String>,
    pub producers: Vec<String>,
    /// who owns the recording and the composition (the ℗ and © lines): the artist when empty
    pub owner: String,
    pub isrc: String,
    pub upc: String,
    pub explicit: bool,
    pub made: Option<Made>,
    pub artwork: Option<PathBuf>,
}

impl Release {
    pub fn title<'a>(&'a self, bp: &'a Blueprint) -> &'a str {
        if self.title.trim().is_empty() { &bp.title } else { self.title.trim() }
    }

    fn label(&self) -> &str {
        if self.label.trim().is_empty() { self.artist.trim() } else { self.label.trim() }
    }

    fn owner(&self) -> &str {
        if self.owner.trim().is_empty() { self.artist.trim() } else { self.owner.trim() }
    }
}

fn read(file: &Path) -> Option<Release> {
    std::fs::read_to_string(file).ok().and_then(|s| serde_json::from_str(&s).ok())
}

/// A song's release details: its folder's release.json, or for a song without one, its title and genre, and the
/// artist of the latest release among the songs next to it.
pub fn load(dir: &Path, bp: &Blueprint) -> Release {
    if let Some(r) = read(&dir.join(FILE)) {
        return r;
    }
    let latest = dir
        .parent()
        .and_then(|p| std::fs::read_dir(p).ok())
        .into_iter()
        .flatten()
        .filter_map(|e| {
            let file = e.ok()?.path().join(FILE);
            let modified = file.metadata().and_then(|m| m.modified()).ok()?;
            Some((modified, read(&file).filter(|r| !r.artist.trim().is_empty())?))
        })
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, r)| r)
        .unwrap_or_default();
    let (genre, subgenre) = store_genre(&bp.genre);
    Release {
        title: bp.title.clone(),
        artist: latest.artist,
        writers: latest.writers,
        producers: latest.producers,
        label: latest.label,
        owner: latest.owner,
        genre: genre.into(),
        subgenre: subgenre.into(),
        ..Default::default()
    }
}

/// Keep a song's release details in its folder.
pub fn save(dir: &Path, r: &Release) -> Result<()> {
    let file = dir.join(FILE);
    std::fs::write(&file, serde_json::to_string_pretty(r)?).with_context(|| format!("cannot write {}", file.display()))
}

/// The genre and subgenre stores list a claudius genre under.
pub fn store_genre(genre: &str) -> (&'static str, &'static str) {
    match genre {
        "house" => ("Dance", "House"),
        "deep_house" => ("Dance", "Deep House"),
        "techno" => ("Electronic", "Techno"),
        "trap" => ("Hip-Hop/Rap", "Trap"),
        "lofi" => ("Hip-Hop/Rap", "Lo-Fi Hip-Hop"),
        "dnb" => ("Electronic", "Drum & Bass"),
        "future_bass" => ("Electronic", "Future Bass"),
        "synthwave" => ("Electronic", "Synthwave"),
        _ => ("Electronic", ""),
    }
}

/// How much a check matters: problems stop a release, warnings are worth fixing, notes are worth knowing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Problem,
    Warning,
    Note,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum About {
    Artwork,
    Audio,
    Sounds,
    Details,
}

impl About {
    pub fn name(self) -> &'static str {
        match self {
            About::Artwork => "artwork",
            About::Audio => "audio",
            About::Sounds => "sounds",
            About::Details => "details",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Check {
    pub level: Level,
    pub about: About,
    pub message: String,
}

/// What a release looks like to a store.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Report {
    /// problems first, then warnings, then notes
    pub checks: Vec<Check>,
    pub artwork: Option<Artwork>,
    /// the credit lines the sounds' licences ask for
    pub credits: Vec<String>,
    pub seconds: f64,
    pub lufs: f64,
    pub true_peak_db: f32,
}

impl Report {
    /// nothing a store would turn the release down for
    pub fn ready(&self) -> bool {
        !self.checks.iter().any(|c| c.level == Level::Problem)
    }

    pub fn problems(&self) -> usize {
        self.checks.iter().filter(|c| c.level == Level::Problem).count()
    }
}

/// A rendered song: its blueprint, the project it was arranged into, and its master.
pub struct Song<'a> {
    pub bp: &'a Blueprint,
    pub project: &'a Project,
    pub master: &'a Stereo,
}

/// Check a release as stores and distributors do. `cover` is the artwork's file.
pub fn check(r: &Release, song: &Song, cover: Option<&[u8]>) -> Report {
    let mut report = Report::default();
    match cover.map(artwork::inspect) {
        None => report.checks.push(Check {
            level: Level::Problem,
            about: About::Artwork,
            message: "add the artwork: stores need a square JPEG or PNG, 3000 × 3000".into(),
        }),
        Some(Err(why)) => report.checks.push(Check {
            level: Level::Problem,
            about: About::Artwork,
            message: format!("the artwork {why}: stores take JPEG or PNG"),
        }),
        Some(Ok(a)) => {
            artwork_checks(&a, &mut report.checks);
            report.artwork = Some(a);
        }
    }
    audio_checks(song.master, &mut report);
    rights::check(&rights::used(song.project), &mut report.checks, &mut report.credits);
    details::check(r, r.title(song.bp), details::today(), &mut report.checks);
    report.checks.sort_by_key(|c| c.level);
    report
}

fn artwork_checks(a: &Artwork, out: &mut Vec<Check>) {
    let mut say = |level, message: String| out.push(Check { level, about: About::Artwork, message });
    let (w, h) = (a.width, a.height);
    let size = format!("the artwork is {w} × {h}");
    if w != h {
        say(Level::Problem, format!("{size}: stores need it square"));
    }
    match w.min(h) {
        side if side < 1400 => say(
            Level::Problem,
            format!("{size}: Apple Music needs at least 1400 × 1400, and stores ask for 3000 × 3000"),
        ),
        _ if w.max(h) > 10_000 => say(Level::Problem, format!("{size}: Spotify takes at most 10000 × 10000")),
        side if side < 3000 => say(
            Level::Warning,
            format!(
                "{size}: stores ask for 3000 × 3000{}",
                if side < 1600 { ", and TuneCore takes no less than 1600 × 1600" } else { "" }
            ),
        ),
        _ if w.max(h) > 3000 => say(
            Level::Warning,
            format!(
                "{size}: CD Baby and TuneCore take at most 3000 × 3000 (Spotify, Apple Music and DistroKid take it)"
            ),
        ),
        _ => {}
    }
    match a.colour {
        Colour::Cmyk => say(Level::Problem, "the artwork is in CMYK, for print: save it as RGB".into()),
        Colour::Grey => say(
            Level::Problem,
            "the artwork is greyscale, and stores take RGB only, for black and white art too: save it as RGB".into(),
        ),
        Colour::Palette => {
            say(Level::Warning, "the artwork has a palette of 256 colours or fewer: save it in full colour".into())
        }
        Colour::Rgb => {}
    }
    if a.bits > 8 {
        say(Level::Warning, format!("the artwork has {} bits a channel: Spotify takes 8 (24-bit colour)", a.bits));
    }
    if a.transparent {
        say(Level::Problem, "the artwork is see-through in places: stores need it opaque".into());
    } else if a.alpha {
        say(
            Level::Note,
            "the artwork has an alpha channel, though nothing in it is see-through: saving it without one is safest"
                .into(),
        );
    }
    if let Some(p) = a.profile.as_deref().filter(|_| !a.is_srgb()) {
        say(
            Level::Warning,
            format!(
                "the artwork's colours are in the {p} profile, and stores show artwork as sRGB: convert it to sRGB"
            ),
        );
    }
    if a.rotated {
        say(
            Level::Warning,
            "the artwork is stored turned, with a tag that says how to show it, which stores ignore: save it upright"
                .into(),
        );
    }
    if let Some(q) = a.quality.filter(|&q| q < 80) {
        say(
            Level::Warning,
            format!(
                "the artwork is a JPEG saved at about quality {q}, which shows blocks and smears on a large screen: \
                 save it at 90 or more, or as PNG"
            ),
        );
    }
    if let Some(d) = a.detail {
        say(
            Level::Warning,
            format!(
                "the artwork has no sharp edges: it looks enlarged from about {d} × {d}, or blurred, and stores turn \
                 down blurry or pixelated art. Use the original at full size; if the softness is the art, ignore this"
            ),
        );
    }
    if a.bytes > 10_000_000 {
        say(
            Level::Warning,
            format!("the artwork is {:.1} MB: TuneCore takes at most 10 MB, CD Baby 25", a.bytes as f64 / 1e6),
        );
    }
}

fn audio_checks(m: &Stereo, report: &mut Report) {
    report.seconds = m.secs();
    report.lufs = crate::loudness::integrated(m);
    report.true_peak_db = crate::fx::true_peak_db(m);
    let (secs, lufs, peak) = (report.seconds, report.lufs, report.true_peak_db);
    let mut say = |level, message: String| report.checks.push(Check { level, about: About::Audio, message });
    if secs < 30.0 {
        say(
            Level::Warning,
            format!(
                "the song is {secs:.0} seconds long: Spotify counts a play from 30 seconds on, so a song this short \
                 earns nothing there"
            ),
        );
    }
    let (lead, tail) = silence(m);
    if lead > 2.0 {
        say(Level::Warning, format!("the song starts with {lead:.1} s of silence: listeners expect it to start"));
    }
    if tail > 5.0 {
        say(Level::Warning, format!("the song ends with {tail:.1} s of silence"));
    }
    if peak > -1.0 {
        say(
            Level::Warning,
            format!(
                "the master peaks at {peak:+.1} dB true peak, and stores' encoders can clip a master above -1 dB: \
                 master(#{{ ceiling: -1.0 }}) keeps it under"
            ),
        );
    }
    if lufs > -14.0 {
        say(
            Level::Note,
            format!(
                "the master is {lufs:.1} LUFS: Spotify plays it {:.0} dB quieter and Apple Music {:.0} dB quieter, as \
                 they play songs at about -14 and -16 LUFS",
                lufs + 14.0,
                lufs + 16.0
            ),
        );
    }
}

/// seconds of silence (under -60 dB) before the song starts, and after it ends
fn silence(m: &Stereo) -> (f64, f64) {
    let heard = |i: &usize| m.l[*i].abs().max(m.r[*i].abs()) > 0.001;
    let n = m.len();
    let first = (0..n).find(heard).unwrap_or(n);
    let last = (0..n).rev().find(heard).map_or(first, |i| i + 1);
    let sr = crate::config::SRF;
    (first as f64 / sr, (n - last) as f64 / sr)
}

/// minutes and seconds
pub fn length(secs: f64) -> String {
    let s = secs.round() as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

/// "3000 × 3000 JPEG", with its profile when it has one
pub fn describe(a: &Artwork) -> String {
    let mut s = format!("{} × {} {}", a.width, a.height, a.format);
    if let Some(p) = &a.profile {
        s.push_str(&format!(", {p}"));
    }
    s
}

/// a name every file system takes
fn file_name(s: &str) -> String {
    let clean: String =
        s.chars()
            .map(|c| {
                if c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                    '-'
                } else {
                    c
                }
            })
            .take(150)
            .collect();
    match clean.trim_matches(|c: char| c == '.' || c.is_whitespace()) {
        "" => "song".into(),
        t => t.to_string(),
    }
}

/// what a kit holds
#[derive(Serialize)]
struct KitFile<'a> {
    release: Release,
    seconds: f64,
    lufs: f64,
    true_peak_db: f32,
    artwork: Option<&'a Artwork>,
    credits: &'a [String],
    checks: &'a [Check],
    files: Vec<String>,
    made_with: String,
}

/// The audio of a kit: the file every distributor takes, and the master for those that take more.
const KIT_AUDIO: [(&str, &str, &str); 2] = [
    ("wav:16", "", "the one to upload, which every distributor takes"),
    ("wav", " (24-bit)", "for distributors that take high-resolution audio"),
];

/// Write a release kit into `dir`: the master as distributors take it, the artwork as given, release.txt with what
/// their forms ask, and release.json with the same for programs. Returns each file with what it is for.
pub fn write_kit(
    dir: &Path,
    r: &Release,
    song: &Song,
    artwork: &[u8],
    report: &Report,
) -> Result<Vec<(PathBuf, String)>> {
    if !report.ready() {
        bail!("{} problem(s) to fix first", report.problems());
    }
    let title = r.title(song.bp).to_string();
    let version = if r.version.trim().is_empty() { String::new() } else { format!(" ({})", r.version.trim()) };
    let base = file_name(&format!("{} - {title}{version}", r.artist.trim()));
    if dir.join("blueprint.json").exists() {
        bail!("{} is a song's folder: a release kit goes in a folder of its own", dir.display());
    }
    std::fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    // the files of an earlier kit here, which a new title would leave behind
    #[derive(Deserialize)]
    struct Earlier {
        #[serde(default)]
        files: Vec<String>,
    }
    let earlier = std::fs::read_to_string(dir.join("release.json")).ok();
    for name in earlier.and_then(|s| serde_json::from_str::<Earlier>(&s).ok()).map(|e| e.files).unwrap_or_default() {
        if !name.starts_with('.') && !name.contains(['/', '\\']) {
            let _ = std::fs::remove_file(dir.join(name));
        }
    }
    let mut tags = export::tags(&title);
    tags.artist = r.artist.trim().to_string();
    tags.album = r.album.trim().to_string();
    let audio: Vec<(PathBuf, Spec, &str)> = KIT_AUDIO
        .iter()
        .map(|(spec, suffix, why)| Ok((dir.join(format!("{base}{suffix}.wav")), spec.parse()?, *why)))
        .collect::<Result<_>>()?;
    std::thread::scope(|s| {
        let (master, tags) = (song.master, &tags);
        let jobs: Vec<_> =
            audio.iter().map(|(file, spec, _)| s.spawn(move || export::write(master, spec, tags, file))).collect();
        jobs.into_iter().try_for_each(|j| j.join().unwrap_or_else(|_| Err(anyhow::anyhow!("the encoder stopped"))))
    })?;
    let mut files: Vec<(PathBuf, String)> = audio
        .iter()
        .map(|(file, spec, why)| (file.clone(), format!("{}: {why}", spec.label(crate::config::SR))))
        .collect();
    let a = report.artwork.as_ref();
    let cover_name = if a.is_some_and(|a| a.format == "PNG") { "cover.png" } else { "cover.jpg" };
    let cover = dir.join(cover_name);
    std::fs::write(&cover, artwork).with_context(|| format!("cannot write {}", cover.display()))?;
    files.push((cover, a.map(describe).unwrap_or_default()));
    let text = dir.join("release.txt");
    files.push((text.clone(), "what to enter in the distributor's form".into()));
    let json = dir.join("release.json");
    files.push((json.clone(), "the same, for programs".into()));
    let names: Vec<String> =
        files.iter().filter_map(|(f, _)| f.file_name().map(|n| n.to_string_lossy().into_owned())).collect();
    std::fs::write(&text, release_text(r, song, report, &files))
        .with_context(|| format!("cannot write {}", text.display()))?;
    let kit = KitFile {
        release: Release {
            title,
            label: r.label().into(),
            owner: r.owner().into(),
            artwork: Some(cover_name.into()),
            ..r.clone()
        },
        seconds: report.seconds,
        lufs: report.lufs,
        true_peak_db: report.true_peak_db,
        artwork: a,
        credits: &report.credits,
        checks: &report.checks,
        files: names,
        made_with: format!("claudius {}", env!("CARGO_PKG_VERSION")),
    };
    std::fs::write(&json, serde_json::to_string_pretty(&kit)?)
        .with_context(|| format!("cannot write {}", json.display()))?;
    Ok(files)
}

fn release_text(r: &Release, song: &Song, report: &Report, files: &[(PathBuf, String)]) -> String {
    let title = r.title(song.bp);
    let or = |s: &str, none: &str| if s.trim().is_empty() { none.to_string() } else { s.trim().to_string() };
    let names = |v: &[String], none: &str| {
        let v: Vec<&str> = v.iter().map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
        if v.is_empty() { none.to_string() } else { v.join(", ") }
    };
    let year = details::days(r.date.trim()).map_or_else(|| details::year(details::today()), details::year);
    let release = if r.album.trim().is_empty() {
        format!("{title}, a single")
    } else {
        format!("{}, an album or EP", r.album.trim())
    };
    let genre = [r.genre.trim(), r.subgenre.trim()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>();
    let assigned = "(leave it empty: the distributor assigns one)";
    let fields = [
        ("Title", title.to_string()),
        ("Version", or(&r.version, "(none)")),
        ("Primary artist", or(&r.artist, "")),
        ("Featuring", names(&r.featuring, "(none)")),
        ("Release", release),
        ("Genre", if genre.is_empty() { "(choose one)".into() } else { genre.join(", ") }),
        ("Release date", or(&r.date, "(choose one at least four weeks ahead)")),
        ("Record label", r.label().to_string()),
        ("Songwriters", names(&r.writers, "(their full legal names)")),
        ("Producers", names(&r.producers, "(none)")),
        ("℗ line", format!("{year} {}", r.owner())),
        ("© line", format!("{year} {}", r.owner())),
        ("ISRC", or(&r.isrc, assigned)),
        ("UPC", or(&r.upc, assigned)),
        ("Explicit lyrics", if r.explicit { "yes" } else { "no" }.into()),
        ("Lyrics language", "none (instrumental)".into()),
        ("Length", length(report.seconds)),
    ];
    let mut s = format!(
        "{} - {title}{}\n\nA release kit from claudius {}: upload the first WAV and the cover to a distributor, and \
         copy the answers below into its form.\n\nFor the form\n",
        r.artist.trim(),
        if r.version.trim().is_empty() { String::new() } else { format!(" ({})", r.version.trim()) },
        env!("CARGO_PKG_VERSION")
    );
    for (k, v) in fields {
        s.push_str(&format!("  {k:<17} {v}\n"));
    }
    s.push_str("\nAI\n  ");
    s.push_str(&r.made.map_or_else(|| "(say how the song was made)".to_string(), Made::disclosure));
    s.push('\n');
    if !report.credits.is_empty() {
        s.push_str(
            "\nCredits\n  The licences of these sounds ask for credit: give it in the release's credits or notes, \
             where the distributor has a place for them, and in descriptions on SoundCloud, YouTube or Bandcamp.\n",
        );
        for c in &report.credits {
            s.push_str(&format!("  - {c}\n"));
        }
    }
    s.push_str("\nFiles\n");
    let width = files.iter().filter_map(|(f, _)| f.file_name()).map(|n| n.len()).max().unwrap_or(0);
    for (f, what) in files {
        let name = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        s.push_str(&format!("  {name:<width$}  {what}\n"));
    }
    s.push_str(
        "\nWhat stores check in artwork, and claudius cannot\n  - the only text on it is the artist's name and the \
         title, spelled as above\n  - no web addresses, QR codes, social media names or logos, store logos, prices, \
         or \"CD\"\n  - an image you made or have a licence for: no stock photo without one\n  - not the artwork of \
         another release, and nothing blurry, pixelated, turned or cut off\n",
    );
    if !report.checks.is_empty() {
        s.push_str("\nChecks\n");
        for c in &report.checks {
            let mark = if c.level == Level::Warning { "!" } else { "-" };
            s.push_str(&format!("  {mark} {}: {}\n", c.about.name(), c.message));
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// a square picture with sharp edges, as a PNG
    fn png(side: u32, rgba: bool) -> Vec<u8> {
        let mut out = Vec::new();
        let px = |x: u32, y: u32| if (x / 50 + y / 50).is_multiple_of(2) { 30u8 } else { 220 };
        let encoder = image::codecs::png::PngEncoder::new(&mut out);
        if rgba {
            let buf: Vec<u8> = (0..side * side).flat_map(|i| [px(i % side, i / side), 90, 160, 0]).collect();
            image::ImageEncoder::write_image(encoder, &buf, side, side, image::ExtendedColorType::Rgba8).unwrap();
        } else {
            let buf: Vec<u8> = (0..side * side).flat_map(|i| [px(i % side, i / side), 90, 160]).collect();
            image::ImageEncoder::write_image(encoder, &buf, side, side, image::ExtendedColorType::Rgb8).unwrap();
        }
        out
    }

    fn checks_of(bytes: &[u8]) -> Vec<Check> {
        let a = artwork::inspect(bytes).unwrap();
        let mut out = Vec::new();
        artwork_checks(&a, &mut out);
        out
    }

    #[test]
    fn square_rgb_artwork_of_3000_passes() {
        assert_eq!(checks_of(&png(3000, false)), []);
    }

    #[test]
    fn small_wide_or_see_through_artwork_is_turned_down() {
        let small = checks_of(&png(1000, false));
        assert!(small.len() == 1 && small[0].level == Level::Problem && small[0].message.contains("1400"), "{small:?}");
        let see_through = checks_of(&png(1500, true));
        let levels: Vec<Level> = see_through.iter().map(|c| c.level).collect();
        assert_eq!(levels, [Level::Warning, Level::Problem], "{see_through:?}");
        let a = artwork::inspect(&png(1500, true)).unwrap();
        assert!(a.alpha && a.transparent);
    }

    #[test]
    fn other_formats_are_named() {
        let report_of = |bytes: &[u8]| artwork::inspect(bytes).unwrap_err();
        assert_eq!(report_of(b"RIFF\0\0\0\0WEBPVP8 "), "is a WebP file");
        assert_eq!(report_of(b"\0\0\0\x18ftypheic\0\0\0\0"), "is an HEIC file");
        assert_eq!(report_of(b"hello"), "is not an image claudius can read");
    }

    #[test]
    fn an_enlarged_picture_is_found_and_a_sharp_one_is_not() {
        // shapes with hard edges, of every contrast
        let side = 1800u32;
        let mut rng = crate::rng::Rng::new(7);
        let mut sharp = image::RgbImage::from_pixel(side, side, image::Rgb([128, 128, 128]));
        for _ in 0..600 {
            let (x, y) = (rng.int(0, i64::from(side) - 1) as u32, rng.int(0, i64::from(side) - 1) as u32);
            let (w, h) = (rng.int(8, 300) as u32, rng.int(8, 300) as u32);
            let v = rng.int(0, 255) as u8;
            for py in y..(y + h).min(side) {
                for px in x..(x + w).min(side) {
                    sharp.put_pixel(px, py, image::Rgb([v, v / 2, 255 - v]));
                }
            }
        }
        let encode = |img: &image::RgbImage| {
            let mut out = Vec::new();
            image::DynamicImage::ImageRgb8(img.clone())
                .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
                .unwrap();
            out
        };
        assert_eq!(artwork::inspect(&encode(&sharp)).unwrap().detail, None);
        for (from, filter) in
            [(side / 4, image::imageops::FilterType::CatmullRom), (side / 3, image::imageops::FilterType::Lanczos3)]
        {
            let small = image::imageops::resize(&sharp, from, from, image::imageops::FilterType::Triangle);
            let enlarged = image::imageops::resize(&small, side, side, filter);
            let d = artwork::inspect(&encode(&enlarged)).unwrap().detail;
            assert!(d.is_some_and(|d| d.abs_diff(from) <= from / 2), "enlarged from {from}: {d:?}");
        }
    }

    #[test]
    fn names_are_safe_for_files() {
        assert_eq!(file_name("AC/DC - Back: In Black?"), "AC-DC - Back- In Black-");
        assert_eq!(file_name(" ..."), "song");
    }

    #[test]
    fn made_reads_its_names() {
        for m in Made::ALL {
            assert_eq!(m.name().parse::<Made>().unwrap(), m);
        }
        assert!("by hand".parse::<Made>().is_err());
    }
}

//! Samples from free online libraries. A search asks Openverse (which indexes Freesound's sounds, Wikimedia Commons'
//! and Jamendo's), Wikimedia Commons itself and ccMixter, and keeps what each library says may be used to make and
//! publish music: CC0, a public domain mark and CC BY by default, as for packs (design record 0007). A download goes to
//! `<data dir>/claudius/samples/<source>/`, with a `.json` beside it naming the sound's author, licence and page,
//! which song credits quote.
//!
//! Recordings in the public domain by age (Commons' records from before 1926, say) are public domain in the United
//! States, but a tune on them may still belong to its composer's heirs where copyright lasts 70 years after the
//! composer's death, so they come only when asked for, as share-alike sounds do. Non-commercial and no-derivatives
//! licences never come: people publish what they make with claudius.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::fetch;

/// a library claudius searches
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// Openverse's index of Freesound, Wikimedia Commons and Jamendo
    Openverse,
    /// Wikimedia Commons' own search, for the recordings Openverse leaves out
    Commons,
    /// ccMixter: stems, a cappellas and samples shared to be remixed
    Ccmixter,
}

impl Source {
    pub const ALL: [Source; 3] = [Source::Openverse, Source::Commons, Source::Ccmixter];

    pub fn name(self) -> &'static str {
        match self {
            Source::Openverse => "openverse",
            Source::Commons => "commons",
            Source::Ccmixter => "ccmixter",
        }
    }

    pub fn parse(s: &str) -> Option<Source> {
        Source::ALL.into_iter().find(|x| x.name() == s.trim().to_ascii_lowercase())
    }
}

/// what a sound's licence lets you do, as claudius sorts the libraries' licences
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Licence {
    /// dedicated to the public domain: free everywhere
    Cc0,
    /// in the public domain: marked so by its author, or by age (see `Found::by_age`)
    PublicDomain,
    /// free, with credit
    By,
    /// free, with credit, but the music made with it must carry the same licence
    BySa,
    /// not for music people may publish: non-commercial or no derivatives
    Restricted,
    Unknown,
}

/// which licences a search keeps beyond CC0, an author's public domain mark and CC BY
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Policy {
    /// recordings in the public domain by age, which are public domain in the United States
    pub us_public_domain: bool,
    /// share-alike sounds (CC BY-SA)
    pub share_alike: bool,
}

impl Policy {
    pub fn allows(&self, f: &Found) -> bool {
        match f.licence {
            Licence::Cc0 | Licence::By => true,
            Licence::PublicDomain => !f.by_age || self.us_public_domain,
            Licence::BySa => self.share_alike,
            Licence::Restricted | Licence::Unknown => false,
        }
    }
}

/// a sound a library offers
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Found {
    /// `freesound:629052`, `commons:Swanee 1920.ogg`, `ccmixter:71182`; for Openverse's other providers
    /// `openverse:<id>`
    pub id: String,
    /// the library that found it
    pub source: Source,
    /// where the sound lives, as the library names it (`freesound`, `wikimedia_audio`, `jamendo`, `ccmixter`)
    pub provider: String,
    pub title: String,
    pub creator: String,
    pub licence: Licence,
    /// the licence as the library states it: `CC0 1.0`, `CC BY 4.0`, `Public domain`
    pub licence_name: String,
    pub licence_url: String,
    /// where a person reads about the sound
    pub page: String,
    /// the file to download
    pub audio: String,
    pub filetype: String,
    pub seconds: Option<f64>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// the year it was recorded, when the library says
    #[serde(default)]
    pub year: Option<i32>,
    /// in the public domain because it is old, not because its author said so
    #[serde(default)]
    pub by_age: bool,
}

impl Found {
    /// the credit a song that plays it gives: what, who, the licence, where
    pub fn credit(&self) -> String {
        let mut s = format!("\"{}\" by {}, {}", self.title, self.creator, self.licence_name);
        if !self.licence_url.is_empty() {
            s.push_str(&format!(" ({})", self.licence_url));
        }
        if !self.page.is_empty() {
            s.push_str(&format!(", {}", self.page));
        }
        if self.by_age {
            s.push_str(
                ". Public domain in the United States as a recording from before 1926; the tune may still be \
                        protected where copyright lasts 70 years after the composer's death",
            );
        }
        s
    }

    /// whether its licence asks for credit
    pub fn credit_required(&self) -> bool {
        matches!(self.licence, Licence::By | Licence::BySa)
    }
}

/// what to look for
#[derive(Clone, Debug)]
pub struct Query {
    pub text: String,
    /// the libraries to ask; none: all of them
    pub sources: Vec<Source>,
    pub policy: Policy,
    /// how many sounds at most, from all the libraries together
    pub limit: usize,
}

/// what a search found, and the libraries that did not answer
#[derive(Debug, Default)]
pub struct Searched {
    pub found: Vec<Found>,
    pub failed: Vec<(Source, String)>,
}

/// Search the libraries for `q.text`. Sounds their licences keep out, uploads that say they are cut from records
/// (`suspect`) and papers' supplementary recordings (`supplement`) are left out; the rest come interleaved by library,
/// and are remembered for `get`.
pub fn search(q: &Query) -> Result<Searched> {
    let text = q.text.trim();
    if text.is_empty() {
        bail!("search for something: a description such as \"drum break\" or \"vinyl crackle\"");
    }
    let sources = if q.sources.is_empty() { Source::ALL.to_vec() } else { q.sources.clone() };
    let per = q.limit.clamp(1, 20);
    let mut out = Searched::default();
    let mut lists: Vec<Vec<Found>> = Vec::new();
    for s in sources {
        let got = match s {
            Source::Openverse => fetch::get_json(&openverse_url(text, &q.policy, per)).map(|v| openverse_results(&v)),
            Source::Commons => fetch::get_json(&commons_url(text, per)).map(|v| commons_results(&v)),
            Source::Ccmixter => fetch::get_json(&ccmixter_url(text, &q.policy, per)).map(|v| ccmixter_results(&v)),
        };
        match got {
            Ok(list) => {
                lists.push(list.into_iter().filter(|f| q.policy.allows(f) && !suspect(f) && !supplement(f)).collect())
            }
            Err(e) => out.failed.push((s, format!("{e:#}"))),
        }
    }
    let mut seen = std::collections::HashSet::new();
    for i in 0.. {
        if out.found.len() >= q.limit || lists.iter().all(|l| i >= l.len()) {
            break;
        }
        for l in &lists {
            if let Some(f) = l.get(i)
                && out.found.len() < q.limit
                && seen.insert(f.audio.clone())
                && seen.insert(f.id.clone())
            {
                out.found.push(f.clone());
            }
        }
    }
    remember(&out.found);
    Ok(out)
}

/// Uploads whose own words say they were cut from someone's record: a library keeps the licence its uploader chose,
/// and an uploader cannot give away a record they do not own.
pub fn suspect(f: &Found) -> bool {
    const SAYS: &[&str] = &[
        "sampled from",
        "sample from",
        "taken from",
        "from the song",
        "from the album",
        "from the record",
        "vinyl rip",
        "record rip",
        "70s sample",
        "80s sample",
        "60s sample",
        "copyrighted",
        "©",
    ];
    let words = format!("{} {}", f.title, f.tags.join(" ")).to_lowercase();
    SAYS.iter().any(|s| words.contains(s))
}

/// The files of a paper's supplementary material, which a bot imports to Wikimedia Commons under the names their
/// journals give them: PLOS's `…-pone.0062397.s008.oga`, Springer's and BMC's `…_MOESM1_ESM`, Scientific Reports'
/// `srep01234-s2`, Frontiers' `fpsyg-05-00123-s001`. A voice in one may hold a note, but it is no music.
pub fn supplement(f: &Found) -> bool {
    let name = format!("{} {}", f.id, f.title).to_lowercase();
    if name.contains("supplementary") {
        return true;
    }
    // words as a file's name or its title on Commons has them, which puts spaces for its underscores
    let words: Vec<&str> = name.split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| !w.is_empty()).collect();
    let digits = |w: &str, n: usize| w.len() == n && w.bytes().all(|b| b.is_ascii_digit());
    let numbered = |w: &str| w.len() >= 2 && w.starts_with('s') && w[1..].bytes().all(|b| b.is_ascii_digit());
    let springer = words.windows(2).any(|w| {
        w[0].len() > 5 && w[0].starts_with("moesm") && w[0][5..].bytes().all(|b| b.is_ascii_digit()) && w[1] == "esm"
    });
    const PLOS: [&str; 7] = ["pone", "pbio", "pcbi", "pgen", "ppat", "pmed", "pntd"];
    let plos = words.windows(3).any(|w| PLOS.contains(&w[0]) && digits(w[1], 7) && numbered(w[2]));
    let srep = words.windows(2).any(|w| {
        w[0].len() > 4 && w[0].starts_with("srep") && w[0][4..].bytes().all(|b| b.is_ascii_digit()) && numbered(w[1])
    });
    let frontiers = words.windows(4).any(|w| {
        w[0].len() >= 4
            && w[0].bytes().all(|b| b.is_ascii_lowercase())
            && digits(w[1], 2)
            && digits(w[2], 5)
            && numbered(w[3])
    });
    springer || plos || srep || frontiers
}

fn enc(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn openverse_url(text: &str, p: &Policy, per: usize) -> String {
    let mut licences = vec!["cc0", "pdm", "by"];
    if p.share_alike {
        licences.push("by-sa");
    }
    format!(
        "https://api.openverse.org/v1/audio/?q={}&license={}&page_size={per}&mature=false",
        enc(text),
        licences.join(",")
    )
}

fn commons_url(text: &str, per: usize) -> String {
    format!(
        "https://commons.wikimedia.org/w/api.php?action=query&format=json&generator=search&gsrnamespace=6\
         &gsrsearch={}&gsrlimit={per}&prop=imageinfo&iiprop=url%7Cmime%7Cextmetadata\
         &iiextmetadatafilter=LicenseShortName%7CLicenseUrl%7CArtist%7CDateTimeOriginal%7CObjectName",
        enc(&format!("filetype:audio -filemime:audio/midi {text}"))
    )
}

fn ccmixter_url(text: &str, p: &Policy, per: usize) -> String {
    // ccMixter's own codes, a list being any of them: `by`, `zero` (CC0) and `sa` (BY-SA); it refuses `by-sa`
    let lic = if p.share_alike { "by,zero,sa" } else { "by,zero" };
    format!("https://ccmixter.org/api/query?f=json&search={}&lic={lic}&limit={per}&sort=rank", enc(text))
}

/// a licence from a Creative Commons URL or code: `cc0`, `pdm`, `by`, `by-sa`, `by-nc` ...
fn licence_of(code_or_url: &str) -> Licence {
    let s = code_or_url.to_ascii_lowercase();
    let restricted = ["by-nc", "nc-sa", "nc-nd", "by-nd", "nc-sampling"].iter().any(|k| s.contains(k));
    if restricted || s == "nc" || s == "nd" {
        return Licence::Restricted;
    }
    if s == "cc0" || s.contains("publicdomain/zero") || s.starts_with("cc0") {
        Licence::Cc0
    } else if s == "pdm" || s.contains("publicdomain/mark") || s.starts_with("public domain") || s == "pd" {
        Licence::PublicDomain
    } else if s == "by-sa" || s.contains("licenses/by-sa/") || s.starts_with("cc by-sa") {
        Licence::BySa
    } else if s == "by" || s.contains("licenses/by/") || s.starts_with("cc by ") || s == "cc by" {
        Licence::By
    } else if s.contains("sampling") {
        // ccMixter's Sampling+ forbids commercial use of the whole work
        Licence::Restricted
    } else {
        Licence::Unknown
    }
}

fn licence_name(l: Licence, version: &str) -> String {
    let v = if version.is_empty() { String::new() } else { format!(" {version}") };
    match l {
        Licence::Cc0 => format!("CC0{v}"),
        Licence::PublicDomain => "Public domain".into(),
        Licence::By => format!("CC BY{v}"),
        Licence::BySa => format!("CC BY-SA{v}"),
        Licence::Restricted => "a restricted licence".into(),
        Licence::Unknown => "an unknown licence".into(),
    }
}

fn str_at<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

/// the number in a Freesound sound's page (`https://freesound.org/people/x/sounds/629052/`)
fn freesound_number(page: &str) -> Option<String> {
    let rest = page.split("/sounds/").nth(1)?;
    let n: String = rest.chars().take_while(char::is_ascii_digit).collect();
    (!n.is_empty()).then_some(n)
}

/// a Commons file's name from its page (`https://commons.wikimedia.org/wiki/File:Swanee_1920.ogg`)
fn commons_name(page: &str) -> Option<String> {
    let name = page.split("/wiki/File:").nth(1)?;
    Some(name.replace('_', " "))
}

/// Openverse's audio results
pub fn openverse_results(v: &Value) -> Vec<Found> {
    let Some(results) = v.get("results").and_then(Value::as_array) else { return Vec::new() };
    results
        .iter()
        .filter_map(|r| {
            let provider = str_at(r, "source").to_string();
            let page = str_at(r, "foreign_landing_url").to_string();
            let code = str_at(r, "license");
            let licence = licence_of(code);
            let id = match provider.as_str() {
                "freesound" => freesound_number(&page).map(|n| format!("freesound:{n}")),
                "wikimedia_audio" => commons_name(&page).map(|n| format!("commons:{n}")),
                _ => None,
            }
            .unwrap_or_else(|| format!("openverse:{}", str_at(r, "id")));
            // Commons' files without the tracking Openverse adds to their links
            let audio = str_at(r, "url");
            let audio =
                if audio.contains("upload.wikimedia.org") { audio.split('?').next().unwrap_or(audio) } else { audio };
            (!audio.is_empty()).then(|| Found {
                id,
                source: Source::Openverse,
                by_age: licence == Licence::PublicDomain && provider == "wikimedia_audio",
                provider,
                title: non_empty(str_at(r, "title"), "untitled"),
                creator: non_empty(str_at(r, "creator"), "an unknown author"),
                licence,
                licence_name: licence_name(licence, str_at(r, "license_version")),
                licence_url: str_at(r, "license_url").to_string(),
                page,
                audio: audio.to_string(),
                filetype: non_empty(str_at(r, "filetype"), "mp3"),
                seconds: r.get("duration").and_then(Value::as_f64).map(|ms| ms / 1000.0),
                tags: r
                    .get("tags")
                    .and_then(Value::as_array)
                    .map(|t| {
                        t.iter().filter_map(|x| x.get("name").and_then(Value::as_str)).map(str::to_string).collect()
                    })
                    .unwrap_or_default(),
                year: None,
            })
        })
        .collect()
}

fn non_empty(s: &str, or: &str) -> String {
    let s = s.trim();
    if s.is_empty() { or.to_string() } else { s.to_string() }
}

/// text without HTML tags or entities, as Commons gives authors (`<a href=...>Paul Whiteman</a>`)
fn plain(html: &str) -> String {
    let mut out = String::new();
    let mut tag = false;
    for c in html.chars() {
        match c {
            '<' => tag = true,
            '>' => tag = false,
            c if !tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Wikimedia Commons' file search results
pub fn commons_results(v: &Value) -> Vec<Found> {
    let Some(pages) = v.get("query").and_then(|q| q.get("pages")).and_then(Value::as_object) else {
        return Vec::new();
    };
    let mut list: Vec<(i64, Found)> = pages
        .values()
        .filter_map(|p| {
            let info = p.get("imageinfo")?.get(0)?;
            let meta = info.get("extmetadata")?;
            let field = |k: &str| meta.get(k).and_then(|m| m.get("value")).and_then(Value::as_str).unwrap_or("");
            let mime = str_at(info, "mime");
            if mime.contains("midi") {
                return None;
            }
            let short = field("LicenseShortName");
            let licence = licence_of(short);
            let name = str_at(p, "title").trim_start_matches("File:").to_string();
            let year = field("DateTimeOriginal")
                .split(|c: char| !c.is_ascii_digit())
                .find(|w| w.len() == 4)
                .and_then(|w| w.parse::<i32>().ok());
            let filetype = name.rsplit('.').next().unwrap_or("ogg").to_ascii_lowercase();
            Some((
                p.get("index").and_then(Value::as_i64).unwrap_or(0),
                Found {
                    id: format!("commons:{name}"),
                    source: Source::Commons,
                    provider: "wikimedia_audio".into(),
                    title: non_empty(&plain(field("ObjectName")), name.rsplit_once('.').map_or(&name, |(a, _)| a)),
                    creator: non_empty(&plain(field("Artist")), "an unknown author"),
                    licence,
                    licence_name: non_empty(short, &licence_name(licence, "")),
                    licence_url: field("LicenseUrl").to_string(),
                    page: str_at(info, "descriptionurl").to_string(),
                    audio: str_at(info, "url").to_string(),
                    filetype,
                    seconds: None,
                    tags: Vec::new(),
                    year,
                    by_age: licence == Licence::PublicDomain,
                },
            ))
        })
        .filter(|(_, f)| !f.audio.is_empty())
        .collect();
    list.sort_by_key(|(i, _)| *i);
    list.into_iter().map(|(_, f)| f).collect()
}

/// `4:29` in seconds
fn clock(s: &str) -> Option<f64> {
    let mut secs = 0.0;
    for part in s.split(':') {
        secs = secs * 60.0 + part.trim().parse::<f64>().ok()?;
    }
    Some(secs)
}

/// ccMixter's query results
pub fn ccmixter_results(v: &Value) -> Vec<Found> {
    let Some(list) = v.as_array() else { return Vec::new() };
    list.iter()
        .filter_map(|u| {
            let file = u.get("files")?.as_array()?.iter().find(|f| {
                let ext = f.get("file_format_info").map(|i| str_at(i, "default-ext")).unwrap_or("");
                matches!(ext, "mp3" | "ogg" | "flac" | "wav")
            })?;
            let url = str_at(u, "license_url");
            let licence = licence_of(url);
            let version = url.trim_end_matches('/').rsplit('/').next().filter(|v| v.contains('.')).unwrap_or("");
            let page = str_at(u, "file_page_url").to_string();
            let number = page.trim_end_matches('/').rsplit('/').next().unwrap_or("").to_string();
            Some(Found {
                id: format!("ccmixter:{number}"),
                source: Source::Ccmixter,
                provider: "ccmixter".into(),
                title: non_empty(str_at(u, "upload_name"), "untitled"),
                creator: non_empty(
                    str_at(u, "user_real_name"),
                    &non_empty(str_at(u, "user_name"), "an unknown author"),
                ),
                licence,
                licence_name: licence_name(licence, version),
                licence_url: url.to_string(),
                page,
                audio: str_at(file, "download_url").to_string(),
                filetype: file.get("file_format_info").map(|i| str_at(i, "default-ext")).unwrap_or("mp3").to_string(),
                seconds: file.get("file_format_info").and_then(|i| clock(str_at(i, "ps"))),
                tags: str_at(u, "upload_tags")
                    .split(',')
                    .map(str::trim)
                    .filter(|t| !t.is_empty())
                    .map(str::to_string)
                    .collect(),
                year: None,
                by_age: false,
            })
        })
        .filter(|f| !f.audio.is_empty() && f.id != "ccmixter:")
        .collect()
}

/// Where downloaded samples live: `$CLAUDIUS_SAMPLES`, else `<data dir>/claudius/samples`.
pub fn samples_dir() -> PathBuf {
    if let Ok(p) = std::env::var("CLAUDIUS_SAMPLES")
        && !p.trim().is_empty()
    {
        return PathBuf::from(p);
    }
    dirs::data_dir().unwrap_or_else(crate::temp_dir).join("claudius").join("samples")
}

fn searches_file() -> PathBuf {
    samples_dir().join("searches.json")
}

/// keep what a search found, so `get` can name a sound by its id
fn remember(found: &[Found]) {
    if found.is_empty() {
        return;
    }
    let mut known: BTreeMap<String, Found> =
        std::fs::read_to_string(searches_file()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    for f in found {
        known.insert(f.id.clone(), f.clone());
    }
    if let Some(dir) = searches_file().parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(s) = serde_json::to_string(&known) {
        let _ = std::fs::write(searches_file(), s);
    }
}

/// a sound an earlier search found, by its id
pub fn recall(id: &str) -> Option<Found> {
    let known: BTreeMap<String, Found> = serde_json::from_str(&std::fs::read_to_string(searches_file()).ok()?).ok()?;
    known.get(id).cloned()
}

/// a file name from an id: letters, digits, `-`, `_` and `.`, the rest as `_`
fn file_stem(id: &str) -> String {
    let s: String = id
        .split_once(':')
        .map_or(id, |(_, rest)| rest)
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || "-_.".contains(c) { c } else { '_' })
        .collect();
    let s = s.trim_matches(|c| c == '.' || c == '_').to_string();
    let s = s.rsplit_once('.').map_or(s.clone(), |(a, _)| a.to_string());
    s.chars().take(80).collect()
}

/// where a sound downloads to: `<samples>/<provider>/<id>.<ext>`
pub fn path_for(f: &Found) -> PathBuf {
    let ext = match f.filetype.as_str() {
        "oga" | "ogg" | "opus" => f.filetype.as_str(),
        "flac" | "wav" | "mp3" => f.filetype.as_str(),
        _ => "mp3",
    };
    let provider = f.id.split_once(':').map_or("other", |(p, _)| p);
    samples_dir().join(provider).join(format!("{}.{ext}", file_stem(&f.id)))
}

/// the `.json` beside a downloaded sound
fn meta_path(audio: &Path) -> PathBuf {
    let mut s = audio.as_os_str().to_owned();
    s.push(".json");
    PathBuf::from(s)
}

/// Download a sound into the samples folder, with its credit beside it, unless it is there already, and check that it
/// decodes. Its licence must be one `policy` keeps.
pub fn fetch(f: &Found, policy: &Policy) -> Result<PathBuf> {
    if !policy.allows(f) {
        bail!(
            "{} is under {}, which {}",
            f.id,
            f.licence_name,
            match f.licence {
                Licence::PublicDomain => "is public domain in the United States only: ask for --us-public-domain",
                Licence::BySa => "makes the music share-alike too: ask for --share-alike",
                _ => "does not allow publishing music made with it",
            }
        );
    }
    let path = path_for(f);
    if path.is_file() && meta_path(&path).is_file() {
        return Ok(path);
    }
    let got = if f.provider == "ccmixter" {
        fetch::download_file_from(&f.audio, &f.page, &path)
    } else {
        fetch::download_file(&f.audio, &path, 0, false)
    };
    got.with_context(|| format!("downloading {}", f.audio))?;
    if let Err(e) = claudius_decode::decode(&path) {
        let _ = std::fs::remove_file(&path);
        return Err(anyhow!("{} downloaded but does not decode: {e:#}", f.id));
    }
    std::fs::write(meta_path(&path), serde_json::to_string_pretty(f)?)
        .with_context(|| format!("writing {}", meta_path(&path).display()))?;
    Ok(path)
}

/// Put a sound's credit beside a file made from it (a loop or a hit cut out of it), so that a song that plays the file
/// credits the sound.
pub fn write_credit(file: &Path, f: &Found) -> Result<()> {
    std::fs::write(meta_path(file), serde_json::to_string_pretty(f)?)
        .with_context(|| format!("writing {}", meta_path(file).display()))
}

/// every downloaded sound, with what its `.json` says
pub fn fetched() -> Vec<(PathBuf, Found)> {
    let mut out = Vec::new();
    let Ok(dirs) = std::fs::read_dir(samples_dir()) else { return out };
    for d in dirs.flatten().filter(|d| d.path().is_dir()) {
        let Ok(files) = std::fs::read_dir(d.path()) else { continue };
        for f in files.flatten() {
            let p = f.path();
            if p.extension().is_some_and(|e| e == "json") {
                continue;
            }
            if let Some(found) = about(&p) {
                out.push((p, found));
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// what the `.json` beside a downloaded sound says about it
pub fn about(audio: &Path) -> Option<Found> {
    serde_json::from_str(&std::fs::read_to_string(meta_path(audio)).ok()?).ok()
}

/// Credits for the downloaded sounds among `paths` (only what a song actually played): those whose licence asks for
/// credit first, then the rest with thanks.
pub fn credits_for_files(paths: &[&Path]) -> String {
    credits_of(&paths.iter().filter_map(|p| about(p)).collect::<Vec<_>>())
}

/// the credits for the recordings `found`, as a song's credits.txt lists them: each recording once, those whose
/// licence asks for credit first (empty: none)
pub fn credits_of(found: &[Found]) -> String {
    let mut seen = std::collections::BTreeSet::new();
    let mut required = Vec::new();
    let mut thanks = Vec::new();
    for f in found {
        if !seen.insert(f.id.clone()) {
            continue;
        }
        if f.credit_required() || f.by_age {
            required.push(f.credit());
        } else {
            thanks.push(f.credit());
        }
    }
    if required.is_empty() && thanks.is_empty() {
        return String::new();
    }
    let mut s = String::from("Samples:\n");
    for line in required.iter().chain(&thanks) {
        s.push_str(&format!("{line}\n"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn openverse_fixture() -> Value {
        serde_json::json!({"result_count": 3, "results": [
            {"id": "a1", "title": "Crunchy Drum Break 2 [128bpm]", "creator": "deadrobotmusic", "source": "freesound",
             "license": "cc0", "license_version": "1.0", "license_url": "https://creativecommons.org/publicdomain/zero/1.0/",
             "foreign_landing_url": "https://freesound.org/people/deadrobotmusic/sounds/629052/",
             "url": "https://cdn.freesound.org/previews/629/629052_11532701-hq.mp3", "filetype": "mp3", "duration": 15000,
             "tags": [{"name": "break"}, {"name": "drums"}]},
            {"id": "b2", "title": "vinyl crackles loop", "creator": "Jay_You", "source": "freesound", "license": "by",
             "license_version": "4.0", "license_url": "https://creativecommons.org/licenses/by/4.0/",
             "foreign_landing_url": "https://freesound.org/people/Jay_You/sounds/383762/",
             "url": "https://cdn.freesound.org/previews/383/383762_6300624-hq.mp3", "filetype": "mp3", "duration": 30000},
            {"id": "c3", "title": "Swanee", "creator": "Al Jolson", "source": "wikimedia_audio", "license": "pdm",
             "license_version": "1.0", "license_url": "https://creativecommons.org/publicdomain/mark/1.0/",
             "foreign_landing_url": "https://commons.wikimedia.org/wiki/File:Swanee_1920.ogg",
             "url": "https://upload.wikimedia.org/wikipedia/commons/a/aa/Swanee_1920.ogg", "filetype": "ogg"}
        ]})
    }

    #[test]
    fn openverse_results_name_each_sound_by_its_library_and_say_who_made_it_and_how_it_may_be_used() {
        let got = openverse_results(&openverse_fixture());
        assert_eq!(got.len(), 3);
        assert_eq!(got[0].id, "freesound:629052");
        assert_eq!(got[0].licence, Licence::Cc0);
        assert_eq!(got[0].licence_name, "CC0 1.0");
        assert_eq!(got[0].seconds, Some(15.0));
        assert_eq!(got[0].tags, ["break", "drums"]);
        assert_eq!(got[1].licence, Licence::By);
        assert!(got[1].credit_required());
        assert_eq!(
            got[1].credit(),
            "\"vinyl crackles loop\" by Jay_You, CC BY 4.0 (https://creativecommons.org/licenses/by/4.0/), \
             https://freesound.org/people/Jay_You/sounds/383762/"
        );
        // an old record in the public domain by age comes only when asked for
        assert_eq!(got[2].id, "commons:Swanee 1920.ogg");
        assert!(got[2].by_age);
        let p = Policy::default();
        assert!(p.allows(&got[0]) && p.allows(&got[1]) && !p.allows(&got[2]));
        assert!(Policy { us_public_domain: true, ..Policy::default() }.allows(&got[2]));
    }

    #[test]
    fn commons_results_keep_the_licence_and_year_and_drop_midi() {
        let v = serde_json::json!({"query": {"pages": {
            "1": {"index": 2, "title": "File:Green Brothers Hunkatin (1920).oga", "imageinfo": [{
                "url": "https://upload.wikimedia.org/x/Hunkatin.oga", "mime": "audio/ogg",
                "descriptionurl": "https://commons.wikimedia.org/wiki/File:Hunkatin.oga",
                "extmetadata": {"LicenseShortName": {"value": "Public domain"}, "DateTimeOriginal": {"value": "1920-04"},
                                "Artist": {"value": "<a href=\"x\">Green Brothers&#39; Novelty Band</a>"}}}]},
            "2": {"index": 1, "title": "File:Cello suite.ogg", "imageinfo": [{
                "url": "https://upload.wikimedia.org/x/Cello.ogg", "mime": "application/ogg",
                "descriptionurl": "https://commons.wikimedia.org/wiki/File:Cello.ogg",
                "extmetadata": {"LicenseShortName": {"value": "CC BY-SA 3.0"},
                                "LicenseUrl": {"value": "https://creativecommons.org/licenses/by-sa/3.0"}}}]},
            "3": {"index": 3, "title": "File:Deva Jazz.mid", "imageinfo": [{
                "url": "https://upload.wikimedia.org/x/Deva.mid", "mime": "audio/midi", "extmetadata": {}}]}
        }}});
        let got = commons_results(&v);
        assert_eq!(got.len(), 2, "{got:?}");
        assert_eq!(got[0].title, "Cello suite");
        assert_eq!(got[0].licence, Licence::BySa);
        assert_eq!(got[1].creator, "Green Brothers' Novelty Band");
        assert_eq!(got[1].year, Some(1920));
        assert!(got[1].by_age && got[1].licence == Licence::PublicDomain);
        assert_eq!(got[1].filetype, "oga");
        let p = Policy::default();
        assert!(!p.allows(&got[0]) && !p.allows(&got[1]));
        assert!(Policy { share_alike: true, us_public_domain: true }.allows(&got[0]));
    }

    #[test]
    fn ccmixter_results_keep_cc_by_and_leave_out_non_commercial_and_sampling_plus() {
        let v = serde_json::json!([
            {"upload_name": "Lost Roamin'", "user_name": "speck", "license_url": "http://creativecommons.org/licenses/by/3.0/",
             "file_page_url": "https://ccmixter.org/files/speck/71077", "upload_tags": ",media,bpm_080_085,instrumental,",
             "files": [{"download_url": "https://ccmixter.org/content/speck/speck_-_Lost_Roamin_1.mp3",
                        "file_format_info": {"default-ext": "mp3", "ps": "3:26"}}]},
            {"upload_name": "Raspberries", "user_name": "m", "license_url": "http://creativecommons.org/licenses/by-nc/4.0/",
             "file_page_url": "https://ccmixter.org/files/m/1", "files": [{"download_url": "https://x/y.mp3",
             "file_format_info": {"default-ext": "mp3", "ps": "1:00"}}]},
            {"upload_name": "Plus", "user_name": "p", "license_url": "http://creativecommons.org/licenses/sampling+/1.0/",
             "file_page_url": "https://ccmixter.org/files/p/2", "files": [{"download_url": "https://x/z.mp3",
             "file_format_info": {"default-ext": "mp3", "ps": "1:00"}}]}
        ]);
        let got = ccmixter_results(&v);
        assert_eq!(got.len(), 3);
        assert_eq!(got[0].id, "ccmixter:71077");
        assert_eq!(got[0].licence, Licence::By);
        assert_eq!(got[0].licence_name, "CC BY 3.0");
        assert_eq!(got[0].seconds, Some(206.0));
        assert!(got[0].tags.contains(&"bpm_080_085".to_string()));
        assert_eq!(got[1].licence, Licence::Restricted);
        assert_eq!(got[2].licence, Licence::Restricted);
        let p = Policy::default();
        assert!(p.allows(&got[0]) && !p.allows(&got[1]) && !p.allows(&got[2]));
    }

    #[test]
    fn ccmixter_is_asked_for_cc_by_and_cc0_and_share_alike_when_wanted() {
        let url = ccmixter_url("dark piano", &Policy::default(), 20);
        assert!(url.contains("&lic=by,zero&"), "{url}");
        let url = ccmixter_url("dark piano", &Policy { share_alike: true, us_public_domain: false }, 20);
        assert!(url.contains("&lic=by,zero,sa&"), "{url}");
        let zero = serde_json::json!([{"upload_name": "Zero", "user_name": "z", "license_url": "http://creativecommons.org/publicdomain/zero/1.0/",
             "file_page_url": "https://ccmixter.org/files/z/3", "files": [{"download_url": "https://x/w.mp3",
             "file_format_info": {"default-ext": "mp3", "ps": "1:00"}}]}]);
        let got = ccmixter_results(&zero);
        assert_eq!(got[0].licence, Licence::Cc0);
        assert!(Policy::default().allows(&got[0]));
    }

    #[test]
    fn uploads_that_say_they_are_cut_from_records_are_left_out() {
        let mut f = openverse_results(&openverse_fixture()).remove(0);
        assert!(!suspect(&f));
        f.title = "70s SAMPLE w/ drum break".into();
        assert!(suspect(&f));
        f.title = "funky loop".into();
        f.tags = vec!["sampled from".into()];
        assert!(suspect(&f));
    }

    #[test]
    fn a_papers_supplementary_recordings_are_no_music() {
        let mut f = openverse_results(&openverse_fixture()).remove(2);
        assert!(!supplement(&f), "{}", f.title);
        let named = |f: &mut Found, id: &str| {
            f.id = format!("commons:{id}");
            f.title = id.rsplit_once('.').map_or(id, |(t, _)| t).to_string();
        };
        for id in [
            "Human-Vocal-Attractiveness-as-Signaled-by-Body-Size-Projection-pone.0062397.s008.oga",
            "Birdsong-Learning-pbio.1001234.s012.ogg",
            "12868_2015_123_MOESM1_ESM.oga",
            "12868 2015 123 MOESM1 ESM.oga",
            "Human Vocal Attractiveness pone.0062397.s008.oga",
            "srep01234-s2.oga",
            "fpsyg-05-00123-s001.oga",
            "Supplementary audio 3.ogg",
        ] {
            named(&mut f, id);
            assert!(supplement(&f), "{id}");
        }
        // music, whatever its numbers
        for id in
            ["Clean Soul (ISRC USUAN1300033).mp3", "Track 05 - 1999 mix.ogg", "Pone song s1.ogg", "Gymnopedie No.1.ogg"]
        {
            named(&mut f, id);
            assert!(!supplement(&f), "{id}");
        }
    }

    #[test]
    fn licences_sort_as_claudius_reads_them() {
        assert_eq!(licence_of("cc0"), Licence::Cc0);
        assert_eq!(licence_of("https://creativecommons.org/publicdomain/zero/1.0/"), Licence::Cc0);
        assert_eq!(licence_of("pdm"), Licence::PublicDomain);
        assert_eq!(licence_of("Public domain"), Licence::PublicDomain);
        assert_eq!(licence_of("by"), Licence::By);
        assert_eq!(licence_of("CC BY 4.0"), Licence::By);
        assert_eq!(licence_of("CC BY-SA 3.0"), Licence::BySa);
        assert_eq!(licence_of("by-nc"), Licence::Restricted);
        assert_eq!(licence_of("http://creativecommons.org/licenses/by-nc-sa/3.0/"), Licence::Restricted);
        assert_eq!(licence_of("http://creativecommons.org/licenses/by-nd/4.0/"), Licence::Restricted);
        assert_eq!(licence_of("GFDL"), Licence::Unknown);
    }

    #[test]
    fn a_sound_downloads_to_a_file_named_by_its_library_and_id() {
        let f = openverse_results(&openverse_fixture()).remove(2);
        let p = path_for(&f);
        assert!(p.ends_with("commons/Swanee_1920.ogg"), "{}", p.display());
        let g = openverse_results(&openverse_fixture()).remove(0);
        assert!(path_for(&g).ends_with("freesound/629052.mp3"), "{}", path_for(&g).display());
    }

    #[test]
    fn credits_name_what_a_song_played_and_say_where_an_old_record_is_free() {
        let dir = tempfile::tempdir().unwrap();
        let mut f = openverse_results(&openverse_fixture()).remove(2);
        f.by_age = true;
        let audio = dir.path().join("Swanee_1920.ogg");
        std::fs::write(&audio, b"x").unwrap();
        std::fs::write(meta_path(&audio), serde_json::to_string(&f).unwrap()).unwrap();
        let credits = credits_for_files(&[audio.as_path(), Path::new("/nowhere.wav")]);
        assert!(credits.starts_with("Samples:\n\"Swanee\" by Al Jolson, Public domain"), "{credits}");
        assert!(credits.contains("Public domain in the United States as a recording from before 1926"), "{credits}");
    }
}

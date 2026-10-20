//! Where a song's recorded sounds come from, and what their licences let a release do.

use std::path::Path;

use claudius_packs::PackInfo;

use super::{About, Check, Level};
use crate::model::{Project, Sound};

/// where a recorded sound comes from
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Origin {
    Pack(Box<PackInfo>),
    /// a pack this machine does not have
    Missing(String),
    AppleLoops,
    /// a recording of the person's own, or a cut from one
    Own,
}

/// the tracks that play sounds of one origin, and what those sounds are
pub(super) struct Used {
    pub origin: Origin,
    pub tracks: Vec<String>,
    pub sounds: Vec<String>,
}

/// what the recorded sounds of a project are, by where they come from
pub(super) fn used(p: &Project) -> Vec<Used> {
    let packs = claudius_packs::packs();
    let packs_dir = claudius_packs::packs_dir();
    let home_loops = dirs::home_dir().map(|h| h.join("Library/Audio/Apple Loops"));
    let mut out: Vec<Used> = Vec::new();
    for t in &p.tracks {
        let (origin, sound) = match &t.sound {
            Sound::Instrument { name, .. } => (pack(name.split(':').next().unwrap_or(name), &packs), None),
            Sound::Sample { path, label, .. } | Sound::Loop { path, label, .. } => {
                let file = Path::new(path);
                let origin = if let Ok(rest) = file.strip_prefix(&packs_dir) {
                    rest.components().next().map_or(Origin::Own, |c| pack(&c.as_os_str().to_string_lossy(), &packs))
                } else if crate::config::APPLE_LOOP_DIRS.iter().any(|d| file.starts_with(d))
                    || home_loops.as_ref().is_some_and(|h| file.starts_with(h))
                    // drum hits claudius cut from Apple Loops are kept in its cache, and say so
                    || label.ends_with("Apple Loops)")
                {
                    Origin::AppleLoops
                } else {
                    Origin::Own
                };
                let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                (origin, Some(if label.is_empty() { name } else { label.clone() }))
            }
            _ => continue,
        };
        let i = match out.iter().position(|u| u.origin == origin) {
            Some(i) => i,
            None => {
                out.push(Used { origin, tracks: Vec::new(), sounds: Vec::new() });
                out.len() - 1
            }
        };
        let u = &mut out[i];
        if !u.tracks.contains(&t.name) {
            u.tracks.push(t.name.clone());
        }
        if let Some(s) = sound.filter(|s| !u.sounds.contains(s)) {
            u.sounds.push(s);
        }
    }
    out
}

fn pack(name: &str, packs: &[PackInfo]) -> Origin {
    packs
        .iter()
        .find(|p| p.name == name)
        .map_or_else(|| Origin::Missing(name.to_string()), |p| Origin::Pack(Box::new(p.clone())))
}

/// what a licence lets a song made with its sounds do
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Terms {
    Free,
    /// free, with credit
    Credit,
    /// a song made with them must be shared under the same licence
    ShareAlike,
    NonCommercial,
    NoDerivatives,
    Unknown,
    /// a user pack's own wording, which claudius cannot read
    Unread,
}

pub(super) fn terms(p: &PackInfo) -> Terms {
    let licence = p.licence.trim();
    if licence.is_empty() || licence.eq_ignore_ascii_case("unknown") {
        return Terms::Unknown;
    }
    let code = licence.to_ascii_uppercase().replace([' ', '_'], "-");
    if code.starts_with("CC-BY") {
        let parts: Vec<&str> = code.split('-').collect();
        return match () {
            _ if parts.contains(&"ND") => Terms::NoDerivatives,
            _ if parts.contains(&"NC") => Terms::NonCommercial,
            _ if parts.contains(&"SA") => Terms::ShareAlike,
            _ => Terms::Credit,
        };
    }
    // the curated packs' own wordings were all read when they were chosen: every one allows music made with them
    if code.starts_with("CC0") || code == "UNLICENSE" || code.contains("PUBLIC-DOMAIN") || !p.user {
        return if p.attribution_required { Terms::Credit } else { Terms::Free };
    }
    Terms::Unread
}

/// the credit line for a pack: who made it, its licence, where to read it
pub(super) fn credit(p: &PackInfo) -> String {
    let mut s = p.attribution.trim().to_string();
    let squash = |t: &str| t.to_ascii_lowercase().replace(['-', ' '], "");
    if !p.licence.is_empty() && !squash(&s).contains(&squash(&p.licence)) {
        s.push_str(&format!(" ({})", p.licence));
    }
    if !p.licence_url.is_empty() {
        s.push_str(&format!(", {}", p.licence_url));
    }
    s
}

pub(super) fn check(used: &[Used], out: &mut Vec<Check>, credits: &mut Vec<String>) {
    let mut say = |level, message: String| out.push(Check { level, about: About::Sounds, message });
    for u in used {
        let tracks = list(&u.tracks);
        match &u.origin {
            Origin::Pack(p) => {
                let title = if p.title.is_empty() { p.name.as_str() } else { p.title.as_str() };
                let licence = p.licence.as_str();
                match terms(p) {
                    Terms::Free => {}
                    Terms::Credit => {
                        let line = credit(p);
                        say(
                            Level::Note,
                            format!("{title}, in {tracks}, asks for credit, which release.txt has: {line}"),
                        );
                        credits.push(line);
                    }
                    Terms::Unknown => say(
                        Level::Problem,
                        format!(
                            "{title}, in {tracks}, has no known licence, and a release needs the rights to every \
                             sound in it: swap those sounds for ones whose licence is known, such as the tr909, tr808 and \
                             tr707 kits"
                        ),
                    ),
                    Terms::NoDerivatives => say(
                        Level::Problem,
                        format!(
                            "{title}, in {tracks}, is {licence}, which forbids new works made from it: swap those sounds"
                        ),
                    ),
                    Terms::NonCommercial => say(
                        Level::Problem,
                        format!(
                            "{title}, in {tracks}, is {licence}, which forbids commercial use, and stores sell and \
                             stream music: swap those sounds"
                        ),
                    ),
                    Terms::ShareAlike => say(
                        Level::Problem,
                        format!(
                            "{title}, in {tracks}, is {licence}: a song made with it has to be shared under {licence} \
                             too, and stores' terms and copy protection rule that out. Share it under that licence \
                             where you can say so (SoundCloud, Bandcamp), or swap those sounds for a store release"
                        ),
                    ),
                    Terms::Unread => say(
                        Level::Warning,
                        format!(
                            "{title}, in {tracks}, has its own licence, '{licence}': make sure it lets you release music \
                             made with it"
                        ),
                    ),
                }
            }
            Origin::Missing(name) => say(
                Level::Problem,
                format!(
                    "the pack '{name}', in {tracks}, is not installed here, so its licence cannot be checked: install \
                     it, or swap those sounds"
                ),
            ),
            Origin::AppleLoops => say(
                Level::Note,
                format!(
                    "Apple Loops, in {tracks}: Apple's licence lets you release music made with them, though not the \
                     loops on their own"
                ),
            ),
            Origin::Own => say(
                Level::Warning,
                format!(
                    "your own recordings, in {tracks} ({}): release them only if you made them or have cleared their \
                     samples",
                    u.sounds.join(", ")
                ),
            ),
        }
    }
}

/// "a", "a and b", "a, b and c"
fn list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(name: &str, licence: &str, credit: bool, user: bool) -> PackInfo {
        PackInfo {
            name: name.into(),
            title: format!("The {name} pack"),
            description: String::new(),
            categories: Vec::new(),
            licence: licence.into(),
            licence_url: String::new(),
            attribution: format!("{name} by Someone"),
            attribution_required: credit,
            licence_note: String::new(),
            source: String::new(),
            installed: true,
            lazy: false,
            size_mb: 1.0,
            default: true,
            opt_in: false,
            warning: None,
            user,
            instruments: 1,
        }
    }

    #[test]
    fn licences_read_as_what_they_allow() {
        for (licence, credit, user, expect) in [
            ("CC0-1.0", false, false, Terms::Free),
            ("CC-BY-3.0", true, false, Terms::Credit),
            ("CC-BY-SA-4.0", true, false, Terms::ShareAlike),
            ("CC BY-NC 4.0", true, true, Terms::NonCommercial),
            ("cc-by-nc-nd-4.0", true, true, Terms::NoDerivatives),
            ("unknown", false, false, Terms::Unknown),
            (
                "jRhodes licence: free for making music; redistributing the samples: CC BY-NC 4.0",
                false,
                false,
                Terms::Free,
            ),
            ("Public domain (AKAI)", false, true, Terms::Free),
            ("free for my friends", false, true, Terms::Unread),
        ] {
            assert_eq!(terms(&info("x", licence, credit, user)), expect, "{licence}");
        }
    }

    #[test]
    fn each_origin_says_what_a_release_needs() {
        let used = [
            Used {
                origin: Origin::Pack(Box::new(info("salamander", "CC-BY-3.0", true, false))),
                tracks: vec!["keys".into()],
                sounds: Vec::new(),
            },
            Used {
                origin: Origin::Pack(Box::new(info("tidal", "unknown", false, false))),
                tracks: vec!["kick".into(), "hats".into(), "snare".into()],
                sounds: Vec::new(),
            },
            Used { origin: Origin::AppleLoops, tracks: vec!["loop".into()], sounds: Vec::new() },
            Used { origin: Origin::Own, tracks: vec!["vox".into()], sounds: vec!["sample 'me.wav'".into()] },
        ];
        let (mut checks, mut credits) = (Vec::new(), Vec::new());
        check(&used, &mut checks, &mut credits);
        let levels: Vec<Level> = checks.iter().map(|c| c.level).collect();
        assert_eq!(levels, [Level::Note, Level::Problem, Level::Note, Level::Warning], "{checks:?}");
        assert!(checks[1].message.starts_with("The tidal pack, in kick, hats and snare, has no known licence"));
        assert_eq!(credits, ["salamander by Someone (CC-BY-3.0)"]);
        assert!(checks[3].message.contains("me.wav"));
    }
}

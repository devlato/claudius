//! Themes: looks of the person's own, or a community's, as data (`docs/src/guide/themes.md`). A theme file is JSON: its
//! format, its name, the look it is drawn on (`dark` or `light`: what egui draws it as, and how the roles' and the
//! sections' colours are shaded on it), and any of the look's colours, each by its name; the colours it does not name
//! are that look's. The studio's two looks are themes too (`builtin`), which Copy this look as a theme writes out
//! whole, to start a theme from. A theme added is kept in claudius's settings folder (`themes/`), or in a browser in
//! the page's storage, and every later studio reads it: a colour it does not know is left out, not refused.

use serde_json::{Map, Value, json};

use crate::theme::{DARK, LIGHT, Palette};

/// the theme format this studio writes; it reads every earlier one
pub const FORMAT: u64 = 1;

/// where a browser keeps the themes added, in the page's storage
#[cfg(target_arch = "wasm32")]
pub const STORED: &str = "claudius-themes";

/// a theme: its name, whether it is drawn on the dark look, and its colours
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub name: String,
    pub dark: bool,
    pub palette: Palette,
}

impl Theme {
    /// the studio's own dark look, or its light one, as a theme
    pub fn builtin(dark: bool) -> Theme {
        Theme { name: if dark { "Dark" } else { "Light" }.into(), dark, palette: if dark { DARK } else { LIGHT } }
    }

    /// The theme a file holds, and the colours it names that this studio does not know (left out).
    pub fn parse(text: &str) -> Result<(Theme, Vec<String>), String> {
        let v: Value = serde_json::from_str(text).map_err(|e| format!("not a theme: {e}"))?;
        let format = v["claudius_theme"].as_u64().ok_or("not a claudius theme: it has no \"claudius_theme\" format")?;
        if format == 0 {
            return Err("a theme's format is 1 or later".into());
        }
        let name = v["name"].as_str().map(str::trim).filter(|n| !n.is_empty()).ok_or("a theme has a name")?;
        let dark = match v["base"].as_str() {
            Some("dark") | None => true,
            Some("light") => false,
            Some(other) => return Err(format!("a theme is drawn on the dark look or the light one, not {other}")),
        };
        let base = if dark { DARK } else { LIGHT };
        let empty = Map::new();
        let colours = v["colors"].as_object().unwrap_or(&empty);
        let (palette, unknown) = base.with_colours(colours)?;
        // trimmed again after the cut, so that a name read back is the name kept, whatever character the cut ends on
        let name = name.chars().take(60).collect::<String>().trim_end().to_string();
        Ok((Theme { name, dark, palette }, unknown))
    }

    /// The theme as a file holds it, every colour named: to keep, to share, or to make another from.
    pub fn to_json(&self) -> String {
        let v = json!({
            "claudius_theme": FORMAT,
            "name": self.name,
            "base": if self.dark { "dark" } else { "light" },
            "colors": self.palette.colours(),
        });
        serde_json::to_string_pretty(&v).unwrap_or_default()
    }

    /// a file name for it, from its name (`theme.json` for a name of no letters or digits)
    #[cfg(not(target_arch = "wasm32"))]
    pub fn file_name(&self) -> String {
        format!("{}.json", self.file_stem())
    }

    /// the name of its file without `.json`: its letters and digits, lower case, the rest dashes; at most 200 bytes, so
    /// that the file and the part file it is first written to stay under a file system's 255
    #[cfg(not(target_arch = "wasm32"))]
    fn file_stem(&self) -> String {
        let stem: String =
            self.name.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { '-' }).collect();
        let mut stem = stem.trim_matches('-').to_string();
        while stem.len() > 200 {
            stem.pop();
        }
        if stem.is_empty() { "theme".into() } else { stem }
    }
}

/// Wears `theme`, a theme of the person's own, kept with the themes added in place of one of its name; with none, the
/// look `view.look`.
pub fn wear(ctx: &eframe::egui::Context, view: &mut crate::panels::View, theme: Option<Theme>) {
    match theme {
        Some(t) => {
            crate::theme::wear(ctx, Some((&t.palette, t.dark)), view.look);
            view.theme = Some(t.name.clone());
            view.themes.retain(|o| o.name != t.name);
            view.themes.push(t);
            view.themes.sort_by_key(|t| t.name.to_lowercase());
        }
        None => {
            crate::theme::wear(ctx, None, view.look);
            view.theme = None;
        }
    }
}

/// the folder the themes added are kept in: `themes` in claudius's settings folder
#[cfg(not(target_arch = "wasm32"))]
pub fn folder() -> std::path::PathBuf {
    claudius::config::settings_dir().join("themes")
}

/// Whether `p` is a theme's file: a `.json`, or the file named `.json` alone that studios before this one wrote for a
/// theme named in symbols only, which every later studio reads (design 0016).
#[cfg(not(target_arch = "wasm32"))]
fn is_theme_file(p: &std::path::Path) -> bool {
    p.extension().is_some_and(|e| e.eq_ignore_ascii_case("json")) || p.file_name().is_some_and(|n| n == ".json")
}

/// The theme files in folder `dir`, in the order they are read: of two files holding one name, the later wins.
#[cfg(not(target_arch = "wasm32"))]
fn theme_files(dir: &std::path::Path) -> std::io::Result<Vec<std::path::PathBuf>> {
    let mut files: Vec<std::path::PathBuf> =
        std::fs::read_dir(dir)?.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| is_theme_file(p)).collect();
    files.sort();
    Ok(files)
}

/// The themes added, as their files hold them (in a browser, the page's storage), by name; one that does not read is
/// left out, and said.
pub fn added(storage: Option<&dyn eframe::Storage>) -> Vec<Theme> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = storage;
        added_in(&folder())
    }
    #[cfg(target_arch = "wasm32")]
    {
        named(
            storage
                .and_then(|s| s.get_string(STORED))
                .and_then(|t| serde_json::from_str::<Vec<String>>(&t).ok())
                .unwrap_or_default(),
        )
    }
}

/// `added` from folder `dir`
#[cfg(not(target_arch = "wasm32"))]
fn added_in(dir: &std::path::Path) -> Vec<Theme> {
    let files = theme_files(dir).unwrap_or_default();
    named(files.iter().filter_map(|p| std::fs::read_to_string(p).ok()).collect())
}

/// the themes `texts` hold, the later of two of one name winning, by name; one that does not read is left out, and said
fn named(texts: Vec<String>) -> Vec<Theme> {
    let mut out: Vec<Theme> = Vec::new();
    for t in texts {
        match Theme::parse(&t) {
            Ok((theme, _)) => {
                out.retain(|o| o.name != theme.name);
                out.push(theme);
            }
            Err(e) => eprintln!("claudius: a theme left out: {e}"),
        }
    }
    out.sort_by_key(|t| t.name.to_lowercase());
    out
}

/// Keeps `theme` with the themes added, a file in the themes folder, in place of one of its name.
#[cfg(not(target_arch = "wasm32"))]
pub fn keep(theme: &Theme) -> Result<(), String> {
    keep_in(&folder(), theme)
}

/// `keep` in folder `dir`: written to the file that holds a theme of its name already (of several, the one read last,
/// which `added` keeps, so that a copy left behind never wins), else to a file of its own name that nothing else is in
/// (`-2`, `-3` ... after it while one is: `Ember` and `Ember!` both stay), whole or not at all (a failed write leaves the
/// theme kept before); and only once it is written, any other file of its name goes. A folder that cannot be listed
/// is said, rather than taken for empty.
#[cfg(not(target_arch = "wasm32"))]
fn keep_in(dir: &std::path::Path, theme: &Theme) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot make {}: {e}", dir.display()))?;
    let holds = |p: &std::path::Path| -> Option<String> {
        std::fs::read_to_string(p).ok().and_then(|t| Theme::parse(&t).ok()).map(|(t, _)| t.name)
    };
    let files = theme_files(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    let same: Vec<std::path::PathBuf> = files.into_iter().filter(|p| holds(p).as_ref() == Some(&theme.name)).collect();
    let path = match same.last() {
        Some(p) => p.clone(),
        None => {
            // a name nothing is in: a link to a disk not there now is something, though it leads nowhere
            let stem = theme.file_stem();
            std::iter::once(dir.join(format!("{stem}.json")))
                .chain((2..).map(|k| dir.join(format!("{stem}-{k}.json"))))
                .find(|p| std::fs::symlink_metadata(p).is_err())
                .expect("a name nothing is in")
        }
    };
    claudius::config::write_whole(&path, |p| Ok(std::fs::write(p, theme.to_json())?)).map_err(|e| format!("{e:#}"))?;
    for p in same.iter().filter(|p| **p != path) {
        let _ = std::fs::remove_file(p);
    }
    Ok(())
}

/// Keeps the themes added in a browser's storage, with the settings.
#[cfg(target_arch = "wasm32")]
pub fn store(themes: &[Theme], storage: &mut dyn eframe::Storage) {
    let texts: Vec<String> = themes.iter().map(Theme::to_json).collect();
    storage.set_string(STORED, serde_json::to_string(&texts).unwrap_or_default());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn themes_whose_names_make_one_file_name_are_kept_apart_and_one_of_no_letters_is_kept_too() {
        let dir = std::env::temp_dir().join(format!("claudius-theme-files-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let named = |name: &str, dark: bool| Theme { name: name.into(), ..Theme::builtin(dark) };
        let names = |dir: &std::path::Path| -> Vec<(String, String)> {
            let mut files: Vec<(String, String)> = std::fs::read_dir(dir)
                .unwrap()
                .filter_map(|e| e.ok().map(|e| e.path()))
                .map(|p| {
                    let t = Theme::parse(&std::fs::read_to_string(&p).unwrap()).unwrap().0;
                    (p.file_name().unwrap().to_string_lossy().into_owned(), t.name)
                })
                .collect();
            files.sort();
            files
        };
        for t in [named("Ember", true), named("Ember!", false), named("!!!", true)] {
            keep_in(&dir, &t).unwrap();
        }
        assert_eq!(
            names(&dir),
            [("ember-2.json", "Ember!"), ("ember.json", "Ember"), ("theme.json", "!!!")]
                .map(|(f, n)| (f.to_string(), n.to_string()))
        );
        // kept again, changed, a theme goes back into its own file, and the one beside it stays as it was
        keep_in(&dir, &named("Ember!", true)).unwrap();
        let again = names(&dir);
        assert_eq!(again.len(), 3);
        let ember2 = Theme::parse(&std::fs::read_to_string(dir.join("ember-2.json")).unwrap()).unwrap().0;
        assert!(ember2.dark && ember2.name == "Ember!");
        let ember = Theme::parse(&std::fs::read_to_string(dir.join("ember.json")).unwrap()).unwrap().0;
        assert_eq!(ember.name, "Ember");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// a folder of its own for a test, empty
    #[cfg(not(target_arch = "wasm32"))]
    fn folder_for(test: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("claudius-themes-{test}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn a_name_cut_where_a_space_falls_reads_back_as_it_was_kept_and_stays_one_file() {
        let dir = folder_for("cut");
        // 59 letters, a space, more: the cut at 60 ends on the space
        let long = format!("{} tail", "a".repeat(59));
        let theme = Theme::parse(&Theme { name: long, ..Theme::builtin(true) }.to_json()).unwrap().0;
        assert_eq!(theme.name, "a".repeat(59), "no space left at the end of the name");
        keep_in(&dir, &theme).unwrap();
        keep_in(&dir, &Theme { dark: false, ..theme.clone() }).unwrap();
        let files = theme_files(&dir).unwrap();
        assert_eq!(files.len(), 1, "kept again into its own file: {files:?}");
        assert!(!added_in(&dir)[0].dark, "and read back as it was kept last");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn a_theme_kept_again_goes_into_the_file_read_last_and_its_other_copies_go() {
        let dir = folder_for("copies");
        let ember = |dark: bool| Theme { name: "Ember".into(), ..Theme::builtin(dark) };
        // two files of one name, as a copy dropped in by hand leaves them: the later one is the one read
        std::fs::write(dir.join("ember-2.json"), ember(true).to_json()).unwrap();
        std::fs::write(dir.join("ember.json"), ember(true).to_json()).unwrap();
        keep_in(&dir, &ember(false)).unwrap();
        let files: Vec<String> =
            theme_files(&dir).unwrap().iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(files, ["ember.json"], "written to the file read last, the copy gone");
        assert!(!added_in(&dir)[0].dark, "the theme as kept last is the one read");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn a_theme_an_earlier_studio_kept_as_dot_json_is_read_and_replaced_when_kept_again() {
        let dir = folder_for("dotjson");
        // studios before this one kept a theme named in symbols alone as `.json`
        std::fs::write(dir.join(".json"), Theme { name: "!!!".into(), ..Theme::builtin(true) }.to_json()).unwrap();
        assert_eq!(added_in(&dir).iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), ["!!!"]);
        keep_in(&dir, &Theme { name: "!!!".into(), ..Theme::builtin(false) }).unwrap();
        let read = added_in(&dir);
        assert_eq!(read.len(), 1);
        assert!(!read[0].dark, "kept again, in its old file: {:?}", theme_files(&dir).unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn a_long_name_makes_a_file_name_short_enough_to_write() {
        // 60 letters of four bytes each, alphanumeric and without a lower case
        let theme = Theme { name: "\u{1d4d4}".repeat(60), ..Theme::builtin(true) };
        assert!(theme.file_name().len() <= 205, "{} bytes", theme.file_name().len());
        let dir = folder_for("long");
        keep_in(&dir, &theme).unwrap();
        assert_eq!(added_in(&dir)[0].name, theme.name);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_looks_are_themes_written_whole_and_read_back_as_they_are() {
        for dark in [true, false] {
            let t = Theme::builtin(dark);
            let (back, unknown) = Theme::parse(&t.to_json()).unwrap();
            assert_eq!(back, t);
            assert!(unknown.is_empty(), "{unknown:?}");
        }
        assert_eq!(Theme::builtin(true).file_name(), "dark.json");
    }

    #[test]
    fn a_theme_names_the_colours_it_changes_and_takes_the_rest_from_its_look() {
        let text = r##"{"claudius_theme": 1, "name": "Ember", "base": "dark",
                        "colors": {"accent": "#ff5500", "edge": "#ffffff", "faint_bg": "#11223380", "shadow": 90,
                                   "glow": "#123456"}}"##;
        let (t, unknown) = Theme::parse(text).unwrap();
        assert_eq!((t.name.as_str(), t.dark), ("Ember", true));
        assert_eq!(t.palette.accent, eframe::egui::Color32::from_rgb(255, 85, 0));
        // the rest the dark look's, but for its edge, its faintest fill (see-through) and its shadows
        let c = t.palette.colours();
        assert_eq!((&c["edge"], &c["shadow"]), (&serde_json::json!("#ffffff"), &serde_json::json!(90)));
        assert!(c["faint_bg"].as_str().is_some_and(|h| h.len() == 9), "{}", c["faint_bg"]);
        for name in ["text", "panel", "lcd", "bad"] {
            assert_eq!(c[name], DARK.colours()[name], "{name}");
        }
        assert_eq!(unknown, ["glow"], "a colour of a later studio's is left out, and said");
        // on the light look, the light one's colours
        let (light, _) = Theme::parse(r#"{"claudius_theme": 1, "name": "Paper", "base": "light"}"#).unwrap();
        assert_eq!(light.palette, LIGHT);
    }

    #[test]
    fn a_theme_that_does_not_read_says_why() {
        let bad = [
            ("{", "not a theme"),
            (r#"{"name": "x"}"#, "no \"claudius_theme\""),
            (r#"{"claudius_theme": 1}"#, "has a name"),
            (r#"{"claudius_theme": 1, "name": "x", "base": "blue"}"#, "not blue"),
            (r#"{"claudius_theme": 1, "name": "x", "colors": {"text": "white"}}"#, "text: \"white\" is not a colour"),
            (r#"{"claudius_theme": 1, "name": "x", "colors": {"lcd_hold": 3}}"#, "lcd_hold: 3 is not 0 to 1"),
        ];
        for (text, says) in bad {
            let e = Theme::parse(text).unwrap_err();
            assert!(e.contains(says), "{text}: {e}");
        }
    }
}

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
        Ok((Theme { name: name.chars().take(60).collect(), dark, palette }, unknown))
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
        let stem: String =
            self.name.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { '-' }).collect();
        let stem = stem.trim_matches('-');
        format!("{}.json", if stem.is_empty() { "theme" } else { stem })
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

/// The themes added, as their files hold them (in a browser, the page's storage), by name; one that does not read is
/// left out, and said.
pub fn added(storage: Option<&dyn eframe::Storage>) -> Vec<Theme> {
    let texts: Vec<String> = {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = storage;
            let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(folder())
                .map(|d| d.filter_map(|e| e.ok().map(|e| e.path())).collect())
                .unwrap_or_default();
            files.retain(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("json")));
            files.sort();
            files.iter().filter_map(|p| std::fs::read_to_string(p).ok()).collect()
        }
        #[cfg(target_arch = "wasm32")]
        {
            storage
                .and_then(|s| s.get_string(STORED))
                .and_then(|t| serde_json::from_str::<Vec<String>>(&t).ok())
                .unwrap_or_default()
        }
    };
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

/// `keep` in folder `dir`: written to the file that holds a theme of its name already, else to a file of its own name
/// that no other theme holds (`-2`, `-3` ... after it while one does: `Ember` and `Ember!` both stay), whole or not at
/// all (a failed write leaves the theme kept before); and only once it is written, any other file of its name goes.
#[cfg(not(target_arch = "wasm32"))]
fn keep_in(dir: &std::path::Path, theme: &Theme) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot make {}: {e}", dir.display()))?;
    let holds = |p: &std::path::Path| -> Option<String> {
        std::fs::read_to_string(p).ok().and_then(|t| Theme::parse(&t).ok()).map(|(t, _)| t.name)
    };
    let mut same: Vec<std::path::PathBuf> =
        std::fs::read_dir(dir).map(|d| d.filter_map(|e| e.ok().map(|e| e.path())).collect()).unwrap_or_default();
    same.retain(|p| {
        p.extension().is_some_and(|e| e.eq_ignore_ascii_case("json")) && holds(p).as_ref() == Some(&theme.name)
    });
    same.sort();
    let path = match same.first() {
        Some(p) => p.clone(),
        None => {
            let name = theme.file_name();
            let stem = name.trim_end_matches(".json");
            let mut path = dir.join(&name);
            let mut k = 2;
            while path.exists() {
                path = dir.join(format!("{stem}-{k}.json"));
                k += 1;
            }
            path
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

//! Choosing a file on this computer and reading it into memory: the system's own dialog in the app, the browser's in a
//! page. `choose` opens it; what was chosen arrives through `chosen`, read, once the dialog is done with it.

use std::cell::RefCell;
use std::path::PathBuf;

use eframe::egui;

/// what a file is chosen for, which says what the dialog offers
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Want {
    /// a song to open: a blueprint.json, a script, or a project the demo saved (a zip)
    Song,
    /// a recording to listen to, or to make a song of
    Audio,
    /// a recording to add to the song (the Add a recording window's)
    Recording,
    /// a release's cover
    Cover,
}

impl Want {
    /// the kinds of file it offers: what they are, and their extensions
    pub fn kinds(self) -> (&'static str, &'static [&'static str]) {
        match self {
            Want::Song => ("a claudius song", &["json", "rhai", "zip"]),
            Want::Audio | Want::Recording => {
                ("audio", &["wav", "mp3", "flac", "m4a", "aac", "aif", "aiff", "ogg", "opus", "caf"])
            }
            Want::Cover => ("an image", &["png", "jpg", "jpeg"]),
        }
    }

    /// the browser's `accept`: its extensions, each with its dot
    #[cfg(any(target_arch = "wasm32", test))]
    fn accept(self) -> String {
        self.kinds().1.iter().map(|e| format!(".{e}")).collect::<Vec<_>>().join(",")
    }
}

/// a file read into memory: its name, as the person sees it, what it holds, and the folder it is in when the system's
/// dialog says (a browser's never does)
#[derive(Clone, Debug, PartialEq)]
pub struct Picked {
    pub name: String,
    pub bytes: Vec<u8>,
    pub folder: Option<PathBuf>,
}

impl Picked {
    /// a song file as its name and its text: a blueprint.json or a script is text, and nothing else is a song
    pub fn text(self) -> Result<(String, String), String> {
        match String::from_utf8(self.bytes) {
            Ok(text) => Ok((self.name, text)),
            Err(_) => Err(format!("{} is not a song: choose a blueprint.json or a claudius script", self.name)),
        }
    }

    /// A blueprint.json that plays files beside it, as a project the demo saved does once unpacked (Safari unpacks a
    /// zip it downloads), as that project (`song::pack_project`), named after its folder: those files go with it, and
    /// the `.json` and `credits.txt` that credit them, since claudius may run where it cannot read them. None: it
    /// plays no file beside it.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn as_project(&self) -> Option<Result<(String, Vec<u8>), String>> {
        use std::path::{Component, Path};
        let folder = self.folder.as_deref()?;
        if !self.name.to_lowercase().ends_with(".json") {
            return None;
        }
        let bp = claudius::model::Blueprint::from_json(std::str::from_utf8(&self.bytes).ok()?).ok()?;
        let within = |f: &str| Path::new(f).components().all(|c| matches!(c, Component::Normal(_)));
        let beside: std::collections::BTreeSet<&str> =
            bp.files().into_iter().filter(|f| !f.contains(':') && within(f) && folder.join(f).is_file()).collect();
        if beside.is_empty() {
            return None;
        }
        let names = beside.iter().flat_map(|f| [f.to_string(), format!("{f}.json")]).chain(["credits.txt".into()]);
        let files = names
            .filter(|f| folder.join(f).is_file())
            .map(|f| {
                std::fs::read(folder.join(&f)).map(|b| (f.clone(), b)).map_err(|e| format!("cannot read {f}: {e}"))
            })
            .collect::<Result<Vec<_>, _>>();
        let name = folder.file_name().map_or_else(|| "project".into(), |n| n.to_string_lossy().into_owned());
        let zip = files.and_then(|files| claudius::song::pack_project(&bp, &files).map_err(|e| format!("{e:#}")));
        Some(zip.map(|zip| (format!("{name}.zip"), zip)))
    }
}

/// the largest file read: claudius takes no more in one go
pub const MOST: usize = 300 << 20;

thread_local! {
    /// what was chosen, read, for each `Want`, until it is taken
    static CHOSEN: RefCell<Vec<(Want, Result<Picked, String>)>> = const { RefCell::new(Vec::new()) };
    /// the window to wake when a file has been read: a browser reads one after the frame that asked for it
    static WAKE: RefCell<Option<egui::Context>> = const { RefCell::new(None) };
}

/// wakes `ctx` when what is chosen next has been read
fn wake(ctx: &egui::Context) {
    WAKE.with(|w| *w.borrow_mut() = Some(ctx.clone()));
}

/// takes what was chosen for `want`, once it has been read
pub fn chosen(want: Want) -> Option<Result<Picked, String>> {
    CHOSEN.with(|c| {
        let mut c = c.borrow_mut();
        let i = c.iter().position(|(w, _)| *w == want)?;
        Some(c.remove(i).1)
    })
}

/// what was read of a file named `name`: refused when it is larger than claudius takes
#[cfg(any(target_arch = "wasm32", test))]
fn arrived(want: Want, name: String, bytes: Result<Vec<u8>, String>) {
    arrived_from(want, name, bytes, None);
}

/// Reads a file dropped on the studio as if it had been chosen for `want`: it arrives through `chosen` (in a browser,
/// once read, after the frame it was dropped in).
pub fn read_dropped(ctx: &egui::Context, want: Want, file: egui::DroppedFileHandle) {
    wake(ctx);
    let path = file.path();
    let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
    #[cfg(not(target_arch = "wasm32"))]
    {
        let bytes = file.bytes().map_err(|e| format!("cannot read {name}: {e}"));
        arrived_from(want, name, bytes, path.parent().map(PathBuf::from));
    }
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_futures::spawn_local(async move {
        let bytes = file.bytes_async().await.map_err(|e| format!("cannot read {name}: {e}"));
        arrived(want, name, bytes);
    });
}

/// what was read of a file named `name` in `folder`
fn arrived_from(want: Want, name: String, bytes: Result<Vec<u8>, String>, folder: Option<PathBuf>) {
    let got = bytes.and_then(|b| match b.len() {
        n if n > MOST => Err(format!("{name} is {} MB, more than claudius takes ({} MB)", n >> 20, MOST >> 20)),
        _ => Ok(Picked { name, bytes: b, folder }),
    });
    CHOSEN.with(|c| c.borrow_mut().push((want, got)));
    WAKE.with(|w| {
        if let Some(ctx) = &*w.borrow() {
            ctx.request_repaint();
        }
    });
}

/// opens the system's dialog for a file for `want`; what is chosen is read and arrives through `chosen` (nothing, when
/// the person closes the dialog)
#[cfg(not(target_arch = "wasm32"))]
pub fn choose(ctx: &egui::Context, want: Want) {
    wake(ctx);
    let (what, extensions) = want.kinds();
    let Some(path) = rfd::FileDialog::new().add_filter(what, extensions).pick_file() else { return };
    let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
    let bytes = std::fs::read(&path).map_err(|e| format!("cannot read {name}: {e}"));
    arrived_from(want, name, bytes, path.parent().map(PathBuf::from));
}

/// reads the file at `path` as if it had been chosen for `want` (for a scripted run)
#[cfg(not(target_arch = "wasm32"))]
pub fn take_path(want: Want, path: &std::path::Path) {
    let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
    let bytes = std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()));
    arrived_from(want, name, bytes, path.parent().map(PathBuf::from));
}

/// Opens the browser's dialog for a file for `want`: a file input of the page's own, which waits in the page (hidden,
/// `data-claudius-pick` naming what it is for) until a file is chosen, and goes when one is, or when another is asked
/// for. What is chosen is read and arrives through `chosen`.
#[cfg(target_arch = "wasm32")]
pub fn choose(ctx: &egui::Context, want: Want) {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;
    wake(ctx);
    let Some(document) = web_sys::window().and_then(|w| w.document()) else { return };
    let Some(body) = document.body() else { return };
    // a dialog closed without a file leaves its input behind: one at a time
    if let Ok(old) = document.query_selector_all("input[data-claudius-pick]") {
        for i in 0..old.length() {
            if let Some(node) = old.item(i) {
                let _ = body.remove_child(&node);
            }
        }
    }
    let Ok(input) =
        document.create_element("input").and_then(|e| e.dyn_into::<web_sys::HtmlInputElement>().map_err(Into::into))
    else {
        return;
    };
    input.set_type("file");
    input.set_accept(&want.accept());
    let _ = input.set_attribute("data-claudius-pick", &format!("{want:?}").to_lowercase());
    let _ = input.set_attribute("style", "display: none");
    let _ = body.append_child(&input);
    let held = input.clone();
    let on_change = Closure::once_into_js(move || {
        let file = held.files().and_then(|f| f.get(0));
        held.remove();
        let Some(file) = file else { return };
        wasm_bindgen_futures::spawn_local(async move {
            let read = wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await;
            let bytes = read
                .map(|b| js_sys::Uint8Array::new(&b).to_vec())
                .map_err(|e| format!("cannot read {}: {e:?}", file.name()));
            arrived(want, file.name(), bytes);
        });
    });
    input.set_onchange(Some(on_change.unchecked_ref()));
    input.click();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_chosen_waits_for_what_it_was_chosen_for() {
        arrived(Want::Cover, "cover.png".into(), Ok(vec![1, 2, 3]));
        arrived(Want::Song, "blueprint.json".into(), Ok(b"{}".to_vec()));
        arrived(Want::Audio, "gone.wav".into(), Err("cannot read gone.wav: no such file".into()));
        assert_eq!(
            chosen(Want::Song),
            Some(Ok(Picked { name: "blueprint.json".into(), bytes: b"{}".to_vec(), folder: None }))
        );
        assert_eq!(chosen(Want::Song), None, "taken once");
        assert!(chosen(Want::Audio).unwrap().unwrap_err().contains("no such file"));
        assert_eq!(chosen(Want::Cover).unwrap().unwrap().bytes, [1, 2, 3]);
    }

    #[test]
    fn a_file_read_wakes_the_window_that_asked_for_it() {
        let ctx = egui::Context::default();
        wake(&ctx);
        // frames, whose textures nothing draws, until egui asks for no more
        for _ in 0..10 {
            if !ctx.has_requested_repaint() {
                break;
            }
            ctx.run_ui(Default::default(), |_| {}).textures_delta.clear();
        }
        assert!(!ctx.has_requested_repaint(), "asleep");
        arrived(Want::Song, "song.rhai".into(), Ok(b"tempo(120)".to_vec()));
        assert!(ctx.has_requested_repaint(), "woken by what arrived");
        assert!(chosen(Want::Song).is_some());
    }

    #[test]
    fn a_file_larger_than_claudius_takes_is_refused_by_its_size() {
        arrived(Want::Audio, "huge.wav".into(), Ok(vec![0; MOST + 1]));
        let e = chosen(Want::Audio).unwrap().unwrap_err();
        assert!(e.starts_with("huge.wav is 300 MB, more than claudius takes (300 MB)"), "{e}");
    }

    #[test]
    fn a_song_is_text_and_anything_else_is_not_one() {
        let song =
            Picked { name: "blueprint.json".into(), bytes: r#"{"title": "Ü"}"#.as_bytes().to_vec(), folder: None };
        assert_eq!(song.text(), Ok(("blueprint.json".into(), r#"{"title": "Ü"}"#.into())));
        let picture =
            Picked { name: "cover.png".into(), bytes: vec![0x89, b'P', b'N', b'G', 0xFF, 0xFE], folder: None };
        assert_eq!(
            picture.text().unwrap_err(),
            "cover.png is not a song: choose a blueprint.json or a claudius script"
        );
    }

    #[test]
    fn a_blueprint_chosen_beside_the_samples_it_plays_goes_as_a_project_with_them() {
        use claudius::model::Sound;
        let dir = std::env::temp_dir().join(format!("claudius-pick-project-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("samples/drums")).unwrap();
        std::fs::write(dir.join("samples/drums/kick.flac"), b"fLaC").unwrap();
        std::fs::write(dir.join("samples/drums/kick.flac.json"), b"{}").unwrap();
        std::fs::write(dir.join("outside.flac"), b"fLaC").unwrap();
        let house = claudius::genres::get("house").unwrap();
        let plain = claudius::compose::generate(&house, 3, &Default::default(), None);
        let mut bp = plain.clone();
        let sample =
            |path: &str| Sound::Sample { path: path.into(), root: 60, start: 0.0, end: None, label: "x".into() };
        (bp.drums[0].sound, bp.drums[1].sound) = (sample("samples/drums/kick.flac"), sample("samples/../outside.flac"));
        let chosen = |bp: &claudius::model::Blueprint, folder: Option<&std::path::Path>| Picked {
            name: "blueprint.json".into(),
            bytes: bp.to_json().into_bytes(),
            folder: folder.map(Into::into),
        };

        let (name, zip) = chosen(&bp, Some(&dir)).as_project().expect("a project").unwrap();
        assert_eq!(name, format!("{}.zip", dir.file_name().unwrap().to_string_lossy()));
        let opened = claudius::song::unpack_project(&zip, &name, &dir.join("songs")).unwrap();
        assert!(
            opened.join("samples/drums/kick.flac").is_file() && opened.join("samples/drums/kick.flac.json").is_file()
        );
        assert!(!opened.join("outside.flac").exists(), "a file from outside its folder stays there");
        assert_eq!(
            chosen(&plain, Some(&dir)).as_project(),
            None,
            "a song that plays no file beside it opens by its text"
        );
        assert_eq!(chosen(&bp, None).as_project(), None, "a browser says no folder");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn each_want_offers_its_kinds_of_file() {
        assert_eq!(Want::Song.accept(), ".json,.rhai,.zip");
        assert!(Want::Audio.kinds().1.contains(&"m4a") && Want::Audio.accept().starts_with(".wav,.mp3,"));
        assert_eq!(Want::Cover.kinds(), ("an image", &["png", "jpg", "jpeg"][..]));
    }
}

//! Choosing a file on this computer and reading it into memory: the system's own dialog in the app, the browser's in a
//! page. `choose` opens it; what was chosen arrives through `chosen`, read, once the dialog is done with it.

use std::cell::RefCell;

use eframe::egui;

/// what a file is chosen for, which says what the dialog offers
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Want {
    /// a song to open: a blueprint.json or a script
    Song,
    /// a recording to listen to, or to make a song of
    Audio,
    /// a release's cover
    Cover,
}

impl Want {
    /// the kinds of file it offers: what they are, and their extensions
    pub fn kinds(self) -> (&'static str, &'static [&'static str]) {
        match self {
            Want::Song => ("a claudius song", &["json", "rhai"]),
            Want::Audio => ("audio", &["wav", "mp3", "flac", "m4a", "aac", "aif", "aiff", "ogg", "opus", "caf"]),
            Want::Cover => ("an image", &["png", "jpg", "jpeg"]),
        }
    }

    /// the browser's `accept`: its extensions, each with its dot
    #[cfg(any(target_arch = "wasm32", test))]
    fn accept(self) -> String {
        self.kinds().1.iter().map(|e| format!(".{e}")).collect::<Vec<_>>().join(",")
    }
}

/// a file read into memory: its name, as the person sees it, and what it holds
#[derive(Clone, Debug, PartialEq)]
pub struct Picked {
    pub name: String,
    pub bytes: Vec<u8>,
}

impl Picked {
    /// a song file as its name and its text: a blueprint.json or a script is text, and nothing else is a song
    pub fn text(self) -> Result<(String, String), String> {
        match String::from_utf8(self.bytes) {
            Ok(text) => Ok((self.name, text)),
            Err(_) => Err(format!("{} is not a song: choose a blueprint.json or a claudius script", self.name)),
        }
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
fn arrived(want: Want, name: String, bytes: Result<Vec<u8>, String>) {
    let got = bytes.and_then(|b| match b.len() {
        n if n > MOST => Err(format!("{name} is {} MB, more than claudius takes ({} MB)", n >> 20, MOST >> 20)),
        _ => Ok(Picked { name, bytes: b }),
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
    arrived(want, name, bytes);
}

/// reads the file at `path` as if it had been chosen for `want` (for a scripted run)
#[cfg(not(target_arch = "wasm32"))]
pub fn take_path(want: Want, path: &std::path::Path) {
    let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
    arrived(want, name, std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display())));
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
        assert_eq!(chosen(Want::Song), Some(Ok(Picked { name: "blueprint.json".into(), bytes: b"{}".to_vec() })));
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
        let song = Picked { name: "blueprint.json".into(), bytes: r#"{"title": "Ü"}"#.as_bytes().to_vec() };
        assert_eq!(song.text(), Ok(("blueprint.json".into(), r#"{"title": "Ü"}"#.into())));
        let picture = Picked { name: "cover.png".into(), bytes: vec![0x89, b'P', b'N', b'G', 0xFF, 0xFE] };
        assert_eq!(
            picture.text().unwrap_err(),
            "cover.png is not a song: choose a blueprint.json or a claudius script"
        );
    }

    #[test]
    fn each_want_offers_its_kinds_of_file() {
        assert_eq!(Want::Song.accept(), ".json,.rhai");
        assert!(Want::Audio.kinds().1.contains(&"m4a") && Want::Audio.accept().starts_with(".wav,.mp3,"));
        assert_eq!(Want::Cover.kinds(), ("an image", &["png", "jpg", "jpeg"][..]));
    }
}

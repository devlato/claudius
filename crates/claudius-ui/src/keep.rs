//! Handing the person a file of the song: in a page, a download the browser keeps with its other downloads; in the
//! app, the system's dialog asks where to save it. `reveal` shows a folder the song was saved in.

/// A file of the song to hand over: the bytes themselves (the demo's render), or where a server serves it.
#[derive(Debug, PartialEq)]
pub enum Delivery {
    Bytes { name: String, mime: String, bytes: Vec<u8> },
    Url { name: String, url: String },
}

/// Hands the file over: the name it was kept under, or None when the person closed the dialog.
#[cfg(target_arch = "wasm32")]
pub fn deliver(d: Delivery) -> Result<Option<String>, String> {
    match d {
        Delivery::Bytes { name, mime, bytes } => download(&name, &mime, &bytes).map(|()| Some(name)),
        Delivery::Url { name, url } => download_url(&url, &name).map(|()| Some(name)),
    }
}

/// Hands the file over: the name it was kept under, or None when the person closed the dialog.
#[cfg(not(target_arch = "wasm32"))]
pub fn deliver(d: Delivery) -> Result<Option<String>, String> {
    match d {
        Delivery::Bytes { name, bytes, .. } => {
            let Some(path) = rfd::FileDialog::new().set_file_name(&name).save_file() else { return Ok(None) };
            std::fs::write(&path, bytes).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
            Ok(Some(path.display().to_string()))
        }
        Delivery::Url { url, .. } => Err(format!("the app downloads nothing from {url}: the song's folder holds it")),
    }
}

#[cfg(target_arch = "wasm32")]
fn download(name: &str, mime: &str, bytes: &[u8]) -> Result<(), String> {
    use wasm_bindgen::JsCast;
    use web_sys::{Blob, BlobPropertyBag, Url};
    let parts = js_sys::Array::of1(&js_sys::Uint8Array::from(bytes));
    let props = BlobPropertyBag::new();
    props.set_type(mime);
    let blob = Blob::new_with_u8_array_sequence_and_options(&parts, &props).map_err(js_error)?;
    let url = Url::create_object_url_with_blob(&blob).map_err(js_error)?;
    let clicked = download_url(&url, name);
    // the browser reads the file after the click: it is let go of a minute later, not at once
    let free = wasm_bindgen::closure::Closure::once_into_js(move || {
        let _ = Url::revoke_object_url(&url);
    });
    if let Some(window) = web_sys::window() {
        let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(free.unchecked_ref(), 60_000);
    }
    clicked
}

#[cfg(target_arch = "wasm32")]
fn download_url(url: &str, name: &str) -> Result<(), String> {
    use wasm_bindgen::JsCast;
    let document = web_sys::window().and_then(|w| w.document()).ok_or("the page has no document")?;
    let link = document
        .create_element("a")
        .map_err(js_error)?
        .dyn_into::<web_sys::HtmlAnchorElement>()
        .map_err(|_| "the page made no link".to_string())?;
    link.set_href(url);
    link.set_download(name);
    link.click();
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn js_error(e: wasm_bindgen::JsValue) -> String {
    e.as_string().unwrap_or_else(|| format!("{e:?}"))
}

/// Shows `dir` in the system's file manager: Finder, Explorer, or the desktop's own.
#[cfg(not(target_arch = "wasm32"))]
pub fn reveal(dir: &std::path::Path) -> Result<(), String> {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    std::process::Command::new(opener)
        .arg(dir)
        .spawn()
        .map(drop)
        .map_err(|e| format!("cannot show {}: {e}", dir.display()))
}

/// The folder a save reply names: "saving to <folder> (…" or "saving to <folder>: …".
#[cfg(any(not(target_arch = "wasm32"), test))]
pub fn saved_folder(message: &str) -> Option<&str> {
    let rest = message.strip_prefix("saving to ")?;
    let end = [rest.find(" ("), rest.find(": ")].into_iter().flatten().min().unwrap_or(rest.len());
    Some(&rest[..end]).filter(|dir| !dir.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_folder_a_save_went_to_is_read_from_either_reply() {
        let at_once = "saving to /Users/me/Music/claudius/trap-liquid-signal-594000 (blueprint.json and song.rhai now, \
                       the audio after the render)";
        assert_eq!(saved_folder(at_once), Some("/Users/me/Music/claudius/trap-liquid-signal-594000"));
        let after_a_job = "saving to C:\\Users\\me\\Music\\claudius\\house-x-1: blueprint.json and song.rhai now, the \
                           audio once the current job (evolving) is done";
        assert_eq!(saved_folder(after_a_job), Some("C:\\Users\\me\\Music\\claudius\\house-x-1"));
        assert_eq!(saved_folder("no song to save"), None);
    }
}

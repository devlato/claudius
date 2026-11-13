use eframe::egui;
use wasm_bindgen::JsValue;
use web_sys::{Blob, BlobPropertyBag, HtmlAudioElement, Url};

use super::{Pcm, Source};

/// `HAVE_METADATA`: duration and seeking work from here on
const HAVE_METADATA: u16 = 1;

pub struct Player {
    el: Option<HtmlAudioElement>,
    /// object URL of generated audio, revoked when replaced
    object_url: Option<String>,
    /// the generated audio itself (the demo engine's render): what a take is recorded over
    pcm: Option<std::sync::Arc<Pcm>>,
    pending_seek: Option<f64>,
    region: Option<(f64, f64)>,
    error: Option<String>,
    loaded: bool,
}

impl Player {
    pub fn new(_ctx: &egui::Context) -> Self {
        let el = HtmlAudioElement::new().ok();
        if let Some(el) = &el {
            el.set_preload("auto");
            el.set_volume(0.9);
        }
        let error = el.is_none().then(|| "this browser has no audio element".to_string());
        Player { el, object_url: None, pcm: None, pending_seek: None, region: None, error, loaded: false }
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn is_loading(&self) -> bool {
        self.loaded && self.el.as_ref().is_some_and(|el| el.ready_state() < HAVE_METADATA && el.error().is_none())
    }

    pub fn load(&mut self, source: Source, keep_position: bool) {
        let Some(el) = &self.el else { return };
        let was_playing = !el.paused();
        let pos = if keep_position { self.position() } else { 0.0 };
        self.pcm = None;
        let url = match source {
            // the site's policy lets media play from blob: URLs only, so a file handed over in its URL (the demo
            // engine's pieces) is handed to the browser as one
            Source::Url(url) if url.starts_with("data:") => match data_object_url(&url) {
                Ok(url) => url,
                Err(e) => {
                    self.error = Some(format!("could not hand the audio to the browser: {e:?}"));
                    return;
                }
            },
            Source::Url(url) => url,
            Source::File(file) => match object_url(&file.bytes, media_type(&file.name)) {
                Ok(url) => url,
                Err(e) => {
                    self.error = Some(format!("could not hand the audio to the browser: {e:?}"));
                    return;
                }
            },
            Source::Pcm(pcm) => match object_url(&pcm.encode_wav(), "audio/wav") {
                Ok(url) => {
                    self.pcm = Some(pcm);
                    url
                }
                Err(e) => {
                    self.error = Some(format!("could not hand the audio to the browser: {e:?}"));
                    return;
                }
            },
        };
        el.set_src(&url);
        if let Some(old) = self.object_url.take() {
            let _ = Url::revoke_object_url(&old);
        }
        if url.starts_with("blob:") {
            self.object_url = Some(url);
        }
        self.loaded = true;
        self.error = None;
        self.pending_seek = (pos > 0.0).then_some(pos);
        if was_playing {
            self.play();
        }
    }

    /// forget the loaded audio (the song it belonged to is gone)
    pub fn clear(&mut self) {
        if let Some(el) = &self.el {
            let _ = el.pause();
            let _ = el.remove_attribute("src");
            el.load();
        }
        if let Some(old) = self.object_url.take() {
            let _ = Url::revoke_object_url(&old);
        }
        self.pcm = None;
        self.loaded = false;
        self.pending_seek = None;
    }

    pub fn tick(&mut self) {
        let Some(el) = &self.el else { return };
        if let Some(err) = el.error() {
            self.error = Some(format!("the browser could not play the audio (media error {})", err.code()));
        }
        if el.ready_state() >= HAVE_METADATA {
            if let Some(t) = self.pending_seek.take() {
                let d = el.duration();
                let t = if d.is_finite() && t >= d { 0.0 } else { t };
                // where the browser started it already, as it was asked to (`seek`), it stays
                if (el.current_time() - t).abs() > 0.01 {
                    el.set_current_time(t);
                }
            }
            if let Some((a, b)) = self.region
                && b > a
                && el.current_time() >= b
            {
                el.set_current_time(a);
            }
        }
    }

    pub fn has_audio(&self) -> bool {
        self.loaded
    }

    /// the audio loaded, when it was handed over as audio rather than a URL: what a take is recorded over
    pub fn pcm(&self) -> Option<std::sync::Arc<Pcm>> {
        self.pcm.clone()
    }

    pub fn play(&mut self) {
        if let Some(el) = &self.el
            && self.loaded
            && let Ok(promise) = el.play()
        {
            // a rejected play() (autoplay policy) must not surface as an uncaught error
            wasm_bindgen_futures::spawn_local(async move {
                let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
            });
        }
    }

    pub fn pause(&mut self) {
        if let Some(el) = &self.el {
            let _ = el.pause();
        }
    }

    pub fn stop(&mut self) {
        self.pause();
        self.seek(0.0);
    }

    pub fn is_playing(&self) -> bool {
        self.el.as_ref().is_some_and(|el| !el.paused())
    }

    pub fn seek(&mut self, secs: f64) {
        let Some(el) = &self.el else { return };
        let secs = secs.max(0.0);
        // before its metadata the browser takes it as where to start (its default playback start position), so that
        // one played at once does not start from the beginning; `tick` sees to it once the metadata is there
        el.set_current_time(secs);
        self.pending_seek = (el.ready_state() < HAVE_METADATA).then_some(secs);
    }

    pub fn position(&self) -> f64 {
        self.pending_seek.or_else(|| self.el.as_ref().map(|el| el.current_time())).unwrap_or(0.0)
    }

    pub fn duration(&self) -> f64 {
        self.el.as_ref().map(|el| el.duration()).filter(|d| d.is_finite()).unwrap_or(0.0)
    }

    pub fn set_loop(&mut self, region: Option<(f64, f64)>) {
        let Some(el) = &self.el else { return };
        match region {
            Some((a, b)) if b > a => {
                el.set_loop(false);
                self.region = Some((a, b));
            }
            Some(_) => {
                el.set_loop(true);
                self.region = None;
            }
            None => {
                el.set_loop(false);
                self.region = None;
            }
        }
    }

    pub fn set_volume(&mut self, volume: f32) {
        if let Some(el) = &self.el {
            el.set_volume(volume.clamp(0.0, 1.0) as f64);
        }
    }
}

/// the media type of an audio file by its name's extension, as the browser is told it
fn media_type(name: &str) -> &'static str {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "mp3" => "audio/mpeg",
        "m4a" | "aac" | "mp4" => "audio/mp4",
        "ogg" | "oga" => "audio/ogg",
        "flac" => "audio/flac",
        "aif" | "aiff" => "audio/aiff",
        _ => "audio/wav",
    }
}

fn object_url(bytes: &[u8], kind: &str) -> Result<String, JsValue> {
    let array = js_sys::Uint8Array::from(bytes);
    let parts = js_sys::Array::of1(&array);
    let props = BlobPropertyBag::new();
    props.set_type(kind);
    let blob = Blob::new_with_u8_array_sequence_and_options(&parts, &props)?;
    Url::create_object_url_with_blob(&blob)
}

/// a `data:<kind>;base64,<file>` URL's file, as a blob: URL of the same kind
fn data_object_url(url: &str) -> Result<String, JsValue> {
    let (head, file) = url["data:".len()..].split_once(',').ok_or_else(|| JsValue::from_str("not a data URL"))?;
    let kind = head.strip_suffix(";base64").ok_or_else(|| JsValue::from_str("not a base64 data URL"))?;
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let bytes: Vec<u8> = window.atob(file)?.chars().map(|c| c as u8).collect();
    object_url(&bytes, kind)
}

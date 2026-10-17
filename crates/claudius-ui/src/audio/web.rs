use eframe::egui;
use wasm_bindgen::JsValue;
use web_sys::{Blob, BlobPropertyBag, HtmlAudioElement, Url};

use super::Source;

/// `HAVE_METADATA`: duration and seeking work from here on
const HAVE_METADATA: u16 = 1;

pub struct Player {
    el: Option<HtmlAudioElement>,
    /// object URL of generated audio, revoked when replaced
    object_url: Option<String>,
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
        Player { el, object_url: None, pending_seek: None, region: None, error, loaded: false }
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
        let url = match source {
            Source::Url(url) => url,
            Source::Pcm(pcm) => match wav_object_url(&pcm.encode_wav()) {
                Ok(url) => url,
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
        self.loaded = false;
        self.pending_seek = None;
    }

    pub fn tick(&mut self) {
        let Some(el) = &self.el else { return };
        if let Some(err) = el.error() {
            self.error = Some(format!("the browser could not play the render (media error {})", err.code()));
        }
        if el.ready_state() >= HAVE_METADATA {
            if let Some(t) = self.pending_seek.take() {
                let d = el.duration();
                el.set_current_time(if d.is_finite() && t >= d { 0.0 } else { t });
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
        if el.ready_state() >= HAVE_METADATA {
            el.set_current_time(secs.max(0.0));
            self.pending_seek = None;
        } else {
            self.pending_seek = Some(secs.max(0.0));
        }
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

fn wav_object_url(bytes: &[u8]) -> Result<String, JsValue> {
    let array = js_sys::Uint8Array::from(bytes);
    let parts = js_sys::Array::of1(&array);
    let props = BlobPropertyBag::new();
    props.set_type("audio/wav");
    let blob = Blob::new_with_u8_array_sequence_and_options(&parts, &props)?;
    Url::create_object_url_with_blob(&blob)
}

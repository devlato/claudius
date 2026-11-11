//! What the palette's words find in the demo, on the studio's own thread rather than the engine's: a search takes
//! microseconds, where the engine may be rendering a song for seconds. In the app, the session holds the model
//! (`claudius_session::meaning`, which downloads it once); in a page, the page fetches the site's copy (`studio/models/`,
//! which `cargo xtask site --sounds` makes) once and holds it. Until it is there, actions are found by name, and the
//! words asked about last are answered again once it has come.

use std::sync::Arc;

use claudius_api::Done;
use claudius_intent::{Answer, DONE, Finder, Item, Model, find_among};
use eframe::egui;

/// for how long after the last words the demo looks for the model to come, to answer them again (seconds)
const WAITS: f64 = 20.0;

/// the words the palette asked about last: how many actions, when, and whether they were found by meaning
#[derive(Default)]
pub struct Finding {
    last: Option<(String, usize, f64, bool)>,
}

impl Finding {
    /// What the words find, by name, and by meaning once the model is there (it is asked for the first time): the
    /// actions, and the changes in `history` (the state's) that the song can go back to before.
    pub fn find(
        &mut self,
        query: &str,
        limit: usize,
        history: &[Done],
        now: f64,
        ctx: Option<&egui::Context>,
    ) -> Answer {
        let (model, finder) = held(ctx);
        self.last = Some((query.to_string(), limit, now, model.is_some()));
        let reachable: Vec<Item> = history
            .iter()
            .filter(|d| d.can_go_back)
            .map(|d| Item { id: &d.id, name: &d.label, words: &d.said })
            .collect();
        Answer {
            query: query.to_string(),
            found: finder.find(query, model.as_deref(), limit),
            done: find_among(query, &reachable, model.as_deref(), DONE),
            by_meaning: finder.by_meaning(),
        }
    }

    /// the words asked about last again, once the model has come since they were found by name
    pub fn again(&mut self, history: &[Done], now: f64, ctx: Option<&egui::Context>) -> Option<Answer> {
        let (query, limit, at, false) = self.last.clone()? else { return None };
        if !ready() {
            if now - at < WAITS
                && let Some(ctx) = ctx
            {
                ctx.request_repaint_after(std::time::Duration::from_millis(250));
            }
            return None;
        }
        Some(self.find(&query, limit, history, now, ctx))
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn held(_: Option<&egui::Context>) -> (Option<Arc<Model>>, Arc<Finder>) {
    claudius_session::meaning::fetch_in_background();
    claudius_session::meaning::held()
}

#[cfg(not(target_arch = "wasm32"))]
fn ready() -> bool {
    claudius_session::meaning::ready()
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    static HELD: std::cell::RefCell<Option<(Arc<Model>, Arc<Finder>)>> = const { std::cell::RefCell::new(None) };
    static ASKED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(target_arch = "wasm32")]
fn held(ctx: Option<&egui::Context>) -> (Option<Arc<Model>>, Arc<Finder>) {
    use claudius_intent::catalogue::ENTRIES;
    if !ASKED.replace(true) {
        let url = format!("models/{}.clde", claudius_intent::model::POTION.name);
        let ctx = ctx.cloned();
        ehttp::fetch(ehttp::Request::get(&url), move |res| {
            let model = res
                .and_then(|r| if r.ok { Ok(r.bytes) } else { Err(format!("{} {}", r.status, r.status_text)) })
                .and_then(|bytes| Model::from_bytes(&bytes));
            match model {
                Ok(model) => {
                    let finder = Finder::new(ENTRIES, Some(&model));
                    HELD.set(Some((Arc::new(model), Arc::new(finder))));
                    if let Some(ctx) = ctx {
                        ctx.request_repaint();
                    }
                }
                Err(why) => log::warn!("the palette finds actions by name: {url}: {why}"),
            }
        });
    }
    match HELD.with_borrow(|h| h.clone()) {
        Some((model, finder)) => (Some(model), finder),
        None => (None, Arc::new(Finder::new(ENTRIES, None))),
    }
}

#[cfg(target_arch = "wasm32")]
fn ready() -> bool {
    HELD.with_borrow(|h| h.is_some())
}

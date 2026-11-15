//! What the person means, found on the device: the studio's actions (`catalogue`), found from a few of their words, the
//! way they would say them as well as by name (`find`), by static word embeddings (`model`: a vector learned for each
//! piece of a word, a phrase the mean of its pieces', no network and no graphics card to run them) in a vector index
//! held in memory (`index`). A phrase is embedded in microseconds, and the model is 7.6 MB, small enough for a laptop
//! of ten years ago and for a browser. Requests in plain words ("undo the last five changes", "more reverb on the
//! snare") are read into what claudius would do for them, for the person to confirm (`ask`), and questions about the
//! song ("what's the bass volume") answered from it (`tell`). The studio's search fields find genres and sounds by
//! meaning the same way (`search`).

#![forbid(unsafe_code)]

pub mod ask;
pub mod catalogue;
pub mod find;
pub mod index;
pub mod model;
pub mod needle;
pub mod pieces;
pub mod search;
pub mod spoken;
pub mod teach;
pub mod tell;
pub mod typos;

pub use ask::{Asker, Proposal};
pub use find::{Answer, By, DONE, Finder, Found, Item, answer, find_among, nearness};
pub use model::Model;
pub use tell::{Told, asking, tell};

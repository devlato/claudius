//! What the person means, found on the device: the studio's actions (`catalogue`), found from a few of their words, the
//! way they would say them as well as by name (`find`), by static word embeddings (`model`: a vector learned for each
//! piece of a word, a phrase the mean of its pieces', no network and no graphics card to run them) in a vector index
//! held in memory (`index`). A phrase is embedded in microseconds, and the model is 7.6 MB, small enough for a laptop
//! of ten years ago and for a browser.

#![forbid(unsafe_code)]

pub mod catalogue;
pub mod find;
pub mod index;
pub mod model;
pub mod pieces;

pub use find::{Answer, By, DONE, Finder, Found, Item, find_among};
pub use model::Model;

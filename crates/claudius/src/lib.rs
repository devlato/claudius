//! claudius: an automatic beatmaker that composes like a producer in a DAW.
//!
//! Every sound is real audio: recorded loops and one-shots (Apple Loops, sample folders, chops of any song) or
//! procedural synthesis rendered in code. Music theory writes the parts into a blueprint, the composer arranges it
//! into a project (tracks, clips, notes, effects, automation), the renderer mixes and masters it, a critic listens,
//! and an evolutionary loop mutates blueprints towards better songs.

#![forbid(unsafe_code)]
// DSP code reads best with index loops over parallel buffers and with the full parameter lists of its processors
#![allow(
    clippy::needless_range_loop,
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::should_implement_trait
)]

pub mod analysis;
pub mod audio;
pub mod audition;
pub mod compose;
pub mod config;
pub mod dsp;
pub mod ears;
pub mod evolve;
pub mod fx;
pub mod genres;
pub mod judge;
pub mod library;
pub mod loudness;
pub mod model;
pub mod pattern;
pub mod render;
pub mod rng;
pub mod script;
pub mod song;
pub mod synth;
pub mod theory;
pub mod warp;

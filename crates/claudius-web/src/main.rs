//! `claudius-engine`: the claudius engine in a browser's Web Worker, which the web studio's demo starts. Each message
//! either way is an object of `json`, a JSON text, and `audio`, sound handed over rather than copied (`Float32Array`s,
//! interleaved stereo at 48 kHz). The page sends a command at a time (`{"what": "set_tempo", "command": {...}}`), the
//! loops it found for a genre built on samples (`{"loops": [{"prepared": {...}, "credit": "..."}]}`, their audio in
//! the same order) or asks for the script reference (`{"reference": true}`). The worker starts with the state and the
//! sounds (`{"state": ..., "sounds": ...}`), answers each command with its reply and the state after it, renders a
//! song newer than its render and sends the state with the master as its audio, and sends the reference when asked.
//! Where the browser has the cores, a crew of copies of it makes a render's tracks side by side first (`crew`).

#![forbid(unsafe_code)]

#[cfg(target_arch = "wasm32")]
mod crew;
#[cfg(target_arch = "wasm32")]
mod worker;

fn main() {
    #[cfg(target_arch = "wasm32")]
    worker::start();
    #[cfg(not(target_arch = "wasm32"))]
    eprintln!("claudius-engine runs in a browser's Web Worker: the web studio builds it for wasm32-unknown-unknown");
}

//! Any text given to the mini-notation parser either parses or returns an error, and a parsed pattern answers a
//! query (or reports that it is too dense) without panicking or running away.

#![no_main]

use claudius::pattern::{Kind, Pattern, mini};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(src) = std::str::from_utf8(data) else { return };
    if let Ok(node) = mini::parse(src) {
        let p = Pattern::new(node, Kind::Sound, String::new());
        let _ = p.try_events(0.0, 4.0);
        let _ = p.try_events(-1.5, 0.25);
    }
});

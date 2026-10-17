//! Any text run as a script either runs or returns an error: the sandbox (operation, time, size and pattern limits)
//! keeps it from panicking, hanging or exhausting memory.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(code) = std::str::from_utf8(data) else { return };
    let _ = claudius::script::run(code, None, None);
});

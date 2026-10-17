//! ONNX Runtime's prebuilt static library for Apple targets contains the CoreML provider, which calls
//! `__isPlatformVersionAtLeast` (an `@available` check). That symbol lives in clang's compiler runtime, which rustc
//! does not link (`-nodefaultlibs`), so link it here; the directive propagates to every binary using this crate.

use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_VENDOR").as_deref() != Ok("apple") {
        return;
    }
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let lib = match os.as_str() {
        "macos" => "clang_rt.osx",
        "ios" => "clang_rt.ios",
        _ => return,
    };
    let clang = std::env::var("CC").unwrap_or_else(|_| "clang".into());
    let dir = Command::new(&clang)
        .arg("--print-runtime-dir")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|d| std::path::Path::new(d).join(format!("lib{lib}.a")).exists());
    match dir {
        Some(d) => {
            println!("cargo:rustc-link-search=native={d}");
            println!("cargo:rustc-link-lib=static={lib}");
        }
        None => println!("cargo:warning=earshot: clang runtime lib{lib}.a not found; linking ONNX Runtime may fail"),
    }
}

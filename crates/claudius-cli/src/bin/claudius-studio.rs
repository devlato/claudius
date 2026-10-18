//! The studio for the Windows Start menu. Windows gives every console program a console window, and `claudius` is
//! one, so this program is built as a window program and starts `claudius studio` without a console. Elsewhere it
//! does the same as `claudius studio`.

#![cfg_attr(windows, windows_subsystem = "windows")]
#![forbid(unsafe_code)]

use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let name = if cfg!(windows) { "claudius.exe" } else { "claudius" };
    let Some(claudius) = std::env::current_exe().ok().and_then(|exe| exe.parent().map(|dir| dir.join(name))) else {
        return ExitCode::FAILURE;
    };
    let mut studio = Command::new(claudius);
    studio.arg("studio").args(std::env::args_os().skip(1));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        studio.creation_flags(CREATE_NO_WINDOW);
    }
    match studio.status() {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        _ => ExitCode::FAILURE,
    }
}

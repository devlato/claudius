//! `claudius-ui [--mock | --server URL | URL]`: the studio as its own program (native), or the wasm entry point
//! that trunk builds for the web.

#![forbid(unsafe_code)]

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let mut server = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--mock" | "--demo" => server = Some("mock".to_string()),
            "--server" => server = args.next(),
            "-h" | "--help" => {
                println!(
                    "claudius studio\n\nusage: claudius-ui [--mock | --server URL | URL]\n\n  --mock          run against the built-in demo (also: CLAUDIUS_MOCK=1)\n  --server URL    the claudius server (also: CLAUDIUS_SERVER; default http://127.0.0.1:7878)"
                );
                return Ok(());
            }
            other if !other.starts_with('-') => server = Some(other.to_string()),
            other => {
                eprintln!("unknown argument {other} (try --help)");
                std::process::exit(2);
            }
        }
    }
    claudius_ui::run_native(server)
}

#[cfg(target_arch = "wasm32")]
fn main() {
    eframe::WebLogger::init(log::LevelFilter::Info).ok();
    wasm_bindgen_futures::spawn_local(async {
        if let Err(e) = claudius_ui::start_web("claudius_canvas").await {
            log::error!("claudius studio could not start: {e:?}");
        }
    });
}

#![forbid(unsafe_code)]
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;

use claudius::compose::{self, Options};
use claudius::ears::Ears;
use claudius::genres::GenreProfile;
use claudius::judge::EarsReport;
use claudius::model::{Blueprint, Project};
use claudius::render::Rendered;
use claudius::theory::scale::Key;
use claudius::{genres, song};

mod args;
mod connect;
mod doctor;
mod report;
mod setup;
mod skills;

use args::*;
use report::*;

fn seed_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64 % 1_000_000)
}

/// earshot's models (loaded once per run) and the embeddings of any reference tracks
struct Listening {
    ears: Option<Ears>,
    refs: Option<Vec<Vec<f32>>>,
}

fn listen(a: &EarsArgs) -> Listening {
    if a.no_ears {
        return Listening { ears: None, refs: None };
    }
    let t = std::time::Instant::now();
    match Ears::load(a.tags) {
        Ok(e) => {
            let refs = if a.refs.is_empty() {
                None
            } else {
                match e.embed_refs(&a.refs) {
                    Ok(r) => Some(r),
                    Err(err) => {
                        eprintln!("could not read the reference tracks: {err}");
                        None
                    }
                }
            };
            eprintln!(
                "ears loaded in {:.1} s{}",
                t.elapsed().as_secs_f64(),
                refs.as_ref().map(|r| format!(", {} reference tracks", r.len())).unwrap_or_default()
            );
            Listening { ears: Some(e), refs }
        }
        Err(err) => {
            eprintln!("earshot is unavailable ({err}); judging with meters and theory only");
            Listening { ears: None, refs: None }
        }
    }
}

impl Listening {
    fn report(&self, r: &Rendered, genre: Option<&GenreProfile>, realness: bool) -> Option<EarsReport> {
        let e = self.ears.as_ref()?;
        let g = genre.map(|g| (g.query.as_str(), g.alts.as_slice()));
        match e.report(&r.master, g, self.refs.as_deref(), realness) {
            Ok(rep) => Some(rep),
            Err(err) => {
                eprintln!("the ears failed on this render: {err}");
                None
            }
        }
    }
}

/// a random token for a shared session: 128 bits from the operating system, as hex
fn new_token() -> Result<String> {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).map_err(|e| anyhow::anyhow!("the operating system gave no randomness for a token: {e}"))?;
    Ok(b.iter().map(|x| format!("{x:02x}")).collect())
}

/// --token or CLAUDIUS_TOKEN, where a blank one (a variable left empty) counts as none
fn given_token(token: Option<String>) -> Option<String> {
    token.map(|t| t.trim().to_owned()).filter(|t| !t.is_empty())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Make { genre, seed, bpm, key, short, stems, samples, sound, sample, loops, ears, out } => {
            let g = genres::get(&genre)?;
            let seed = seed.unwrap_or_else(seed_now);
            let key = key.map(|k| Key::parse(&k).with_context(|| format!("cannot read key '{k}'"))).transpose()?;
            let opts = Options {
                bpm,
                key,
                short,
                samples,
                loops: sound.library_on(),
                family: loops && sound.library_on(),
                ..Default::default()
            };
            let listening = listen(&ears);
            let t = std::time::Instant::now();
            let lib = sound.library()?;
            let lib_ref = lib.as_ref().map(|l| l as &dyn compose::Library);
            let mut bp = compose::generate(&g, seed, &opts, lib_ref);
            let mut rng = claudius::rng::Rng::new(seed ^ 0x5a);
            for s in &sample {
                let known = claudius::library::user_entries()
                    .into_iter()
                    .find(|e| std::path::Path::new(&e.path) == s.as_path());
                let (sbpm, beats, skey) = match known {
                    Some(e) if e.tempo > 0.0 => (e.tempo, e.beats, e.key.map(|k| (k, e.minor))),
                    _ => {
                        let x = claudius::audio::load(s)?;
                        let a = claudius::analysis::song::analyze(&x, None);
                        let beats = ((x.secs() / (60.0 / a.bpm) / 4.0).floor().clamp(1.0, 4.0)) * 4.0;
                        (a.bpm, beats, Some((a.key.tonic, a.key.mode.is_minor())))
                    }
                };
                compose::add_sample(&mut bp, &s.to_string_lossy(), sbpm, beats, skey, &mut rng);
            }
            let dir = out.join(format!("{}-{}-{seed}", g.name, song::slug(&bp.title)));
            let f = |_: &Project, r: &Rendered| listening.report(r, Some(&g), true);
            let (_, r, w, v) = song::render_and_write(&bp, &dir, stems, Some(&f))?;
            print_summary(&bp, &r, &w, &v, t.elapsed().as_secs_f64());
        }
        Cmd::Evolve {
            genre,
            from,
            seed,
            start,
            generations,
            population,
            children,
            preview_bars,
            keep,
            samples,
            sound,
            loops,
            ears,
            out,
        } => {
            let g = genres::get(&genre)?;
            let seed = seed.unwrap_or_else(seed_now);
            let listening = listen(&ears);
            let lib = sound.library()?;
            let lib_ref = lib.as_ref().map(|l| l as &dyn compose::Library);
            let opts = Options {
                samples,
                loops: sound.library_on(),
                family: loops && sound.library_on(),
                ..Default::default()
            };
            let initial: Vec<Blueprint> = match &from {
                Some(p) => vec![serde_json::from_str(&std::fs::read_to_string(p)?)?],
                None => (0..start.max(1)).map(|i| compose::generate(&g, seed + i as u64, &opts, lib_ref)).collect(),
            };
            let settings = claudius::evolve::Settings {
                generations,
                population,
                children,
                preview_bars,
                seed,
                crossover: 0.2,
                cancel: None,
            };
            let dir = out.join(format!("evolve-{}-{seed}", g.name));
            std::fs::create_dir_all(&dir)?;
            println!(
                "evolving {} ({} start, {generations} generations x {children} children, {preview_bars}-bar previews{})",
                g.name,
                initial.len(),
                if listening.ears.is_some() { ", with ears" } else { ", without ears" }
            );
            let t = std::time::Instant::now();
            let log = |s: &str| println!("{s}");
            let audition = |_: &Project, r: &Rendered| listening.report(r, Some(&g), false);
            let ears_ref: Option<claudius::evolve::Ears> =
                listening.ears.as_ref().map(|_| &audition as claudius::evolve::Ears);
            let all = claudius::evolve::run(&g, initial, lib_ref, ears_ref, &settings, &log)?;
            std::fs::write(dir.join("evolution.json"), serde_json::to_string_pretty(&all)?)?;
            println!(
                "searched {} candidates in {:.0} s; rendering the best {keep} in full",
                all.len(),
                t.elapsed().as_secs_f64()
            );
            let full = |_: &Project, r: &Rendered| listening.report(r, Some(&g), true);
            for (rank, c) in all.iter().take(keep).enumerate() {
                let d = dir.join(format!("{}-{}-{}", rank + 1, c.id, song::slug(&c.blueprint.title)));
                let (_, r, w, v) = song::render_and_write(&c.blueprint, &d, false, Some(&full))?;
                println!(
                    "#{} {} ({}): preview {:.1}, full song {:.1}",
                    rank + 1,
                    c.id,
                    c.blueprint.title,
                    c.score,
                    v.total
                );
                for l in c.blueprint.lineage.iter().skip(1) {
                    println!("    {l}");
                }
                print_summary(&c.blueprint, &r, &w, &v, 0.0);
            }
        }
        Cmd::Mutate { blueprint, n, seed, ears, out } => {
            let bp = song::read_blueprint(&blueprint)?;
            let g = genres::get(&bp.genre)?;
            let listening = listen(&ears);
            let lib = open_library()?;
            let lib_ref = lib.as_ref().map(|l| l as &dyn compose::Library);
            let mut rng = claudius::rng::Rng::new(seed.unwrap_or_else(seed_now));
            let dir = out.unwrap_or_else(|| {
                blueprint.parent().map(|p| p.join("mutations")).unwrap_or_else(|| PathBuf::from("out/mutations"))
            });
            let f = |_: &Project, r: &Rendered| listening.report(r, Some(&g), false);
            for i in 0..n {
                let (child, what) = claudius::evolve::mutate(&bp, &g, lib_ref, &[], &mut rng);
                let d = dir.join(format!("{}", i + 1));
                let (_, _, w, v) = song::render_and_write(&child, &d, false, Some(&f))?;
                println!("{}: {what} -> {:.1} ({})", i + 1, v.total, w.dir.display());
            }
        }
        Cmd::Analyze { file, genre, json } => {
            let range = genre.map(|g| genres::get(&g)).transpose()?.map(|g| g.bpm);
            let x = claudius::audio::load(&file)?;
            let a = claudius::analysis::song::analyze(&x, range);
            if json {
                println!("{}", serde_json::to_string_pretty(&a)?);
            } else {
                print_analysis(&a);
            }
        }
        Cmd::Remake { file, genre, seed, ears, out } => {
            let g = genres::get(&genre)?;
            let seed = seed.unwrap_or_else(seed_now);
            let listening = listen(&ears);
            let x = claudius::audio::load(&file)?;
            let a = claudius::analysis::song::analyze(&x, None);
            print_analysis(&a);
            let lib = open_library()?;
            let lib_ref = lib.as_ref().map(|l| l as &dyn compose::Library);
            let bp = claudius::analysis::song::to_blueprint(
                &a,
                &g,
                seed,
                &Options::default(),
                lib_ref,
                &file.to_string_lossy(),
            );
            let dir = out.join(format!("remake-{}-{}-{seed}", g.name, song::slug(&bp.title)));
            let t = std::time::Instant::now();
            let f = |_: &Project, r: &Rendered| listening.report(r, Some(&g), true);
            let (_, r, w, v) = song::render_and_write(&bp, &dir, false, Some(&f))?;
            print_summary(&bp, &r, &w, &v, t.elapsed().as_secs_f64());
        }
        Cmd::Chop { file, bars, genre, find, top, out } => {
            let range = genre.map(|g| genres::get(&g)).transpose()?.map(|g| g.bpm);
            let c = claudius::library::chop::chop(&file, bars, &out, range)?;
            print_analysis(&c.analysis);
            println!("chopped {} loops and {} one-shots (added to the user library):", c.loops.len(), c.oneshots.len());
            for l in &c.loops {
                println!(
                    "  {} ({:.0} BPM, {} beats{})",
                    l.path,
                    l.tempo,
                    l.beats,
                    l.key.map(|k| format!(", key {}", claudius::theory::scale::pc_name(k, false))).unwrap_or_default()
                );
            }
            for (lane, p) in &c.oneshots {
                println!("  {lane}: {}", p.display());
            }
            #[cfg(feature = "ears")]
            if !find.is_empty() {
                let listening = listen(&EarsArgs::default());
                let Some(e) = &listening.ears else { anyhow::bail!("--find needs earshot's models") };
                for q in &find {
                    let found = claudius::library::chop::find(e, &file, q, top, &out)?;
                    println!("'{q}':");
                    for (p, t) in &found {
                        println!("  {} ({:.2}-{:.2} s, z {:+.1} {})", p.display(), t.start, t.end, t.z, t.verdict);
                    }
                }
            }
            #[cfg(not(feature = "ears"))]
            if !find.is_empty() {
                let _ = top;
                anyhow::bail!("--find listens for what you describe, and this build of claudius has no ears");
            }
        }
        Cmd::Render { blueprint, stems, ears, out } => {
            let bp = song::read_blueprint(&blueprint)?;
            let g = genres::get(&bp.genre).ok();
            let listening = listen(&ears);
            let dir = out.unwrap_or_else(|| blueprint.parent().map(|p| p.to_path_buf()).unwrap_or_default());
            let t = std::time::Instant::now();
            let f = |_: &Project, r: &Rendered| listening.report(r, g.as_ref(), true);
            let (_, r, w, v) = song::render_and_write(&bp, &dir, stems, Some(&f))?;
            print_summary(&bp, &r, &w, &v, t.elapsed().as_secs_f64());
        }
        Cmd::Judge { file, genre, ears, json } => {
            let listening = listen(&ears);
            let project_genre = file
                .parent()
                .map(|d| d.join("project.json"))
                .and_then(|p| std::fs::read_to_string(p).ok())
                .and_then(|s| serde_json::from_str::<Project>(&s).ok())
                .map(|p| p.genre);
            let g = genre.or(project_genre).filter(|s| !s.is_empty()).map(|s| genres::get(&s)).transpose()?;
            let f = |_: &Project, r: &Rendered| listening.report(r, g.as_ref(), true);
            let v = song::judge_file(&file, Some(&f))?;
            if json {
                println!("{}", serde_json::to_string_pretty(&v)?);
            } else {
                println!("{}: {}", file.display(), v.summary);
                if let Some(e) = &v.ears {
                    print_ears(e);
                }
                for (k, m) in &v.measures {
                    println!("  {k}: {}", (m * 100.0).round() / 100.0);
                }
                for i in &v.issues {
                    println!("  - [{:.2}] {} ({})", i.severity, i.what, i.target);
                }
            }
        }
        Cmd::Genres => {
            for g in genres::all()? {
                println!("{:12} {:>3.0}-{:<3.0} BPM  {}", g.name, g.bpm.0, g.bpm.1, g.description);
            }
        }
        Cmd::Sounds { out } => {
            let t = std::time::Instant::now();
            let files = claudius::audition::render_all(&out)?;
            println!("rendered {} sounds into {} in {:.1} s", files.len(), out.display(), t.elapsed().as_secs_f64());
        }
        Cmd::Script { file, on, print, ears, out } => {
            let code = if file.as_os_str() == "-" {
                let mut s = String::new();
                std::io::Read::read_to_string(&mut std::io::stdin(), &mut s)?;
                s
            } else {
                std::fs::read_to_string(&file).with_context(|| format!("cannot read {}", file.display()))?
            };
            let base: Option<Blueprint> = match &on {
                Some(p) => {
                    let p = if p.is_dir() { p.join("blueprint.json") } else { p.clone() };
                    Some(serde_json::from_str(
                        &std::fs::read_to_string(&p).with_context(|| format!("cannot read {}", p.display()))?,
                    )?)
                }
                None => None,
            };
            let lib = open_library()?.map(std::sync::Arc::new);
            let result = claudius::script::run(&code, base, lib).map_err(|e| anyhow::anyhow!("{e}"))?;
            for line in &result.log {
                eprintln!("{line}");
            }
            let Some(bp) = result.bp else {
                anyhow::bail!("the script made no song (start it with song(\"house\") or blank(\"house\"))")
            };
            if print {
                print!("{}", claudius::script::to_script(&bp));
            } else {
                let g = genres::get(&bp.genre)?;
                let listening = listen(&ears);
                let t = std::time::Instant::now();
                let dir = out.join(format!("{}-{}-{}", bp.genre, song::slug(&bp.title), bp.seed));
                let f = |_: &Project, r: &Rendered| listening.report(r, Some(&g), true);
                let (_, r, w, v) = song::render_and_write(&bp, &dir, false, Some(&f))?;
                print_summary(&bp, &r, &w, &v, t.elapsed().as_secs_f64());
            }
        }
        Cmd::Reference => print!("{}", claudius::script::REFERENCE),
        Cmd::Setup { yes, no_packs, no_models, tags } => setup::run(yes, !no_packs, !no_models, tags)?,
        Cmd::Doctor { port } => doctor::run(port, web_dir(std::env::var_os("CLAUDIUS_WEB").map(PathBuf::from))),
        Cmd::Connect { client, http, remove, no_skills, port } => connect::run(client, http, remove, port, !no_skills)?,
        Cmd::Skills { out } => {
            skills::install(&out)?;
            skills::zips(&out)?;
            println!(
                "wrote the claudius skills ({}) to {}: a folder each, for assistants that read SKILL.md folders, and a \
                 zip each, to upload in Claude's settings under Capabilities",
                skills::names().join(", "),
                out.display()
            );
        }
        Cmd::Studio { app } => {
            use claudius_server::mcp::Probe;
            let addr = std::net::SocketAddr::from(([127, 0, 0, 1], app.port));
            let url = match claudius_server::mcp::probe(&format!("http://{addr}"), &None) {
                Probe::Claudius => {
                    eprintln!("a claudius is already running on port {}: opening the studio on it", app.port);
                    format!("http://{addr}")
                }
                Probe::NeedsToken => anyhow::bail!(
                    "the claudius on port {} asks for a token, which the studio window cannot send: open \
                     http://{addr}/?token=<token> in a browser, or start another studio with --port",
                    app.port
                ),
                Probe::Nothing => {
                    let session = app.app();
                    let local = claudius_server::spawn(
                        session,
                        addr,
                        claudius_server::Options { web: app.web(), ..Default::default() },
                    )?;
                    eprintln!("claudius engine at http://{local} (web UI, API, MCP at /mcp)");
                    format!("http://{local}")
                }
            };
            #[cfg(feature = "studio")]
            claudius_ui::run_native(Some(url)).map_err(|e| anyhow::anyhow!("the studio window failed: {e}"))?;
            #[cfg(not(feature = "studio"))]
            {
                eprintln!(
                    "this claudius was built without the studio window; open {url} in a browser (ctrl-c to stop)"
                );
                loop {
                    std::thread::park();
                }
            }
        }
        Cmd::Serve { app, host, public, token, files } => {
            let ip: std::net::IpAddr = host.parse().with_context(|| format!("not an IP address: {host}"))?;
            // a claudius others can reach always asks for a token (one is made when none is given): the Host check
            // stops web pages, not a program on the network that sends `Host: localhost`. It also keeps to the
            // shared folders, so that a token on the network is not a key to every file.
            let shared = public || !ip.is_loopback();
            let token = given_token(token);
            let made = (shared && token.is_none()).then(new_token).transpose()?;
            let token = token.or_else(|| made.clone());
            let web = app.web();
            let roots = (shared || !files.is_empty())
                .then(|| claudius::config::shared_roots().into_iter().chain(files).collect());
            let session = app.app_with(roots);
            let addr = std::net::SocketAddr::new(ip, app.port);
            let what = format!(
                "{}API at /api, MCP at /mcp{}",
                if web.is_some() {
                    "web UI at /, "
                } else {
                    "(web UI not built: trunk build --release in crates/claudius-ui), "
                },
                if token.is_some() { " (token required)" } else { "" }
            );
            let listening = move |mut addr: std::net::SocketAddr| {
                // listening on every address: this computer reaches it on loopback (browsers refuse 0.0.0.0)
                if addr.ip().is_unspecified() {
                    addr.set_ip(std::net::Ipv4Addr::LOCALHOST.into());
                }
                eprintln!("claudius at http://{addr}: {what}");
                if let Some(t) = &made {
                    eprintln!(
                        "shared with a new token: open http://{addr}/?token={t} (through your tunnel, its address \
                         with ?token={t}), or send Authorization: Bearer {t}. --token sets your own."
                    );
                }
            };
            tokio::runtime::Runtime::new()?.block_on(claudius_server::serve(
                session,
                addr,
                claudius_server::Options { web, public, token },
                listening,
            ))?;
        }
        Cmd::Mcp { app, url, token } => {
            let token = given_token(token);
            // stdout carries the protocol from here on: everything else goes to stderr
            let rt = tokio::runtime::Runtime::new()?;
            let base = url.clone().unwrap_or_else(|| format!("http://127.0.0.1:{}", app.port));
            let found = claudius_server::mcp::probe(&base, &token);
            let backend = if found == claudius_server::mcp::Probe::Claudius {
                eprintln!("claudius mcp: driving the claudius app at {base}");
                claudius_server::mcp::Backend::Remote { base, token }
            } else if found == claudius_server::mcp::Probe::NeedsToken {
                anyhow::bail!(
                    "the claudius at {base} asks for a token: pass --token or set CLAUDIUS_TOKEN (a wrong one is refused \
                     too)"
                );
            } else if url.is_some() {
                anyhow::bail!("no claudius answers at {base}");
            } else {
                let session = app.app();
                let addr = std::net::SocketAddr::from(([127, 0, 0, 1], app.port));
                let base = match claudius_server::spawn(
                    session.clone(),
                    addr,
                    claudius_server::Options { web: app.web(), ..Default::default() },
                ) {
                    Ok(local) => {
                        eprintln!("claudius mcp: a session of its own; open http://{local} to hear and see it");
                        format!("http://{local}")
                    }
                    Err(e) => {
                        eprintln!("claudius mcp: a session of its own (no web UI: {e})");
                        String::new()
                    }
                };
                claudius_server::mcp::Backend::Local { app: session, base }
            };
            rt.block_on(claudius_server::mcp::serve_stdio(backend))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_blank_token_is_no_token() {
        assert_eq!(super::given_token(Some(String::new())), None);
        assert_eq!(super::given_token(Some(" \t".into())), None);
        assert_eq!(super::given_token(Some(" s3cret\n".into())).as_deref(), Some("s3cret"));
        assert_eq!(super::given_token(None), None);
    }
}

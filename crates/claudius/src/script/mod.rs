//! claudius script: Rhai with Strudel-style patterns and song-editing commands.
//!
//! ```js
//! song("house", 7)                               // start from a generated song (or skip it to edit the current one)
//! tempo(124); key("A minor"); swing(0.54)
//! prog("A", "i7 VImaj7 iv7 v7")
//! track("hats", s("[~ hh]*4").gain(0.6).pan(0.2))
//! track("bass", note("<a1 f1 d2 e2>").struct("~ x ~ x ~ x ~ x").s("808"))
//! track("keys", chord("prog:A").struct("x ~ ~ x ~ ~ x ~").voicing().s("epiano").room(0.3))
//! track("lead", n("<[0 2 4 2] [4 6 7 6]>").s("pluck").delay(0.25).every(4, |x| x.fast(2)))
//! drum("kick", "x...x...x...x...")
//! part("bass", #{ style: "syncopated" })
//! mix("lead", #{ level: -2, reverb: 0.3 })
//! form([sec("intro", 8, "kick hats"), sec("drop", 16, "*").impact(), sec("outro", 8, "kick")])
//! ```
//!
//! Patterns are written in mini-notation inside strings (see `pattern::mini`); a song prints back as a script with
//! `to_script`, and running that script on the song changes nothing.

mod form;
mod mix;
mod patterns;
mod print;
mod song;
mod sounds;
mod steps;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use rhai::{Dynamic, Engine, EvalAltResult};

use crate::model::Blueprint;
use crate::pattern::{Node, Pattern, Value, fmt_num, mini, quote};

pub use form::Sec;
pub use mix::mix_of;
pub use print::to_script;
pub use song::{blank, remode};
pub use sounds::{drum_name, drum_names, drum_role, resolve_sound, role_of};
pub use steps::{STEP_ACCENT, STEP_GHOST, hits_to_steps, steps_to_hits};

/// a pattern track may play at most this many notes per bar (averaged over 64 bars)
pub const MAX_NOTES_PER_BAR: usize = 1024;

/// a script stops after this long
pub const MAX_SCRIPT_SECONDS: u64 = 10;

/// the language reference (markdown), for people and assistants
pub const REFERENCE: &str = include_str!("../script_reference.md");

type R<T> = Result<T, Box<EvalAltResult>>;

fn fail<T>(msg: impl Into<String>) -> R<T> {
    Err(msg.into().into())
}

fn num(d: &Dynamic) -> Option<f64> {
    d.as_float().ok().or_else(|| d.as_int().ok().map(|i| i as f64))
}

fn text(d: &Dynamic) -> Option<String> {
    d.clone().into_string().ok()
}

/// a parameter that may be a number, a mini-notation string or a pattern
fn param(d: &Dynamic) -> R<(Node, String)> {
    if let Some(x) = num(d) {
        return Ok((Node::Atom(Value::Num(x)), fmt_num(x)));
    }
    if let Some(s) = text(d) {
        let n = mini::parse(&s).map_err(|e| e.to_string())?;
        return Ok((n, quote(&s)));
    }
    if let Some(p) = d.clone().try_cast::<Pattern>() {
        return Ok((p.node(), p.repr.clone()));
    }
    fail("expected a number, a pattern string or a pattern")
}

fn constant(d: &Dynamic, what: &str) -> R<f64> {
    num(d).ok_or_else(|| format!("{what} needs a number").into())
}

/// an error when a pattern track's code no longer reads (a blueprint from another version, or edited by hand), rather
/// than the track going silent
pub fn check_patterns(bp: &Blueprint) -> Result<(), String> {
    for p in &bp.patterns {
        eval_pattern(&p.code).map_err(|e| format!("pattern track '{}' no longer reads: {e}", p.name))?;
    }
    Ok(())
}

/// evaluate one pattern expression (the code a pattern track stores)
pub fn eval_pattern(code: &str) -> Result<Pattern, String> {
    let mut engine = Engine::new();
    limits(&mut engine);
    patterns::register_patterns(&mut engine);
    engine.eval::<Pattern>(code).map_err(|e| e.to_string())
}

/// what a script may do: no files, no eval, bounded work, memory and time
fn limits(engine: &mut Engine) {
    engine.set_max_operations(5_000_000);
    engine.set_max_call_levels(64);
    engine.set_max_expr_depths(128, 64);
    engine.set_max_string_size(200_000);
    engine.set_max_array_size(10_000);
    engine.set_max_map_size(1_000);
    engine.set_module_resolver(rhai::module_resolvers::DummyModuleResolver::new());
    engine.disable_symbol("eval");
    let start = std::time::Instant::now();
    engine.on_progress(move |_| {
        (start.elapsed().as_secs() >= MAX_SCRIPT_SECONDS)
            .then(|| Dynamic::from(format!("the script ran for more than {MAX_SCRIPT_SECONDS} seconds")))
    });
}

pub struct Ctx {
    pub bp: Option<Blueprint>,
    pub lib: Option<Arc<crate::library::Library>>,
    pub log: Vec<String>,
    /// the folders scripts may read audio files from (None: anywhere)
    pub roots: Option<Vec<std::path::PathBuf>>,
}

pub struct Outcome {
    pub bp: Option<Blueprint>,
    pub log: Vec<String>,
}

/// run a script against `bp` (the current song, or None to start empty)
pub fn run(code: &str, bp: Option<Blueprint>, lib: Option<Arc<crate::library::Library>>) -> Result<Outcome, String> {
    run_with(code, bp, lib, None)
}

/// `run`, reading audio files only inside `roots` (see `config::path_allowed`)
pub fn run_with(
    code: &str,
    bp: Option<Blueprint>,
    lib: Option<Arc<crate::library::Library>>,
    roots: Option<Vec<std::path::PathBuf>>,
) -> Result<Outcome, String> {
    let ctx = Rc::new(RefCell::new(Ctx { bp, lib, log: Vec::new(), roots }));
    let mut engine = Engine::new();
    limits(&mut engine);
    patterns::register_patterns(&mut engine);
    song::register_song(&mut engine, ctx.clone());
    let printed: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let pr = printed.clone();
    engine.on_print(move |s| pr.borrow_mut().push(s.to_string()));
    let pr = printed.clone();
    engine.on_debug(move |s, _, _| pr.borrow_mut().push(s.to_string()));
    let result = engine.run(code);
    drop(engine);
    result.map_err(|e| e.to_string())?;
    let mut cx = Rc::try_unwrap(ctx).map_err(|_| "script context still borrowed".to_string())?.into_inner();
    if let Some(bp) = &cx.bp {
        bp.check_size()?;
    }
    cx.log.extend(printed.borrow().iter().cloned());
    Ok(Outcome { bp: cx.bp, log: cx.log })
}

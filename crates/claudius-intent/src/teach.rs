//! Teaching a small model on the device to read requests (`ask`), as Cactus's Needle reads them: the tools it is given,
//! one for each thing a request can ask, named in the words people use and each argument a span of the request
//! (`TOOLS`); lessons, a request each with the call it asks and the line of reasoning that derives each argument from its
//! span, made from many ways of saying each thing and from requests no tool serves, which call nothing (`lessons`); and a
//! call said again in the words the readings read (`said`), so that the song grounds what the model read as it grounds a
//! request: its tracks, its change in the history, its values from what the song has now.

use std::sync::LazyLock;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

/// The tools the model is given, one for each thing a request can ask. Tracks are given as said ("the toms",
/// "everything but the vocals"), and the readings find them in the song. Short, so that the tools, a request and its
/// answer fit the model's window of 1024 positions together, the tools read once.
pub const TOOLS: &str = concat!(
    r#"[{"name":"undo","description":"Undo the last changes","parameters":{"type":"object","properties":{"count":{"type":"integer","minimum":1}}}},"#,
    r#"{"name":"redo","description":"Redo undone changes","parameters":{"type":"object","properties":{"count":{"type":"integer","minimum":1}}}},"#,
    r#"{"name":"go_back","description":"Go back to before an earlier change","parameters":{"type":"object","properties":{"change":{"type":"string"}},"required":["change"]}},"#,
    r#"{"name":"set_volume","description":"Make tracks louder or quieter, or set them in dB","parameters":{"type":"object","properties":{"tracks":{"type":"string"},"way":{"type":"string","enum":["louder","quieter"]},"by_db":{"type":"number"},"to_db":{"type":"number"}},"required":["tracks"]}},"#,
    r#"{"name":"set_drive","description":"More or less drive or distortion on tracks","parameters":{"type":"object","properties":{"tracks":{"type":"string"},"way":{"type":"string","enum":["more","less"]},"to_percent":{"type":"number"}},"required":["tracks"]}},"#,
    r#"{"name":"set_reverb","description":"More or less reverb on tracks","parameters":{"type":"object","properties":{"tracks":{"type":"string"},"way":{"type":"string","enum":["more","less"]},"to_percent":{"type":"number"}},"required":["tracks"]}},"#,
    r#"{"name":"set_delay","description":"More or less delay or echo on tracks","parameters":{"type":"object","properties":{"tracks":{"type":"string"},"way":{"type":"string","enum":["more","less"]},"to_percent":{"type":"number"}},"required":["tracks"]}},"#,
    r#"{"name":"pan","description":"Pan tracks","parameters":{"type":"object","properties":{"tracks":{"type":"string"},"side":{"type":"string","enum":["left","right","center"]},"amount":{"type":"string","enum":["a bit","hard"]}},"required":["tracks","side"]}},"#,
    r#"{"name":"set_tempo","description":"Make the song faster or slower, or set its BPM","parameters":{"type":"object","properties":{"way":{"type":"string","enum":["faster","slower"]},"by_bpm":{"type":"number"},"bpm":{"type":"number"}}}},"#,
    r#"{"name":"set_key","description":"Move the song to another key","parameters":{"type":"object","properties":{"key":{"type":"string"}},"required":["key"]}},"#,
    r#"{"name":"swap_sound","description":"Give tracks another sound","parameters":{"type":"object","properties":{"tracks":{"type":"string"},"sound":{"type":"string"}},"required":["tracks"]}},"#,
    r#"{"name":"cut_track","description":"Take tracks out from a time on","parameters":{"type":"object","properties":{"tracks":{"type":"string"},"from":{"type":"string","description":"m:ss"}},"required":["tracks","from"]}},"#,
    r#"{"name":"mute","description":"Mute tracks","parameters":{"type":"object","properties":{"tracks":{"type":"string"}},"required":["tracks"]}},"#,
    r#"{"name":"unmute","description":"Unmute tracks","parameters":{"type":"object","properties":{"tracks":{"type":"string"}},"required":["tracks"]}},"#,
    r#"{"name":"solo","description":"Solo tracks","parameters":{"type":"object","properties":{"tracks":{"type":"string"}},"required":["tracks"]}},"#,
    r#"{"name":"remove_track","description":"Remove tracks from the song","parameters":{"type":"object","properties":{"tracks":{"type":"string"}},"required":["tracks"]}}]"#,
);

/// A tool call, as the model writes one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Call {
    pub name: String,
    #[serde(default)]
    pub arguments: Map<String, Value>,
}

/// A request, with the calls it asks (none when no tool serves it) and how each argument comes from its words.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Lesson {
    pub query: String,
    pub answers: Vec<Call>,
    pub reasoning: String,
}

impl Lesson {
    /// What the model reads and what it is to write, as Needle 3 is taught: the turn with the tools and the request,
    /// as the app asks it, and the reasoning line and the calls, closed.
    pub fn rendered(&self) -> (String, String) {
        let prompt = needle_infer::prompt::build_prompt(&self.query, TOOLS, None);
        let answers = serde_json::to_string(&self.answers).unwrap_or_else(|_| "[]".into());
        let target = format!("<think>\n{}\n</think>\n<tool_call>{answers}</tool_call><|im_end|>", self.reasoning);
        (prompt, target)
    }
}

/// The first call in the model's completion `text` for `query`, between its markers; none when it calls nothing, when
/// the call does not fit its tool, or when its reasoning quotes no words of the request or words the request does not
/// say, as a model guessing does.
pub fn read(query: &str, text: &str) -> Option<Call> {
    let payload = needle_infer::v3_engine::extract_tool_call(text)?;
    let mut call = serde_json::from_str::<Vec<Call>>(&payload).ok()?.into_iter().next()?;
    // an argument given as null is one not given
    call.arguments.retain(|_, v| !v.is_null());
    let reasoning = needle_infer::v3_engine::V3Engine::reasoning(text)?;
    let asked: Vec<String> = query.split_whitespace().map(word).collect();
    let quoted: Vec<&str> = quotes(reasoning).collect();
    let said = !quoted.is_empty() && quoted.iter().all(|q| says(&asked, q));
    (said && fits(&call)).then_some(call)
}

/// the words a line of reasoning quotes, each clause `'words' -> what they give`
fn quotes(reasoning: &str) -> impl Iterator<Item = &str> {
    reasoning.split("; ").filter_map(|clause| Some(clause.trim().strip_prefix('\'')?.split_once("' ->")?.0))
}

/// whether the request says every word of the quote, a phrase split around what it acts on ("slow the whole thing
/// down" says 'slow down') or turned about ("turn off the delay" says 'delay off') as well
fn says(asked: &[String], quote: &str) -> bool {
    quote.split_whitespace().map(word).all(|q| asked.contains(&q))
}

fn word(w: &str) -> String {
    w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase()
}

/// whether `call` names one of the tools and gives what it requires, each argument one of its parameters, of its
/// type, and among its values when it has a few
fn fits(call: &Call) -> bool {
    static TOOLED: LazyLock<Vec<Value>> = LazyLock::new(|| serde_json::from_str(TOOLS).unwrap_or_default());
    let Some(tool) = TOOLED.iter().find(|t| t["name"] == call.name.as_str()) else { return false };
    let parameters = &tool["parameters"];
    let required = parameters["required"].as_array().map(Vec::as_slice).unwrap_or_default();
    required.iter().all(|k| k.as_str().is_some_and(|k| call.arguments.contains_key(k)))
        && call.arguments.iter().all(|(k, v)| {
            let p = &parameters["properties"][k];
            match p["type"].as_str() {
                Some("integer") => {
                    number(v).is_some_and(|n| n.fract() == 0.0 && p["minimum"].as_f64().is_none_or(|m| n >= m))
                }
                Some("number") => number(v).is_some(),
                Some("string") => v.as_str().is_some_and(|s| {
                    !s.trim().is_empty() && p["enum"].as_array().is_none_or(|values| values.iter().any(|e| e == s))
                }),
                _ => false,
            }
        })
}

/// a number, written as one or as words that are one ("7", "40%")
fn number(v: &Value) -> Option<f64> {
    v.as_f64().or_else(|| v.as_str()?.trim_end_matches('%').trim().parse().ok())
}

/// A call said again in the words the readings read, which then find in the song what it names; none for a call of
/// no tool, or one without what it needs.
pub fn said(call: &Call) -> Option<String> {
    let a = &call.arguments;
    let text = |k: &str| a.get(k).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty());
    let num = |k: &str| a.get(k).and_then(number);
    let n = |x: f64| if x.fract() == 0.0 { format!("{x:.0}") } else { format!("{x}") };
    let tracks = || text("tracks");
    Some(match call.name.as_str() {
        "undo" => format!("undo the last {} changes", n(num("count").unwrap_or(1.0).max(1.0))),
        "redo" => format!("redo the last {}", n(num("count").unwrap_or(1.0).max(1.0))),
        "go_back" => format!("go back to before {}", text("change")?),
        "set_volume" => {
            let t = tracks()?;
            match (num("to_db"), num("by_db"), text("way")) {
                (Some(v), _, _) => format!("set {t} to {} db", n(v)),
                (_, Some(b), _) if b < 0.0 => format!("turn {t} down by {} db", n(-b)),
                (_, Some(b), _) => format!("turn {t} up by {} db", n(b)),
                (_, _, Some("quieter")) => format!("make {t} quieter"),
                _ => format!("make {t} louder"),
            }
        }
        name @ ("set_drive" | "set_reverb" | "set_delay") => {
            let t = tracks()?;
            let fx = &name[4..];
            match (num("to_percent"), text("way")) {
                (Some(v), _) if v <= 0.0 => format!("remove the {fx} from {t}"),
                (Some(v), _) => format!("set the {fx} on {t} to {}%", n(v.min(100.0))),
                (_, Some("less")) => format!("less {fx} on {t}"),
                _ => format!("more {fx} on {t}"),
            }
        }
        "pan" => {
            let t = tracks()?;
            match (text("side")?, text("amount")) {
                ("center", _) => format!("center {t}"),
                (side, Some(amount)) => format!("pan {t} {amount} {side}"),
                (side, None) => format!("pan {t} {side}"),
            }
        }
        "set_tempo" => match (num("bpm"), num("by_bpm"), text("way")) {
            (Some(b), _, _) => format!("set the tempo to {}", n(b)),
            (_, Some(b), _) if b < 0.0 => format!("slow it down by {} bpm", n(-b)),
            (_, Some(b), _) => format!("speed it up by {} bpm", n(b)),
            (_, _, Some("slower")) => "slow it down".into(),
            _ => "make it faster".into(),
        },
        "set_key" => format!("change the key to {}", text("key")?),
        "swap_sound" => match text("sound") {
            Some(sound) => format!("swap {} for {}", tracks()?, with_article(sound)),
            None => format!("try a different sound for {}", tracks()?),
        },
        "cut_track" => format!("cut {} from {}", tracks()?, text("from")?),
        "mute" => format!("mute {}", tracks()?),
        "unmute" => format!("unmute {}", tracks()?),
        "solo" => format!("solo {}", tracks()?),
        "remove_track" => format!("remove {} from the song", tracks()?),
        _ => return None,
    })
}

/// "a pluck", "an organ", and a sound said with its own article as it is
fn with_article(sound: &str) -> String {
    let lower = sound.to_lowercase();
    if ["a ", "an ", "the ", "some "].iter().any(|a| lower.starts_with(a)) {
        return sound.to_string();
    }
    let vowel = lower.starts_with(['a', 'e', 'i', 'o', 'u']) || lower.starts_with("8");
    format!("{} {sound}", if vowel { "an" } else { "a" })
}

/// Lessons made from many ways of saying each thing a request can ask, `each` of every way with other tracks and
/// values (a way with nothing to fill said again as often, so that each way weighs the same), and the requests no
/// tool serves; the same for the same `seed`.
pub fn lessons(seed: u64, each: usize) -> Vec<Lesson> {
    let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let mut out: Vec<Lesson> = Vec::new();
    for way in WAYS.iter().chain(MORE) {
        for _ in 0..each {
            let fill = Fill::draw(&mut rng);
            let query = fill.put(way.say);
            let mut arguments = Map::new();
            for (k, v) in way.args {
                arguments.insert(k.to_string(), fill.value(v));
            }
            let call = Call { name: way.tool.to_string(), arguments };
            out.push(Lesson { query, answers: vec![call], reasoning: fill.put(way.why) });
        }
    }
    // requests no tool serves, up to one in eight: the studio's actions, talk, and what is asked not to be done
    let nothing =
        |query: String| Lesson { query, answers: Vec::new(), reasoning: "No tool serves this request.".into() };
    let mut idle: Vec<Lesson> = NOTHING.iter().map(|q| nothing(q.to_string())).collect();
    let wanted = out.len() / 7;
    for _ in 0..wanted * 3 {
        if idle.len() >= wanted.max(NOTHING.len()) {
            break;
        }
        let fill = Fill::draw(&mut rng);
        let query = fill.put(rng.pick(NOT_ASKED));
        if !idle.iter().any(|l| l.query == query) {
            idle.push(nothing(query));
        }
    }
    out.extend(idle);
    out
}

/// what is said to the studio that no tool does: asked not to be done, wondered about, or done by an action
const NOT_ASKED: &[&str] = &[
    "don't mute {t}",
    "do not touch {t}",
    "leave {t} as it is",
    "never mind {t}",
    "i love {t}",
    "what is {t} playing",
    "how loud is {t}",
    "is {t} too loud",
    "why is {t} so quiet",
    "who made {t}",
    "what tempo is it",
    "what key is the song in, {key}?",
    "can you save the song",
    "can you play it from the top",
    "please render the song",
    "open the mixer for {t}",
    "show me {t} in the notes",
    "record a take over {t}",
    "export {t} as a stem",
    "make a new song in {key}",
    "start a new song at {bpm} bpm",
    "how do i swap a sound",
    "what does reverb do",
    "explain what {t} does",
];

/// One way of saying a request: its words, with the slots a fill puts in (`{t}` the tracks, `{n}` a small number,
/// `{db}` decibels, `{p}` a percent, `{bpm}` a tempo, `{time}` a time, `{sound}`, `{key}`, `{change}`), the tool it
/// asks, its arguments (each a slot or a value of its own), and the reasoning that derives them.
struct Way {
    say: &'static str,
    tool: &'static str,
    args: &'static [(&'static str, &'static str)],
    why: &'static str,
}

const fn way(
    say: &'static str,
    tool: &'static str,
    args: &'static [(&'static str, &'static str)],
    why: &'static str,
) -> Way {
    Way { say, tool, args, why }
}

const T: (&str, &str) = ("tracks", "{t}");

const WAYS: &[Way] = &[
    way("undo", "undo", &[], "'undo' -> undo"),
    way("undo that", "undo", &[], "'undo that' -> undo"),
    way("undo the last {n} changes", "undo", &[("count", "{n}")], "'last {n} changes' -> count {n#}"),
    way("take back the last {n} edits", "undo", &[("count", "{n}")], "'take back' -> undo; 'last {n}' -> count {n#}"),
    way("go back {n} steps", "undo", &[("count", "{n}")], "'go back {n} steps' -> undo, count {n#}"),
    way("revert the last {n} things i did", "undo", &[("count", "{n}")], "'revert the last {n}' -> undo, count {n#}"),
    way("oops, undo it", "undo", &[], "'undo it' -> undo"),
    way("redo", "redo", &[], "'redo' -> redo"),
    way("redo the last {n}", "redo", &[("count", "{n}")], "'redo the last {n}' -> count {n#}"),
    way("bring back what i undid", "redo", &[], "'bring back what i undid' -> redo"),
    way("undo my undo", "redo", &[], "'undo my undo' -> redo"),
    way("go back to before {change}", "go_back", &[("change", "{change}")], "'before {change}' -> change '{change}'"),
    way(
        "lets go back to before {change}",
        "go_back",
        &[("change", "{change}")],
        "'before {change}' -> change '{change}'",
    ),
    way("revert to before {change}", "go_back", &[("change", "{change}")], "'before {change}' -> change '{change}'"),
    way(
        "i liked it better before {change}",
        "go_back",
        &[("change", "{change}")],
        "'before {change}' -> change '{change}'",
    ),
    way(
        "undo everything since {change}",
        "go_back",
        &[("change", "{change}")],
        "'everything since {change}' -> change '{change}'",
    ),
    way("make {t} louder", "set_volume", &[T, ("way", "louder")], "'{t}' -> tracks; 'louder' -> louder"),
    way("turn up {t}", "set_volume", &[T, ("way", "louder")], "'turn up' -> louder; '{t}' -> tracks"),
    way("{t} too quiet", "set_volume", &[T, ("way", "louder")], "'too quiet' -> louder; '{t}' -> tracks"),
    way("i can barely hear {t}", "set_volume", &[T, ("way", "louder")], "'barely hear' -> louder; '{t}' -> tracks"),
    way("boost {t} a little", "set_volume", &[T, ("way", "louder")], "'boost' -> louder; '{t}' -> tracks"),
    way("make {t} quieter", "set_volume", &[T, ("way", "quieter")], "'{t}' -> tracks; 'quieter' -> quieter"),
    way("turn {t} down", "set_volume", &[T, ("way", "quieter")], "'turn down' -> quieter; '{t}' -> tracks"),
    way("{t} way too loud", "set_volume", &[T, ("way", "quieter")], "'too loud' -> quieter; '{t}' -> tracks"),
    way("pull {t} back a bit", "set_volume", &[T, ("way", "quieter")], "'pull back' -> quieter; '{t}' -> tracks"),
    way("turn {t} up by {n} db", "set_volume", &[T, ("by_db", "{n}")], "'{t}' -> tracks; 'up by {n} db' -> by_db {n#}"),
    way(
        "lower {t} by {n} db",
        "set_volume",
        &[T, ("by_db", "-{n}")],
        "'{t}' -> tracks; 'lower by {n} db' -> by_db -{n#}",
    ),
    way("{t} down {n}db", "set_volume", &[T, ("by_db", "-{n}")], "'{t}' -> tracks; 'down {n}db' -> by_db -{n#}"),
    way("set {t} to {db} db", "set_volume", &[T, ("to_db", "{db}")], "'{t}' -> tracks; 'to {db} db' -> to_db {db}"),
    way("put {t} at {db}db", "set_volume", &[T, ("to_db", "{db}")], "'{t}' -> tracks; 'at {db}db' -> to_db {db}"),
    way("more distortion on {t}", "set_drive", &[T, ("way", "more")], "'more distortion' -> more; '{t}' -> tracks"),
    way("make {t} dirtier", "set_drive", &[T, ("way", "more")], "'dirtier' -> more drive; '{t}' -> tracks"),
    way("add some grit to {t}", "set_drive", &[T, ("way", "more")], "'grit' -> more drive; '{t}' -> tracks"),
    way("less drive on {t}", "set_drive", &[T, ("way", "less")], "'less drive' -> less; '{t}' -> tracks"),
    way("clean up {t}", "set_drive", &[T, ("way", "less")], "'clean up' -> less drive; '{t}' -> tracks"),
    way(
        "set the overdrive on {t} to {p}%",
        "set_drive",
        &[T, ("to_percent", "{p}")],
        "'{t}' -> tracks; '{p}%' -> to_percent {p}",
    ),
    way(
        "lets increase overload for {t} to {p}%",
        "set_drive",
        &[T, ("to_percent", "{p}")],
        "'overload' -> drive; '{t}' -> tracks; '{p}%' -> to_percent {p}",
    ),
    way(
        "remove the distortion from {t}",
        "set_drive",
        &[T, ("to_percent", "0")],
        "'remove the distortion' -> to_percent 0; '{t}' -> tracks",
    ),
    way("more reverb on {t}", "set_reverb", &[T, ("way", "more")], "'more reverb' -> more; '{t}' -> tracks"),
    way("put {t} in a big hall", "set_reverb", &[T, ("way", "more")], "'big hall' -> more reverb; '{t}' -> tracks"),
    way("make {t} wetter", "set_reverb", &[T, ("way", "more")], "'wetter' -> more reverb; '{t}' -> tracks"),
    way("less reverb on {t}", "set_reverb", &[T, ("way", "less")], "'less reverb' -> less; '{t}' -> tracks"),
    way("dry up {t}", "set_reverb", &[T, ("way", "less")], "'dry up' -> less reverb; '{t}' -> tracks"),
    way("{t} drowning in reverb", "set_reverb", &[T, ("way", "less")], "'drowning in reverb' -> less; '{t}' -> tracks"),
    way(
        "set the reverb on {t} to {p}%",
        "set_reverb",
        &[T, ("to_percent", "{p}")],
        "'{t}' -> tracks; '{p}%' -> to_percent {p}",
    ),
    way(
        "take the reverb off {t}",
        "set_reverb",
        &[T, ("to_percent", "0")],
        "'reverb off' -> to_percent 0; '{t}' -> tracks",
    ),
    way("more delay on {t}", "set_delay", &[T, ("way", "more")], "'more delay' -> more; '{t}' -> tracks"),
    way("add echo to {t}", "set_delay", &[T, ("way", "more")], "'add echo' -> more delay; '{t}' -> tracks"),
    way("less echo on {t}", "set_delay", &[T, ("way", "less")], "'less echo' -> less delay; '{t}' -> tracks"),
    way(
        "set the delay on {t} to {p}%",
        "set_delay",
        &[T, ("to_percent", "{p}")],
        "'{t}' -> tracks; '{p}%' -> to_percent {p}",
    ),
    way("no delay on {t}", "set_delay", &[T, ("to_percent", "0")], "'no delay' -> to_percent 0; '{t}' -> tracks"),
    way("pan {t} left", "pan", &[T, ("side", "left")], "'{t}' -> tracks; 'left' -> side left"),
    way("move {t} to the right", "pan", &[T, ("side", "right")], "'{t}' -> tracks; 'right' -> side right"),
    way(
        "pan {t} hard right",
        "pan",
        &[T, ("side", "right"), ("amount", "hard")],
        "'hard right' -> side right, amount hard",
    ),
    way(
        "{t} a little to the left",
        "pan",
        &[T, ("side", "left"), ("amount", "a bit")],
        "'a little to the left' -> side left, amount a bit",
    ),
    way("center {t}", "pan", &[T, ("side", "center")], "'center' -> side center; '{t}' -> tracks"),
    way("put {t} in the middle", "pan", &[T, ("side", "center")], "'middle' -> side center; '{t}' -> tracks"),
    way("set the tempo to {bpm}", "set_tempo", &[("bpm", "{bpm}")], "'tempo to {bpm}' -> bpm {bpm}"),
    way("make it {bpm} bpm", "set_tempo", &[("bpm", "{bpm}")], "'{bpm} bpm' -> bpm {bpm}"),
    way("make it faster", "set_tempo", &[("way", "faster")], "'faster' -> faster"),
    way("it drags, speed it up", "set_tempo", &[("way", "faster")], "'speed it up' -> faster"),
    way("slow it down a little", "set_tempo", &[("way", "slower")], "'slow it down' -> slower"),
    way("speed it up by {n} bpm", "set_tempo", &[("by_bpm", "{n}")], "'up by {n} bpm' -> by_bpm {n#}"),
    way("drop the tempo by {n}", "set_tempo", &[("by_bpm", "-{n}")], "'drop by {n}' -> by_bpm -{n#}"),
    way("change the key to {key}", "set_key", &[("key", "{key}")], "'{key}' -> key"),
    way("put it in {key}", "set_key", &[("key", "{key}")], "'in {key}' -> key '{key}'"),
    way("transpose it to {key}", "set_key", &[("key", "{key}")], "'to {key}' -> key '{key}'"),
    way("swap {t} for {sound}", "swap_sound", &[T, ("sound", "{sound}")], "'{t}' -> tracks; 'for {sound}' -> sound"),
    way("use {sound} for {t}", "swap_sound", &[T, ("sound", "{sound}")], "'use {sound}' -> sound; '{t}' -> tracks"),
    way(
        "replace {t} with {sound}",
        "swap_sound",
        &[T, ("sound", "{sound}")],
        "'{t}' -> tracks; 'with {sound}' -> sound",
    ),
    way("try a different sound for {t}", "swap_sound", &[T], "'different sound' -> swap; '{t}' -> tracks"),
    way("cut {t} from {time}", "cut_track", &[T, ("from", "{time}")], "'{t}' -> tracks; 'from {time}' -> from {time}"),
    way("no {t} after {time}", "cut_track", &[T, ("from", "{time}")], "'{t}' -> tracks; 'after {time}' -> from {time}"),
    way(
        "{t} should stop at {time}",
        "cut_track",
        &[T, ("from", "{time}")],
        "'{t}' -> tracks; 'stop at {time}' -> from {time}",
    ),
    way("mute {t}", "mute", &[T], "'mute' -> mute; '{t}' -> tracks"),
    way("silence {t} for now", "mute", &[T], "'silence' -> mute; '{t}' -> tracks"),
    way("kill {t} for a bit", "mute", &[T], "'kill for a bit' -> mute; '{t}' -> tracks"),
    way("unmute {t}", "unmute", &[T], "'unmute' -> unmute; '{t}' -> tracks"),
    way("bring {t} back in", "unmute", &[T], "'bring back in' -> unmute; '{t}' -> tracks"),
    way("solo {t}", "solo", &[T], "'solo' -> solo; '{t}' -> tracks"),
    way("let me hear only {t}", "solo", &[T], "'only' -> solo; '{t}' -> tracks"),
    way("just {t} please", "solo", &[T], "'just' -> solo; '{t}' -> tracks"),
    way("remove {t}", "remove_track", &[T], "'remove' -> remove_track; '{t}' -> tracks"),
    way("get rid of {t} completely", "remove_track", &[T], "'get rid of' -> remove_track; '{t}' -> tracks"),
    way("delete {t}", "remove_track", &[T], "'delete' -> remove_track; '{t}' -> tracks"),
];

/// More ways of saying the same things: other words and other shapes of a request, its numbers in words too.
const MORE: &[Way] = &[
    way("undo it", "undo", &[], "'undo it' -> undo"),
    way("undo please", "undo", &[], "'undo' -> undo"),
    way("ctrl z", "undo", &[], "'ctrl z' -> undo"),
    way("take that back", "undo", &[], "'take that back' -> undo"),
    way("scrap that", "undo", &[], "'scrap that' -> undo"),
    way("i didn't like that, undo it", "undo", &[], "'undo it' -> undo"),
    way("cancel my last change", "undo", &[], "'cancel my last change' -> undo"),
    way("revert that", "undo", &[], "'revert that' -> undo"),
    way("can you undo the last {n}", "undo", &[("count", "{n}")], "'the last {n}' -> count {n#}"),
    way("undo {n} times", "undo", &[("count", "{n}")], "'{n} times' -> count {n#}"),
    way("roll back the last {n} edits", "undo", &[("count", "{n}")], "'roll back' -> undo; 'last {n}' -> count {n#}"),
    way("step back {n} times", "undo", &[("count", "{n}")], "'step back' -> undo; '{n} times' -> count {n#}"),
    way("scrap the last {n} things", "undo", &[("count", "{n}")], "'scrap the last {n}' -> undo, count {n#}"),
    way("undo the previous {n} changes", "undo", &[("count", "{n}")], "'previous {n}' -> count {n#}"),
    way("redo that", "redo", &[], "'redo that' -> redo"),
    way("redo please", "redo", &[], "'redo' -> redo"),
    way("put that change back", "redo", &[], "'put that change back' -> redo"),
    way("reapply what i undid", "redo", &[], "'what i undid' -> redo"),
    way("redo {n} times", "redo", &[("count", "{n}")], "'redo {n} times' -> count {n#}"),
    way("redo the last {n} undone changes", "redo", &[("count", "{n}")], "'last {n}' -> count {n#}"),
    way("back to before {change}", "go_back", &[("change", "{change}")], "'before {change}' -> change '{change}'"),
    way(
        "take the song back to before {change}",
        "go_back",
        &[("change", "{change}")],
        "'before {change}' -> change '{change}'",
    ),
    way(
        "restore the song from before {change}",
        "go_back",
        &[("change", "{change}")],
        "'from before {change}' -> change '{change}'",
    ),
    way("rewind to before {change}", "go_back", &[("change", "{change}")], "'before {change}' -> change '{change}'"),
    way(
        "can we go back to before {change}",
        "go_back",
        &[("change", "{change}")],
        "'before {change}' -> change '{change}'",
    ),
    way(
        "revert everything after {change}",
        "go_back",
        &[("change", "{change}")],
        "'everything after {change}' -> change '{change}'",
    ),
    way(
        "go back to how it was before {change}",
        "go_back",
        &[("change", "{change}")],
        "'before {change}' -> change '{change}'",
    ),
    way("louder {t}", "set_volume", &[T, ("way", "louder")], "'louder' -> louder; '{t}' -> tracks"),
    way("{t} louder please", "set_volume", &[T, ("way", "louder")], "'louder' -> louder; '{t}' -> tracks"),
    way("raise {t}", "set_volume", &[T, ("way", "louder")], "'raise' -> louder; '{t}' -> tracks"),
    way("bring {t} up", "set_volume", &[T, ("way", "louder")], "'bring up' -> louder; '{t}' -> tracks"),
    way("push {t} up a bit", "set_volume", &[T, ("way", "louder")], "'push up' -> louder; '{t}' -> tracks"),
    way("more volume on {t}", "set_volume", &[T, ("way", "louder")], "'more volume' -> louder; '{t}' -> tracks"),
    way("gimme more {t}", "set_volume", &[T, ("way", "louder")], "'more' -> louder; '{t}' -> tracks"),
    way("{t} are buried", "set_volume", &[T, ("way", "louder")], "'buried' -> louder; '{t}' -> tracks"),
    way("i want to hear more of {t}", "set_volume", &[T, ("way", "louder")], "'hear more' -> louder; '{t}' -> tracks"),
    way("{t} could be louder", "set_volume", &[T, ("way", "louder")], "'louder' -> louder; '{t}' -> tracks"),
    way("can you make {t} louder", "set_volume", &[T, ("way", "louder")], "'louder' -> louder; '{t}' -> tracks"),
    way("quieter {t} please", "set_volume", &[T, ("way", "quieter")], "'quieter' -> quieter; '{t}' -> tracks"),
    way("lower {t}", "set_volume", &[T, ("way", "quieter")], "'lower' -> quieter; '{t}' -> tracks"),
    way("bring {t} down a bit", "set_volume", &[T, ("way", "quieter")], "'bring down' -> quieter; '{t}' -> tracks"),
    way("{t} is a bit loud", "set_volume", &[T, ("way", "quieter")], "'a bit loud' -> quieter; '{t}' -> tracks"),
    way("{t} too much", "set_volume", &[T, ("way", "quieter")], "'too much' -> quieter; '{t}' -> tracks"),
    way("make {t} less loud", "set_volume", &[T, ("way", "quieter")], "'less loud' -> quieter; '{t}' -> tracks"),
    way("{t} need to be quieter", "set_volume", &[T, ("way", "quieter")], "'quieter' -> quieter; '{t}' -> tracks"),
    way(
        "raise {t} by {n} db",
        "set_volume",
        &[T, ("by_db", "{n}")],
        "'{t}' -> tracks; 'raise by {n} db' -> by_db {n#}",
    ),
    way("bring {t} up {n} db", "set_volume", &[T, ("by_db", "{n}")], "'{t}' -> tracks; 'up {n} db' -> by_db {n#}"),
    way("{t} up {n}db", "set_volume", &[T, ("by_db", "{n}")], "'{t}' -> tracks; 'up {n}db' -> by_db {n#}"),
    way(
        "boost {t} {n} decibels",
        "set_volume",
        &[T, ("by_db", "{n}")],
        "'{t}' -> tracks; 'boost {n} decibels' -> by_db {n#}",
    ),
    way(
        "turn down {t} by {n} db",
        "set_volume",
        &[T, ("by_db", "-{n}")],
        "'{t}' -> tracks; 'down by {n} db' -> by_db -{n#}",
    ),
    way("drop {t} {n} db", "set_volume", &[T, ("by_db", "-{n}")], "'{t}' -> tracks; 'drop {n} db' -> by_db -{n#}"),
    way("cut {t} by {n} db", "set_volume", &[T, ("by_db", "-{n}")], "'{t}' -> tracks; 'cut by {n} db' -> by_db -{n#}"),
    way("{t} volume to {db} db", "set_volume", &[T, ("to_db", "{db}")], "'{t}' -> tracks; 'to {db} db' -> to_db {db}"),
    way("turn {t} to {db}db", "set_volume", &[T, ("to_db", "{db}")], "'{t}' -> tracks; 'to {db}db' -> to_db {db}"),
    way("more grit on {t}", "set_drive", &[T, ("way", "more")], "'more grit' -> more drive; '{t}' -> tracks"),
    way(
        "more crunch on {t} please",
        "set_drive",
        &[T, ("way", "more")],
        "'more crunch' -> more drive; '{t}' -> tracks",
    ),
    way("distort {t} more", "set_drive", &[T, ("way", "more")], "'distort more' -> more drive; '{t}' -> tracks"),
    way("saturate {t}", "set_drive", &[T, ("way", "more")], "'saturate' -> more drive; '{t}' -> tracks"),
    way(
        "crank the overdrive on {t}",
        "set_drive",
        &[T, ("way", "more")],
        "'crank the overdrive' -> more drive; '{t}' -> tracks",
    ),
    way(
        "make {t} sound more distorted",
        "set_drive",
        &[T, ("way", "more")],
        "'more distorted' -> more drive; '{t}' -> tracks",
    ),
    way(
        "give {t} some distortion",
        "set_drive",
        &[T, ("way", "more")],
        "'some distortion' -> more drive; '{t}' -> tracks",
    ),
    way("{t} too clean", "set_drive", &[T, ("way", "more")], "'too clean' -> more drive; '{t}' -> tracks"),
    way(
        "a bit less saturation on {t}",
        "set_drive",
        &[T, ("way", "less")],
        "'less saturation' -> less drive; '{t}' -> tracks",
    ),
    way("{t} is too distorted", "set_drive", &[T, ("way", "less")], "'too distorted' -> less drive; '{t}' -> tracks"),
    way("{t} too fuzzy", "set_drive", &[T, ("way", "less")], "'too fuzzy' -> less drive; '{t}' -> tracks"),
    way(
        "reduce the distortion on {t}",
        "set_drive",
        &[T, ("way", "less")],
        "'reduce the distortion' -> less drive; '{t}' -> tracks",
    ),
    way(
        "drive {t} to {p}%",
        "set_drive",
        &[T, ("to_percent", "{p}")],
        "'{t}' -> tracks; 'drive to {p}%' -> to_percent {p}",
    ),
    way(
        "{t} drive at {p} percent",
        "set_drive",
        &[T, ("to_percent", "{p}")],
        "'{t}' -> tracks; '{p} percent' -> to_percent {p}",
    ),
    way(
        "distort {t} to {p}%",
        "set_drive",
        &[T, ("to_percent", "{p}")],
        "'{t}' -> tracks; 'distort to {p}%' -> to_percent {p}",
    ),
    way(
        "take the drive off {t}",
        "set_drive",
        &[T, ("to_percent", "0")],
        "'drive off' -> to_percent 0; '{t}' -> tracks",
    ),
    way("add reverb to {t}", "set_reverb", &[T, ("way", "more")], "'add reverb' -> more; '{t}' -> tracks"),
    way("drench {t} in reverb", "set_reverb", &[T, ("way", "more")], "'drench in reverb' -> more; '{t}' -> tracks"),
    way("{t} needs more room", "set_reverb", &[T, ("way", "more")], "'more room' -> more reverb; '{t}' -> tracks"),
    way(
        "make {t} sound bigger, more space",
        "set_reverb",
        &[T, ("way", "more")],
        "'more space' -> more reverb; '{t}' -> tracks",
    ),
    way("add some ambience to {t}", "set_reverb", &[T, ("way", "more")], "'ambience' -> more reverb; '{t}' -> tracks"),
    way("{t} sound too dry", "set_reverb", &[T, ("way", "more")], "'too dry' -> more reverb; '{t}' -> tracks"),
    way("a little more reverb on {t}", "set_reverb", &[T, ("way", "more")], "'more reverb' -> more; '{t}' -> tracks"),
    way("too much reverb on {t}", "set_reverb", &[T, ("way", "less")], "'too much reverb' -> less; '{t}' -> tracks"),
    way("{t} too roomy", "set_reverb", &[T, ("way", "less")], "'too roomy' -> less reverb; '{t}' -> tracks"),
    way("make {t} drier", "set_reverb", &[T, ("way", "less")], "'drier' -> less reverb; '{t}' -> tracks"),
    way("{t} reverb at {p}%", "set_reverb", &[T, ("to_percent", "{p}")], "'{t}' -> tracks; '{p}%' -> to_percent {p}"),
    way(
        "reverb on {t} to {p} percent",
        "set_reverb",
        &[T, ("to_percent", "{p}")],
        "'{t}' -> tracks; '{p} percent' -> to_percent {p}",
    ),
    way("no reverb on {t}", "set_reverb", &[T, ("to_percent", "0")], "'no reverb' -> to_percent 0; '{t}' -> tracks"),
    way(
        "give {t} a slapback echo",
        "set_delay",
        &[T, ("way", "more")],
        "'slapback echo' -> more delay; '{t}' -> tracks",
    ),
    way("make {t} echo more", "set_delay", &[T, ("way", "more")], "'echo more' -> more delay; '{t}' -> tracks"),
    way("a touch more delay on {t}", "set_delay", &[T, ("way", "more")], "'more delay' -> more; '{t}' -> tracks"),
    way("echo on {t} please", "set_delay", &[T, ("way", "more")], "'echo' -> more delay; '{t}' -> tracks"),
    way("less delay on {t}", "set_delay", &[T, ("way", "less")], "'less delay' -> less; '{t}' -> tracks"),
    way("turn the delay down on {t}", "set_delay", &[T, ("way", "less")], "'delay down' -> less; '{t}' -> tracks"),
    way(
        "echo on {t} at {p} percent",
        "set_delay",
        &[T, ("to_percent", "{p}")],
        "'{t}' -> tracks; '{p} percent' -> to_percent {p}",
    ),
    way("{t} delay to {p}%", "set_delay", &[T, ("to_percent", "{p}")], "'{t}' -> tracks; 'to {p}%' -> to_percent {p}"),
    way(
        "kill the echo on {t}",
        "set_delay",
        &[T, ("to_percent", "0")],
        "'kill the echo' -> to_percent 0; '{t}' -> tracks",
    ),
    way(
        "turn off the delay on {t}",
        "set_delay",
        &[T, ("to_percent", "0")],
        "'delay off' -> to_percent 0; '{t}' -> tracks",
    ),
    way("push {t} over to the left", "pan", &[T, ("side", "left")], "'{t}' -> tracks; 'to the left' -> side left"),
    way(
        "{t} should sit on the right side",
        "pan",
        &[T, ("side", "right")],
        "'{t}' -> tracks; 'right side' -> side right",
    ),
    way(
        "hard left for {t}",
        "pan",
        &[T, ("side", "left"), ("amount", "hard")],
        "'hard left' -> side left, amount hard",
    ),
    way(
        "pan {t} slightly right",
        "pan",
        &[T, ("side", "right"), ("amount", "a bit")],
        "'slightly right' -> side right, amount a bit",
    ),
    way(
        "pan {t} a bit left",
        "pan",
        &[T, ("side", "left"), ("amount", "a bit")],
        "'a bit left' -> side left, amount a bit",
    ),
    way("centre {t}", "pan", &[T, ("side", "center")], "'centre' -> side center; '{t}' -> tracks"),
    way("move {t} to the middle", "pan", &[T, ("side", "center")], "'middle' -> side center; '{t}' -> tracks"),
    way("bpm {bpm}", "set_tempo", &[("bpm", "{bpm}")], "'bpm {bpm}' -> bpm {bpm}"),
    way("change the bpm to {bpm}", "set_tempo", &[("bpm", "{bpm}")], "'bpm to {bpm}' -> bpm {bpm}"),
    way("{bpm} bpm please", "set_tempo", &[("bpm", "{bpm}")], "'{bpm} bpm' -> bpm {bpm}"),
    way("can it be {bpm} bpm", "set_tempo", &[("bpm", "{bpm}")], "'{bpm} bpm' -> bpm {bpm}"),
    way("bump the tempo to {bpm}", "set_tempo", &[("bpm", "{bpm}")], "'tempo to {bpm}' -> bpm {bpm}"),
    way("make it {bpm}", "set_tempo", &[("bpm", "{bpm}")], "'make it {bpm}' -> bpm {bpm}"),
    way("the tempo should be {bpm}", "set_tempo", &[("bpm", "{bpm}")], "'tempo {bpm}' -> bpm {bpm}"),
    way("faster please", "set_tempo", &[("way", "faster")], "'faster' -> faster"),
    way("speed up the song", "set_tempo", &[("way", "faster")], "'speed up' -> faster"),
    way("make the song quicker", "set_tempo", &[("way", "faster")], "'quicker' -> faster"),
    way("it's too slow", "set_tempo", &[("way", "faster")], "'too slow' -> faster"),
    way("speed it up a bit", "set_tempo", &[("way", "faster")], "'speed it up' -> faster"),
    way("slow it down", "set_tempo", &[("way", "slower")], "'slow it down' -> slower"),
    way("a bit slower please", "set_tempo", &[("way", "slower")], "'slower' -> slower"),
    way("too fast, slow it down", "set_tempo", &[("way", "slower")], "'too fast' -> slower"),
    way("slow the whole thing down", "set_tempo", &[("way", "slower")], "'slow down' -> slower"),
    way("up the tempo by {n}", "set_tempo", &[("by_bpm", "{n}")], "'up by {n}' -> by_bpm {n#}"),
    way("{n} bpm faster", "set_tempo", &[("by_bpm", "{n}")], "'{n} bpm faster' -> by_bpm {n#}"),
    way("knock the tempo down by {n}", "set_tempo", &[("by_bpm", "-{n}")], "'down by {n}' -> by_bpm -{n#}"),
    way("slow it down by {n} bpm", "set_tempo", &[("by_bpm", "-{n}")], "'down by {n} bpm' -> by_bpm -{n#}"),
    way("in {key} please", "set_key", &[("key", "{key}")], "'in {key}' -> key '{key}'"),
    way("move the song to {key}", "set_key", &[("key", "{key}")], "'to {key}' -> key '{key}'"),
    way("switch the key to {key}", "set_key", &[("key", "{key}")], "'key to {key}' -> key '{key}'"),
    way("can we hear it in {key}", "set_key", &[("key", "{key}")], "'in {key}' -> key '{key}'"),
    way("lets try it in {key}", "set_key", &[("key", "{key}")], "'in {key}' -> key '{key}'"),
    way("key of {key}", "set_key", &[("key", "{key}")], "'key of {key}' -> key '{key}'"),
    way("make {t} {sound}", "swap_sound", &[T, ("sound", "{sound}")], "'{t}' -> tracks; '{sound}' -> sound"),
    way("put {sound} on {t}", "swap_sound", &[T, ("sound", "{sound}")], "'{sound}' -> sound; '{t}' -> tracks"),
    way("change {t} to {sound}", "swap_sound", &[T, ("sound", "{sound}")], "'{t}' -> tracks; 'to {sound}' -> sound"),
    way("try {t} as {sound}", "swap_sound", &[T, ("sound", "{sound}")], "'{t}' -> tracks; 'as {sound}' -> sound"),
    way("i want {t} to be {sound}", "swap_sound", &[T, ("sound", "{sound}")], "'{t}' -> tracks; 'be {sound}' -> sound"),
    way("play {t} on {sound}", "swap_sound", &[T, ("sound", "{sound}")], "'{t}' -> tracks; 'on {sound}' -> sound"),
    way("give {t} another sound", "swap_sound", &[T], "'another sound' -> swap; '{t}' -> tracks"),
    way("a new sound for {t}", "swap_sound", &[T], "'new sound' -> swap; '{t}' -> tracks"),
    way("cut {t} out at {time}", "cut_track", &[T, ("from", "{time}")], "'{t}' -> tracks; 'at {time}' -> from {time}"),
    way("stop {t} at {time}", "cut_track", &[T, ("from", "{time}")], "'{t}' -> tracks; 'at {time}' -> from {time}"),
    way(
        "take {t} out from {time}",
        "cut_track",
        &[T, ("from", "{time}")],
        "'{t}' -> tracks; 'from {time}' -> from {time}",
    ),
    way("lose {t} from {time}", "cut_track", &[T, ("from", "{time}")], "'{t}' -> tracks; 'from {time}' -> from {time}"),
    way(
        "{t} should drop out at {time}",
        "cut_track",
        &[T, ("from", "{time}")],
        "'{t}' -> tracks; 'at {time}' -> from {time}",
    ),
    way(
        "kill {t} from {time} on",
        "cut_track",
        &[T, ("from", "{time}")],
        "'{t}' -> tracks; 'from {time}' -> from {time}",
    ),
    way(
        "after {time} no more {t}",
        "cut_track",
        &[T, ("from", "{time}")],
        "'after {time}' -> from {time}; '{t}' -> tracks",
    ),
    way("end {t} at {time}", "cut_track", &[T, ("from", "{time}")], "'{t}' -> tracks; 'at {time}' -> from {time}"),
    way("shut {t} off for now", "mute", &[T], "'shut off' -> mute; '{t}' -> tracks"),
    way("turn off {t}", "mute", &[T], "'turn off' -> mute; '{t}' -> tracks"),
    way("turn {t} off", "mute", &[T], "'turn off' -> mute; '{t}' -> tracks"),
    way("can you mute {t}", "mute", &[T], "'mute' -> mute; '{t}' -> tracks"),
    way("mute {t} for a bit", "mute", &[T], "'mute' -> mute; '{t}' -> tracks"),
    way("bring back {t}", "unmute", &[T], "'bring back' -> unmute; '{t}' -> tracks"),
    way("turn {t} back on", "unmute", &[T], "'back on' -> unmute; '{t}' -> tracks"),
    way("unmute {t} please", "unmute", &[T], "'unmute' -> unmute; '{t}' -> tracks"),
    way("only {t}", "solo", &[T], "'only' -> solo; '{t}' -> tracks"),
    way("play {t} on its own", "solo", &[T], "'on its own' -> solo; '{t}' -> tracks"),
    way("i only want to hear {t}", "solo", &[T], "'only' -> solo; '{t}' -> tracks"),
    way("can i hear just {t}", "solo", &[T], "'just' -> solo; '{t}' -> tracks"),
    way("solo {t} for a second", "solo", &[T], "'solo' -> solo; '{t}' -> tracks"),
    way("throw out {t}", "remove_track", &[T], "'throw out' -> remove_track; '{t}' -> tracks"),
    way("scrap {t} track", "remove_track", &[T], "'scrap' -> remove_track; '{t}' -> tracks"),
    way("i don't want {t} at all", "remove_track", &[T], "'don't want at all' -> remove_track; '{t}' -> tracks"),
    way("remove {t} entirely", "remove_track", &[T], "'remove' -> remove_track; '{t}' -> tracks"),
    way("take {t} out of the song", "remove_track", &[T], "'out of the song' -> remove_track; '{t}' -> tracks"),
];

/// requests no tool serves: they find the studio's actions instead, or nothing
const NOTHING: &[&str] = &[
    "what's the weather like",
    "hello",
    "thanks",
    "who are you",
    "tell me a joke",
    "write me a poem",
    "what time is it",
    "save",
    "save the song",
    "open a project",
    "export the song as a wav",
    "new song",
    "make a new song",
    "play",
    "stop",
    "render it",
    "show me the mixer",
    "open the script",
    "how do i record my voice",
    "what key is it in",
    "how loud is it",
    "is this any good",
    "what genre is this",
    "make it sound better",
    "make it groovier",
    "do something cool",
    "rate my song",
    "connect an assistant",
    "download the sample packs",
    "don't mute the drums",
];

/// what a lesson's slots are filled with
struct Fill {
    t: &'static str,
    n: u32,
    /// the small number as it is said: in digits, or in words
    n_said: String,
    db: i32,
    p: u32,
    bpm: u32,
    time: String,
    sound: &'static str,
    key: &'static str,
    change: &'static str,
}

/// small numbers in words, as requests say them
const NUMBER_WORDS: [&str; 10] = ["one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten"];

impl Fill {
    fn draw(rng: &mut Rng) -> Fill {
        let n = 1 + rng.below(9) as u32;
        Fill {
            t: rng.pick(TRACKS),
            n,
            n_said: if rng.below(5) < 2 { NUMBER_WORDS[n as usize - 1].to_string() } else { n.to_string() },
            db: -12 + rng.below(16) as i32,
            p: 5 * rng.below(21) as u32,
            bpm: 60 + 2 * rng.below(60) as u32,
            time: format!("{}:{:02}", rng.below(4), rng.below(60)),
            sound: rng.pick(SOUNDS),
            key: rng.pick(KEYS),
            change: rng.pick(CHANGES),
        }
    }

    /// the words, the small number as it is said
    fn put(&self, s: &str) -> String {
        self.put_with(s, &self.n_said)
    }

    /// an argument: a number where its slot is one (the small number as a number, however it is said), else the
    /// words it stands for
    fn value(&self, v: &str) -> Value {
        let filled = self.put_with(v, &self.n.to_string());
        match filled.parse::<f64>() {
            Ok(x) if !v.contains("{t}") && !v.contains("{time}") && !v.contains("{key}") => json!(x),
            // a sound is said with its article; the argument is the sound
            _ if v.contains("{sound}") => {
                json!(["an ", "a ", "some "].iter().find_map(|a| filled.strip_prefix(a)).unwrap_or(&filled))
            }
            _ => json!(filled),
        }
    }

    fn put_with(&self, s: &str, n: &str) -> String {
        s.replace("{n#}", &self.n.to_string())
            .replace("{t}", self.t)
            .replace("{n}", n)
            .replace("{db}", &self.db.to_string())
            .replace("{p}", &self.p.to_string())
            .replace("{bpm}", &self.bpm.to_string())
            .replace("{time}", &self.time)
            .replace("{sound}", self.sound)
            .replace("{key}", self.key)
            .replace("{change}", self.change)
    }
}

const TRACKS: &[&str] = &[
    "the kick",
    "the kick drum",
    "the snare",
    "the hats",
    "the hi-hats",
    "the toms",
    "the clap",
    "the drums",
    "the whole kit",
    "the bass",
    "the bassline",
    "the sub",
    "the guitar",
    "the guitars",
    "the piano",
    "the keys",
    "the vocals",
    "the vox",
    "the lead",
    "the melody",
    "the whistle",
    "the pad",
    "the strings",
    "the organ",
    "the choir",
    "the kick and the snare",
    "the bass and drums",
    "the guitar and the piano",
    "everything except the vocals",
    "kick",
    "vocals",
    "drums",
];

const SOUNDS: &[&str] =
    &["an organ", "a pluck", "an 808", "a reese", "a supersaw", "a snare", "a warm pad", "a shaker"];

const KEYS: &[&str] = &[
    "A minor",
    "C major",
    "D minor",
    "E flat major",
    "F sharp minor",
    "G major",
    "B minor",
    "e minor",
    "d major",
    "Bb major",
    "C# minor",
    "g minor",
];

const CHANGES: &[&str] = &[
    "we added the whistles",
    "the whistle",
    "the tempo change",
    "it got faster",
    "the bass got quieter",
    "we changed the bass level",
    "the strings came in",
    "the string pad",
];

/// a small deterministic random generator (xorshift), for lessons that are the same for the same seed
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn pick<T: Copy>(&mut self, from: &[T]) -> T {
        from[self.below(from.len())]
    }
}

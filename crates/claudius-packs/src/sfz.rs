//! A practical SFZ reader: `#define` / `#include` preprocessing (including mid-line includes and redefinitions, as
//! the sfzinstruments mappings use them), the `<control> <global> <master> <group> <region>` inheritance chain, and
//! default CC values (`set_ccN`, `set_hdccN`). Regions come out as flat opcode maps; `index.rs` interprets them.

use std::collections::HashMap;
use std::sync::LazyLock;

use anyhow::{Result, bail};
use regex::Regex;

#[derive(Clone, Debug, Default)]
pub struct SfzRegion {
    pub opcodes: HashMap<String, String>,
    pub default_path: String,
    pub note_offset: i32,
    pub octave_offset: i32,
}

impl SfzRegion {
    pub fn get(&self, k: &str) -> Option<&str> {
        self.opcodes.get(k).map(|s| s.as_str())
    }

    pub fn f32(&self, k: &str) -> Option<f32> {
        self.get(k).and_then(|v| v.trim().parse::<f32>().ok())
    }

    /// a key opcode (`lokey`, `pitch_keycenter` ...) as a MIDI number, with note / octave offsets applied
    pub fn key(&self, k: &str) -> Option<i32> {
        let v = self.get(k)?;
        let n = crate::notes::parse_sfz_key(v)?;
        Some(n + self.note_offset + 12 * self.octave_offset)
    }
}

#[derive(Clone, Debug, Default)]
pub struct SfzFile {
    pub regions: Vec<SfzRegion>,
    /// default controller values, normalised to 0..1
    pub cc: HashMap<u32, f32>,
}

impl SfzFile {
    pub fn cc_value(&self, cc: u32) -> f32 {
        self.cc.get(&cc).copied().unwrap_or(0.0)
    }
}

static HEADER_OR_OPCODE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<([A-Za-z_]+)>|(?:^|\s)([A-Za-z0-9_]+)=").expect("regex"));
static INCLUDE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"#include\s+"([^"]+)""#).expect("regex"));
static BLOCK_COMMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)/\*.*?\*/").expect("regex"));

#[derive(Clone, Copy, PartialEq, Eq)]
enum Level {
    None,
    Control,
    Global,
    Master,
    Group,
    Region,
    Ignored,
}

struct State {
    defines: Vec<(String, String)>,
    level: Level,
    global: HashMap<String, String>,
    master: HashMap<String, String>,
    group: HashMap<String, String>,
    default_path: String,
    note_offset: i32,
    octave_offset: i32,
    out: SfzFile,
    in_region: bool,
}

/// Parse an SFZ document. `include` returns the text of an `#include`d file; paths are relative to the directory of
/// the root .sfz file (sforzando / sfizz semantics).
pub fn parse(text: &str, include: &mut dyn FnMut(&str) -> Result<String>) -> Result<SfzFile> {
    let mut st = State {
        defines: Vec::new(),
        level: Level::None,
        global: HashMap::new(),
        master: HashMap::new(),
        group: HashMap::new(),
        default_path: String::new(),
        note_offset: 0,
        octave_offset: 0,
        out: SfzFile::default(),
        in_region: false,
    };
    process(text, include, &mut st, 0)?;
    Ok(st.out)
}

fn process(text: &str, include: &mut dyn FnMut(&str) -> Result<String>, st: &mut State, depth: usize) -> Result<()> {
    if depth > 16 {
        bail!("sfz: #include nesting too deep");
    }
    let text = BLOCK_COMMENT.replace_all(text, " ");
    for raw in text.lines() {
        let line = match raw.find("//") {
            Some(i) => &raw[..i],
            None => raw,
        };
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if let Some(rest) = t.strip_prefix("#define") {
            let rest = rest.trim();
            let mut it = rest.splitn(2, char::is_whitespace);
            if let (Some(name), Some(val)) = (it.next(), it.next())
                && name.starts_with('$')
            {
                let name = name.to_string();
                let val = expand(val.trim(), &st.defines);
                st.defines.retain(|(n, _)| *n != name);
                st.defines.push((name, val));
            }
            continue;
        }
        let mut last = 0;
        for cap in INCLUDE.captures_iter(line) {
            let m = cap.get(0).expect("match");
            segment(&line[last..m.start()], st);
            let path = expand(&cap[1], &st.defines);
            let inc = include(&path.replace('\\', "/"))?;
            process(&inc, include, st, depth + 1)?;
            last = m.end();
        }
        segment(&line[last..], st);
    }
    Ok(())
}

/// substitute `$VARIABLES`, longest defined name first
fn expand(s: &str, defines: &[(String, String)]) -> String {
    if !s.contains('$') || defines.is_empty() {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    let b = s.as_bytes();
    while i < s.len() {
        if b[i] == b'$' {
            let mut best: Option<&(String, String)> = None;
            for d in defines {
                if s[i..].starts_with(d.0.as_str()) && best.is_none_or(|b| d.0.len() > b.0.len()) {
                    best = Some(d);
                }
            }
            if let Some((name, val)) = best {
                out.push_str(val);
                i += name.len();
                continue;
            }
        }
        let c = s[i..].chars().next().expect("char");
        out.push(c);
        i += c.len_utf8();
    }
    out
}

fn segment(seg: &str, st: &mut State) {
    // `<region>sample=x.wav` is legal: make sure the opcode after a header starts at a word boundary
    let seg = expand(seg, &st.defines).replace('>', "> ");
    if seg.trim().is_empty() {
        return;
    }
    // (position of token start, end of the matched prefix, header name | opcode name)
    let mut toks: Vec<(usize, usize, bool, String)> = Vec::new();
    for cap in HEADER_OR_OPCODE.captures_iter(&seg) {
        if let Some(h) = cap.get(1) {
            let m = cap.get(0).expect("match");
            toks.push((m.start(), m.end(), true, h.as_str().to_ascii_lowercase()));
        } else if let Some(k) = cap.get(2) {
            let m = cap.get(0).expect("match");
            toks.push((k.start(), m.end(), false, k.as_str().to_string()));
        }
    }
    for (i, (_, end, is_header, name)) in toks.iter().enumerate() {
        if *is_header {
            header(name, st);
        } else {
            let stop = toks.get(i + 1).map_or(seg.len(), |t| t.0);
            let val = seg[*end..stop].trim();
            opcode(name, val, st);
        }
    }
}

fn header(name: &str, st: &mut State) {
    st.in_region = false;
    st.level = match name {
        "control" => Level::Control,
        "global" => {
            st.global.clear();
            st.master.clear();
            st.group.clear();
            Level::Global
        }
        "master" => {
            st.master.clear();
            st.group.clear();
            Level::Master
        }
        "group" => {
            st.group.clear();
            Level::Group
        }
        "region" => {
            let mut ops = st.global.clone();
            ops.extend(st.master.iter().map(|(k, v)| (k.clone(), v.clone())));
            ops.extend(st.group.iter().map(|(k, v)| (k.clone(), v.clone())));
            st.out.regions.push(SfzRegion {
                opcodes: ops,
                default_path: st.default_path.clone(),
                note_offset: st.note_offset,
                octave_offset: st.octave_offset,
            });
            st.in_region = true;
            Level::Region
        }
        _ => Level::Ignored,
    };
}

fn opcode(name: &str, val: &str, st: &mut State) {
    // a few aliases from SFZ v1 / ARIA
    let name = match name {
        "loopstart" => "loop_start",
        "loopend" => "loop_end",
        "loopmode" => "loop_mode",
        "trigger_mode" => "trigger",
        _ => name,
    };
    match st.level {
        Level::Control => match name {
            "default_path" => st.default_path = val.replace('\\', "/"),
            "note_offset" => st.note_offset = val.parse().unwrap_or(0),
            "octave_offset" => st.octave_offset = val.parse().unwrap_or(0),
            _ => {
                if let Some(n) = name.strip_prefix("set_hdcc") {
                    if let (Ok(cc), Ok(v)) = (n.parse::<u32>(), val.parse::<f32>()) {
                        st.out.cc.insert(cc, v.clamp(0.0, 1.0));
                    }
                } else if let Some(n) = name.strip_prefix("set_cc")
                    && let (Ok(cc), Ok(v)) = (n.parse::<u32>(), val.parse::<f32>())
                {
                    st.out.cc.insert(cc, (v / 127.0).clamp(0.0, 1.0));
                }
            }
        },
        Level::Global => {
            st.global.insert(name.to_string(), val.to_string());
        }
        Level::Master => {
            st.master.insert(name.to_string(), val.to_string());
        }
        Level::Group => {
            st.group.insert(name.to_string(), val.to_string());
        }
        Level::Region => {
            if st.in_region
                && let Some(r) = st.out.regions.last_mut()
            {
                r.opcodes.insert(name.to_string(), val.to_string());
            }
        }
        // opcodes before any header behave like <global> in practice
        Level::None => {
            st.global.insert(name.to_string(), val.to_string());
        }
        Level::Ignored => {}
    }
}

/// Normalise a sample path: backslashes, `./`, `../` segments. `base` is the directory of the root .sfz file.
pub fn join_path(base: &str, default_path: &str, sample: &str) -> String {
    let sample = sample.replace('\\', "/");
    let mut full = String::new();
    if !base.is_empty() {
        full.push_str(base.trim_end_matches('/'));
        full.push('/');
    }
    if !default_path.is_empty() && !sample.starts_with('/') {
        full.push_str(default_path.trim_start_matches("./"));
        if !full.ends_with('/') {
            full.push('/');
        }
    }
    full.push_str(sample.trim_start_matches("./"));
    normalize_path(&full)
}

pub fn normalize_path(p: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for seg in p.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_inc(_: &str) -> Result<String> {
        bail!("no includes")
    }

    #[test]
    fn inheritance_and_values_with_spaces() {
        let src = r#"
// comment
<control> default_path=Samples\Grand Piano\
<global> ampeg_release=0.5 volume=-3
<group> lovel=1 hivel=64 /* block
comment */
<region> sample=C4 soft.wav lokey=58 hikey=61 pitch_keycenter=c4
<region> sample=D4 soft.wav key=62
<group> lovel=65 hivel=127 ampeg_release=0.7
<region> sample=C4 loud.wav lokey=58 hikey=61 pitch_keycenter=60 trigger=attack
<curve> curve_index=7 v000=0 v127=1
"#;
        let f = parse(src, &mut no_inc).unwrap();
        assert_eq!(f.regions.len(), 3);
        let r = &f.regions[0];
        assert_eq!(r.get("sample"), Some("C4 soft.wav"));
        assert_eq!(r.get("ampeg_release"), Some("0.5"));
        assert_eq!(r.get("volume"), Some("-3"));
        assert_eq!(r.key("pitch_keycenter"), Some(60));
        assert_eq!(r.default_path, "Samples/Grand Piano/");
        assert_eq!(f.regions[1].key("key"), Some(62));
        assert_eq!(f.regions[1].get("hivel"), Some("64"));
        assert_eq!(f.regions[2].get("ampeg_release"), Some("0.7"));
        assert_eq!(f.regions[2].get("lovel"), Some("65"));
        assert_eq!(join_path("", &r.default_path, r.get("sample").unwrap()), "Samples/Grand Piano/C4 soft.wav");
    }

    #[test]
    fn defines_and_mid_line_includes() {
        // the Salamander layout: per-layer defines, then a shared region list that uses them
        let mut files: HashMap<&str, &str> = HashMap::new();
        files.insert("Data/notes.txt", "<group> #include \"Data/v1.txt\" lovel=1 hivel=64 #include \"Data/region.txt\"\n<group> #include \"Data/v2.txt\" lovel=65 hivel=127 #include \"Data/region.txt\"");
        files.insert("Data/v1.txt", "#define $VEL v1");
        files.insert("Data/v2.txt", "#define $VEL v16");
        files.insert(
            "Data/region.txt",
            "<region> lokey=21 hikey=22 pitch_keycenter=21 sample=A0$VEL.$EXT\n<region> key=24 sample=C1$VEL.$EXT",
        );
        let src = "#define $EXT flac\n#define $E xx\n<control>\ndefault_path=Samples/\nset_hdcc$CC=0.73\n<global> #include \"Data/notes.txt\"";
        let mut inc = |p: &str| -> Result<String> { Ok(files.get(p).map(|s| s.to_string()).unwrap_or_default()) };
        let f = parse(src, &mut inc).unwrap();
        let samples: Vec<_> = f.regions.iter().map(|r| r.get("sample").unwrap().to_string()).collect();
        assert_eq!(samples, ["A0v1.flac", "C1v1.flac", "A0v16.flac", "C1v16.flac"]);
        assert_eq!(f.regions[2].get("lovel"), Some("65"));
        assert_eq!(f.regions[0].default_path, "Samples/");
    }

    #[test]
    fn cc_defaults_and_paths() {
        let f =
            parse("<control> set_cc7=127 set_hdcc99=0.25\n<region> sample=..\\Samples\\a.wav", &mut no_inc).unwrap();
        assert!((f.cc_value(7) - 1.0).abs() < 1e-6);
        assert!((f.cc_value(99) - 0.25).abs() < 1e-6);
        assert_eq!(f.cc_value(1), 0.0);
        assert_eq!(join_path("Programs", "", f.regions[0].get("sample").unwrap()), "Samples/a.wav");
    }
}

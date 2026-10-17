//! Strudel sample maps: `{"_base": "https://.../", "bd": ["bd/1.wav", ...], "piano": {"c4": "x.wav", "a3": [...]}}`
//! and the `github:user/repo[/branch[/path]]` shorthand, which reads `strudel.json` from that location.

use anyhow::{Context, Result, bail};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub enum MapValue {
    /// one sound with variants (`bd:0`, `bd:1` ...)
    List(Vec<String>),
    /// a pitched instrument: note name -> files (several files = variants, picked by index like `n` in Strudel)
    Pitched(Vec<(String, Vec<String>)>),
}

#[derive(Clone, Debug, Default)]
pub struct SampleMap {
    pub base: String,
    pub entries: Vec<(String, MapValue)>,
    /// `_license` / `_licence` / `license` when the map declares one
    pub licence: Option<String>,
    pub attribution: Option<String>,
}

/// URL of the `strudel.json` a `github:` shorthand points to, or the URL itself.
pub fn map_url(spec: &str) -> Result<String> {
    let s = spec.trim();
    if let Some(path) = s.strip_prefix("github:") {
        let path = path.trim_matches('/');
        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
        if parts.len() < 2 {
            bail!("github shorthand needs user/repo: {spec}");
        }
        let mut p = parts.join("/");
        if parts.len() == 2 {
            p.push_str("/main");
        }
        return Ok(format!("https://raw.githubusercontent.com/{p}/strudel.json"));
    }
    if s.starts_with("http://") || s.starts_with("https://") {
        return Ok(s.to_string());
    }
    bail!("not a sample map URL or github: shorthand: {spec}")
}

/// `(owner, repo)` for raw.githubusercontent.com / github.com URLs
pub fn github_repo(url: &str) -> Option<(String, String)> {
    for prefix in
        ["https://raw.githubusercontent.com/", "https://github.com/", "https://media.githubusercontent.com/media/"]
    {
        if let Some(rest) = url.strip_prefix(prefix) {
            let mut it = rest.split('/');
            let (o, r) = (it.next()?, it.next()?);
            if !o.is_empty() && !r.is_empty() {
                return Some((o.to_string(), r.to_string()));
            }
        }
    }
    None
}

/// directory of a URL, with a trailing slash
pub fn url_dir(url: &str) -> String {
    match url.rfind('/') {
        Some(i) => url[..=i].to_string(),
        None => String::new(),
    }
}

/// Join a map value with the base. Absolute URLs are kept; relative ones are appended to the base, percent-encoding
/// characters that are not already encoded.
pub fn resolve(base: &str, value: &str) -> String {
    if value.starts_with("http://") || value.starts_with("https://") {
        return value.to_string();
    }
    let mut b = base.to_string();
    if !b.is_empty() && !b.ends_with('/') {
        b.push('/');
    }
    b.push_str(&encode_path(value.trim_start_matches("./")));
    b
}

/// percent-encode a path for a URL, leaving `/` and existing `%XX` escapes alone
pub fn encode_path(p: &str) -> String {
    let mut out = String::with_capacity(p.len() + 8);
    let b = p.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        let keep = c.is_ascii_alphanumeric()
            || matches!(c, b'/' | b'-' | b'_' | b'.' | b'~' | b'!' | b'(' | b')' | b'\'' | b'*');
        if keep {
            out.push(c as char);
        } else if c == b'%' && i + 2 < b.len() && b[i + 1].is_ascii_hexdigit() && b[i + 2].is_ascii_hexdigit() {
            out.push('%');
        } else {
            out.push_str(&format!("%{c:02X}"));
        }
        i += 1;
    }
    out
}

/// decode `%XX` escapes (for local file names derived from URLs)
pub fn decode_path(p: &str) -> String {
    let b = p.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Ok(v) = u8::from_str_radix(&p[i + 1..i + 3], 16)
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn strings(v: &Value) -> Vec<String> {
    match v {
        Value::String(s) => vec![s.clone()],
        Value::Array(a) => a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect(),
        _ => Vec::new(),
    }
}

/// Parse a sample map. `url` is where it came from: without `_base`, values are relative to its directory.
pub fn parse_map(json: &str, url: &str) -> Result<SampleMap> {
    let v: Value = serde_json::from_str(json).context("sample map is not valid JSON")?;
    let Value::Object(obj) = v else { bail!("sample map must be a JSON object") };
    let mut map = SampleMap { base: url_dir(url), ..Default::default() };
    for (k, val) in &obj {
        if k.starts_with('_') || k == "license" || k == "licence" {
            match k.as_str() {
                "_base" => {
                    if let Some(b) = val.as_str() {
                        map.base = if b.starts_with("http://") || b.starts_with("https://") {
                            b.to_string()
                        } else if let Some(gh) = b.strip_prefix("github:") {
                            // `_base: "github:user/repo/branch/"` is accepted by Strudel too
                            let gh = gh.trim_matches('/');
                            let parts: Vec<&str> = gh.split('/').collect();
                            let p = if parts.len() == 2 { format!("{gh}/main") } else { gh.to_string() };
                            format!("https://raw.githubusercontent.com/{p}/")
                        } else {
                            resolve(&url_dir(url), b)
                        };
                    }
                }
                "_license" | "_licence" | "license" | "licence" => map.licence = val.as_str().map(|s| s.to_string()),
                "_attribution" | "_author" | "_credit" | "_credits" => {
                    map.attribution = val.as_str().map(|s| s.to_string())
                }
                _ => {}
            }
            continue;
        }
        match val {
            Value::String(_) | Value::Array(_) => {
                let files = strings(val);
                if !files.is_empty() {
                    map.entries.push((k.clone(), MapValue::List(files)));
                }
            }
            Value::Object(notes) => {
                let mut list = Vec::new();
                for (note, f) in notes {
                    let files = strings(f);
                    if !files.is_empty() {
                        list.push((note.clone(), files));
                    }
                }
                if !list.is_empty() {
                    map.entries.push((k.clone(), MapValue::Pitched(list)));
                }
            }
            _ => {}
        }
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_shorthand() {
        assert_eq!(
            map_url("github:tidalcycles/uzu-drumkit").unwrap(),
            "https://raw.githubusercontent.com/tidalcycles/uzu-drumkit/main/strudel.json"
        );
        assert_eq!(
            map_url("github:yaxu/clean-breaks/main").unwrap(),
            "https://raw.githubusercontent.com/yaxu/clean-breaks/main/strudel.json"
        );
        assert_eq!(
            map_url("github:user/repo/dev/sub/dir/").unwrap(),
            "https://raw.githubusercontent.com/user/repo/dev/sub/dir/strudel.json"
        );
        assert_eq!(map_url("https://x.org/a.json").unwrap(), "https://x.org/a.json");
        assert!(map_url("bd").is_err());
        assert_eq!(
            github_repo("https://raw.githubusercontent.com/felixroos/dough-samples/main/piano.json"),
            Some(("felixroos".into(), "dough-samples".into()))
        );
    }

    #[test]
    fn maps() {
        let json = r#"{
            "_base": "https://example.org/samples/",
            "_license": "CC0-1.0",
            "bd": ["bd/BD 1.wav", "bd/2.wav"],
            "sd": "sd/1.wav",
            "piano": {"C4": "C4v8.mp3", "a3": ["A3a.mp3", "A3b.mp3"], "Ds1": "Ds1v8.mp3"},
            "abs": ["https://cdn.org/x.wav"]
        }"#;
        let m = parse_map(json, "https://github.com/x/y/strudel.json").unwrap();
        assert_eq!(m.base, "https://example.org/samples/");
        assert_eq!(m.licence.as_deref(), Some("CC0-1.0"));
        let bd = m.entries.iter().find(|e| e.0 == "bd").unwrap();
        assert_eq!(bd.1, MapValue::List(vec!["bd/BD 1.wav".into(), "bd/2.wav".into()]));
        assert_eq!(resolve(&m.base, "bd/BD 1.wav"), "https://example.org/samples/bd/BD%201.wav");
        assert_eq!(resolve(&m.base, "Grand%20Piano/A%230.wav"), "https://example.org/samples/Grand%20Piano/A%230.wav");
        assert_eq!(resolve(&m.base, "https://cdn.org/x.wav"), "https://cdn.org/x.wav");
        let MapValue::Pitched(p) = &m.entries.iter().find(|e| e.0 == "piano").unwrap().1 else { panic!() };
        assert_eq!(p.len(), 3);
        // no _base: relative to the map's own directory
        let m =
            parse_map(r#"{"hh": ["hh/1.wav"]}"#, "https://raw.githubusercontent.com/a/b/main/strudel.json").unwrap();
        assert_eq!(resolve(&m.base, "hh/1.wav"), "https://raw.githubusercontent.com/a/b/main/hh/1.wav");
        assert_eq!(decode_path("A%230%20x.wav"), "A#0 x.wav");
    }
}

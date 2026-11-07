//! The raw commands an agent sends with the `command` tool, checked against their JSON schemas before they go: a
//! command claudius has no name for, or a field it does not take, is answered with what it does take, where serde
//! alone would drop the field and send the command without it.

use std::sync::LazyLock;

use claudius_api::Command;
use serde_json::{Map, Value};

static SCHEMA: LazyLock<Value> =
    LazyLock::new(|| serde_json::to_value(schemars::schema_for!(Command)).unwrap_or(Value::Null));

/// every command, by its name (`set_tempo`), with its schema: an object's, with its `properties` and `required`
fn commands() -> Vec<(&'static str, &'static Map<String, Value>)> {
    let variants = SCHEMA.get("oneOf").or_else(|| SCHEMA.get("anyOf")).and_then(Value::as_array);
    variants
        .into_iter()
        .flatten()
        .filter_map(|v| {
            let v = v.as_object()?;
            let cmd = v.get("properties")?.get("cmd")?;
            let name = cmd.get("const").or_else(|| cmd.get("enum").and_then(|e| e.get(0)))?.as_str()?;
            Some((name, v))
        })
        .collect()
}

/// the names of every command claudius takes
pub fn names() -> Vec<&'static str> {
    commands().into_iter().map(|(name, _)| name).collect()
}

fn schema_of(name: &str) -> Option<&'static Map<String, Value>> {
    commands().into_iter().find(|(n, _)| *n == name).map(|(_, s)| s)
}

/// what a value of this schema is, in a word or two: `number`, `string`, `true or false`, `list of {t, d, p, v}` ...
fn kind(s: &Value) -> String {
    if let Some(r) = s.get("$ref").and_then(Value::as_str) {
        let name = r.rsplit('/').next().unwrap_or(r);
        let def = SCHEMA.get("$defs").and_then(|d| d.get(name));
        // an object by its fields, which the agent can write, rather than by a type's name, which it cannot look up
        return match def.and_then(|d| d.get("properties")).and_then(Value::as_object) {
            Some(p) => format!("{{{}}}", p.keys().map(String::as_str).collect::<Vec<_>>().join(", ")),
            None => def.map_or_else(|| name.to_string(), kind),
        };
    }
    let one = |t: &str| match t {
        "boolean" => "true or false".to_string(),
        "integer" => "whole number".to_string(),
        "array" => format!("list of {}", s.get("items").map_or("values".to_string(), kind)),
        "object" => "object".to_string(),
        other => other.to_string(),
    };
    match s.get("type") {
        Some(Value::String(t)) => one(t),
        // an optional field is its type or null
        Some(Value::Array(ts)) => {
            ts.iter().filter_map(Value::as_str).filter(|t| *t != "null").map(one).collect::<Vec<_>>().join(" or ")
        }
        _ => s
            .get("anyOf")
            .or_else(|| s.get("oneOf"))
            .and_then(Value::as_array)
            .map(|v| v.iter().filter(|x| x.get("type").and_then(Value::as_str) != Some("null")).map(kind))
            .map_or_else(|| "value".to_string(), |k| k.collect::<Vec<_>>().join(" or ")),
    }
}

/// what a command does and takes, a line a field: its name, whether it is needed, its kind and what it is for
pub fn fields(name: &str) -> String {
    let Some(s) = schema_of(name) else { return String::new() };
    let required: Vec<&str> =
        s.get("required").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).collect();
    let props = s.get("properties").and_then(Value::as_object);
    let fields: Vec<(&String, &Value)> = props.into_iter().flatten().filter(|(k, _)| *k != "cmd").collect();
    let mut out = match s.get("description").and_then(Value::as_str) {
        Some(what) => format!("{name}: {}\n", what.replace('\n', " ")),
        None => String::new(),
    };
    if fields.is_empty() {
        out.push_str(&format!("{name} takes nothing but its name\n"));
    } else {
        out.push_str(&format!("{name} takes:\n"));
    }
    for (field, p) in fields {
        let needed = if required.contains(&field.as_str()) { ", needed" } else { "" };
        let what = p.get("description").and_then(Value::as_str).map(|d| format!(": {}", d.replace('\n', " ")));
        out.push_str(&format!("  {field} ({}{needed}){}\n", kind(p), what.unwrap_or_default()));
    }
    out
}

/// how many single-letter edits turn `a` into `b`
fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let here = row[j + 1];
            row[j + 1] = (prev + usize::from(ca != *cb)).min(row[j] + 1).min(here + 1);
            prev = here;
        }
    }
    row[b.len()]
}

/// the command, or field, that `wrong` was most likely meant to be, when one is near enough to say so
fn nearest<'a>(wrong: &str, among: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    among
        .into_iter()
        .map(|n| (distance(wrong, n), n))
        .filter(|(d, n)| *d <= (n.len() / 3).max(1) || n.contains(wrong) || wrong.contains(n))
        .min_by_key(|(d, _)| *d)
        .map(|(_, n)| n)
}

/// The command an agent wrote, as claudius takes it; or why not, with what it takes instead: a name claudius does not
/// know (and the one nearest it), a field the command does not take, or a value of the wrong kind.
pub fn check(v: Value) -> Result<Command, String> {
    let Some(obj) = v.as_object() else {
        return Err("a command is a JSON object such as {\"cmd\": \"set_tempo\", \"bpm\": 124}".into());
    };
    let Some(name) = obj.get("cmd").and_then(Value::as_str) else {
        return Err(format!("a command names what it does in \"cmd\", one of: {}", names().join(", ")));
    };
    let Some(schema) = schema_of(name) else {
        let near = nearest(name, names()).map(|n| format!(" (did you mean {n}?)")).unwrap_or_default();
        return Err(format!("claudius has no command \"{name}\"{near}; it has: {}", names().join(", ")));
    };
    let props = schema.get("properties").and_then(Value::as_object);
    let taken = |k: &str| props.is_some_and(|p| p.contains_key(k));
    let unknown: Vec<String> = obj
        .keys()
        .filter(|k| !taken(k))
        .map(|k| match nearest(k, props.into_iter().flatten().map(|(p, _)| p.as_str()).filter(|p| *p != "cmd")) {
            Some(n) => format!("\"{k}\" (did you mean \"{n}\"?)"),
            None => format!("\"{k}\""),
        })
        .collect();
    if !unknown.is_empty() {
        return Err(format!("{name} takes no {}, so nothing was sent.\n{}", unknown.join(", "), fields(name)));
    }
    let name = name.to_string();
    serde_json::from_value(v).map_err(|e| format!("{name}: {e}, so nothing was sent.\n{}", fields(&name)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn every_command_has_a_schema_with_its_fields() {
        let names = names();
        assert!(names.len() > 50, "{names:?}");
        for name in ["set_tempo", "set_mix", "add_section", "install_packs", "undo"] {
            assert!(names.contains(&name), "{name}");
        }
        let mix = fields("set_mix");
        assert!(mix.contains("track (string, needed)"), "{mix}");
        assert!(mix.contains("level_db (number)"), "{mix}");
        assert!(fields("undo").contains("nothing but its name"), "{}", fields("undo"));
        // an object's fields, as the agent writes them
        let trig = fields("set_trig");
        assert!(trig.contains("trig ({prob, cond, ratchet"), "{trig}");
        let notes = fields("set_notes");
        assert!(notes.contains("list of {t, d, p, v}") || notes.contains("list of {d, p, t, v}"), "{notes}");
    }

    #[test]
    fn a_command_with_a_field_it_does_not_take_is_not_sent_and_the_answer_says_what_it_takes() {
        let e = check(json!({"cmd": "set_mix", "track": "bass", "volume_db": -3})).unwrap_err();
        assert!(e.contains("takes no \"volume_db\""), "{e}");
        assert!(e.contains("level_db"), "{e}");
        let e = check(json!({"cmd": "set_mix", "track": "bass", "lvl_db": -3})).unwrap_err();
        assert!(e.contains("did you mean \"level_db\"?"), "{e}");
    }

    #[test]
    fn a_command_claudius_has_no_name_for_is_answered_with_the_nearest_and_the_rest() {
        let e = check(json!({"cmd": "set_tempi", "bpm": 120})).unwrap_err();
        assert!(e.contains("did you mean set_tempo?"), "{e}");
        assert!(e.contains("add_section"), "{e}");
        assert!(check(json!({"bpm": 120})).unwrap_err().contains("\"cmd\""));
        assert!(check(json!("set_tempo")).is_err());
    }

    #[test]
    fn a_value_of_the_wrong_kind_or_a_missing_field_is_answered_with_the_fields() {
        let e = check(json!({"cmd": "set_tempo", "bpm": "fast"})).unwrap_err();
        assert!(e.contains("bpm (number, needed)"), "{e}");
        let e = check(json!({"cmd": "add_section", "kind": "break"})).unwrap_err();
        assert!(e.contains("missing field"), "{e}");
    }

    #[test]
    fn a_command_as_the_studio_sends_it_goes_through_as_it_is() {
        let c = check(json!({"cmd": "set_tempo", "bpm": 124.0})).unwrap();
        assert_eq!(c, Command::SetTempo { bpm: 124.0 });
        let c = check(json!({"cmd": "install_packs"})).unwrap();
        assert_eq!(c, Command::InstallPacks { names: Vec::new() });
        assert!(check(json!({"cmd": "set_mix", "track": "bass", "level_db": -3.0, "solo": null})).is_ok());
    }
}

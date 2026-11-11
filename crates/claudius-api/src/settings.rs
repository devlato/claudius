//! Settings, as any claudius wrote them and any later one reads them. A settings file is a JSON object with the version
//! of its kind that wrote it (`"version"`; a file without one is version 0, as claudius wrote them before it had one)
//! and its fields, each read on its own: a field a later claudius changed, or an earlier one wrote wrong, costs only
//! itself, and the rest are read as they were. A field this claudius does not know, as a later one may have written,
//! is kept when it writes the file again, so that going back to an older claudius and forward again loses nothing.

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

/// A settings file's fields, and the version of its kind that wrote them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fields {
    pub version: u64,
    map: Map<String, Value>,
}

impl Fields {
    /// A settings file's text: its fields, or none when it is not a JSON object (a file cut short, or another file).
    pub fn parse(text: &str) -> Fields {
        match serde_json::from_str::<Value>(text) {
            Ok(Value::Object(map)) => Fields::of(map),
            _ => Fields::default(),
        }
    }

    /// fields already parsed, and the version they say they are
    pub fn of(mut map: Map<String, Value>) -> Fields {
        let version = map.remove("version").and_then(|v| v.as_u64()).unwrap_or(0);
        Fields { version, map }
    }

    /// whether there are any
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// the field `key`, if it is there and reads as a `T`
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.map.get(key).and_then(|v| T::deserialize(v).ok())
    }

    /// the field `key`, or `default` where it is not there or does not read as a `T`
    pub fn or<T: DeserializeOwned>(&self, key: &str, default: T) -> T {
        self.get(key).unwrap_or(default)
    }

    /// `key` renamed `to`, as a migration from an earlier version does, unless `to` is there already
    pub fn rename(&mut self, key: &str, to: &str) {
        if !self.map.contains_key(to)
            && let Some(v) = self.map.remove(key)
        {
            self.map.insert(to.to_string(), v);
        }
    }

    /// `key` set to `value`
    pub fn set(&mut self, key: &str, value: impl Serialize) {
        if let Ok(v) = serde_json::to_value(value) {
            self.map.insert(key.to_string(), v);
        }
    }

    /// The fields of `value` (a struct, as its kind writes it) set over these, at `version`: the file to write, with the
    /// fields this claudius does not know kept as they were.
    pub fn with(mut self, value: &impl Serialize, version: u64) -> Fields {
        if let Ok(Value::Object(map)) = serde_json::to_value(value) {
            self.map.extend(map);
        }
        self.at(version)
    }

    /// these fields, as the file of `version` they are written as
    pub fn at(mut self, version: u64) -> Fields {
        self.version = version;
        self
    }

    /// the file's text: its version first, then its fields
    pub fn to_text(&self) -> String {
        #[derive(Serialize)]
        struct File<'a> {
            version: u64,
            #[serde(flatten)]
            fields: &'a Map<String, Value>,
        }
        serde_json::to_string_pretty(&File { version: self.version, fields: &self.map }).unwrap_or_default() + "\n"
    }
}

#[cfg(test)]
mod tests {
    use serde::Serialize;

    use super::Fields;

    #[test]
    fn each_field_is_read_on_its_own() {
        let f = Fields::parse(r#"{"volume": 0.7, "tab": 12, "sevenths": true, "later": {"x": 1}}"#);
        assert_eq!(f.version, 0, "a file without a version is version 0");
        assert_eq!(f.or("volume", 1.0), 0.7);
        assert_eq!(f.or("tab", "mixer".to_string()), "mixer", "a field that does not read is its default alone");
        assert!(f.or("sevenths", false));
        assert_eq!(f.or("missing", 3), 3);
        assert!(Fields::parse("{\"volume\": 0.").is_empty(), "a file cut short reads as none");
        assert!(Fields::parse("[1, 2]").is_empty());
    }

    #[test]
    fn what_this_claudius_does_not_know_is_kept_when_it_writes_again() {
        #[derive(Serialize)]
        struct Mine {
            volume: f32,
        }
        let read = Fields::parse(r#"{"version": 7, "volume": 0.2, "from_a_later_claudius": [1, 2]}"#);
        assert_eq!(read.version, 7);
        let text = read.with(&Mine { volume: 0.5 }, 2).to_text();
        let again = Fields::parse(&text);
        assert_eq!((again.version, again.or("volume", 0.0)), (2, 0.5));
        assert_eq!(again.get::<Vec<u8>>("from_a_later_claudius"), Some(vec![1, 2]));
        assert!(text.trim_start().starts_with("{\n  \"version\": 2"), "{text}");
    }

    #[test]
    fn a_field_an_earlier_version_named_otherwise_is_renamed() {
        let mut f = Fields::parse(r#"{"old": 1, "kept": 2}"#);
        f.rename("old", "new");
        f.rename("kept", "new");
        assert_eq!((f.get::<u8>("new"), f.get::<u8>("old")), (Some(1), None));
        assert_eq!(f.get::<u8>("kept"), Some(2), "a name taken already is not written over");
    }
}

//! The studio's settings, kept between runs in eframe's storage (a file in the app, the page's local storage in a
//! browser), as any studio wrote them and any later one reads them (`claudius_api::settings`): JSON under `KEY`, with its
//! version, each field read on its own, and the fields a later studio wrote kept. A studio before claudius 0.6 kept
//! them as RON under eframe's own key, without a version: they are read as version 0 until the settings are first
//! kept again.

use claudius_api::settings::Fields;
use serde::Serialize;
use serde_json::{Map, Value};

use crate::panels::{self, Tab, View};
use crate::theme::Look;

/// where the settings are kept
pub const KEY: &str = "settings";

/// the version of the settings this studio writes
pub const VERSION: u64 = 1;

/// what the studio keeps between runs
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Prefs {
    pub tab: Tab,
    pub volume: f32,
    pub mutate_count: usize,
    pub keep_best: bool,
    pub generations: usize,
    pub children: usize,
    pub sevenths: bool,
    pub tour_done: bool,
    pub record: panels::record::Settings,
    /// dark, light, or as the system is: since version 1
    pub look: Look,
    /// whether a page speaks what has the keyboard: since version 1
    pub speak: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs::of(&View::default())
    }
}

impl Prefs {
    /// what a view keeps
    pub fn of(v: &View) -> Prefs {
        Prefs {
            tab: v.tab,
            volume: v.volume,
            mutate_count: v.mutate_count,
            keep_best: v.keep_best,
            generations: v.generations,
            children: v.children,
            sevenths: v.sevenths,
            tour_done: v.tour_done,
            record: v.record.settings.clone(),
            look: v.look,
            speak: v.speak,
        }
    }

    /// The settings kept in `storage`, as any studio kept them; the defaults where there are none.
    pub fn load(storage: Option<&dyn eframe::Storage>) -> Prefs {
        Prefs::read(&kept(storage))
    }

    /// The settings of a version's fields, each read on its own: the default's where it is not there or does not read.
    pub fn read(f: &Fields) -> Prefs {
        let d = Prefs::default();
        let record = f.get::<Map<String, Value>>("record").map(Fields::of).unwrap_or_default();
        let r = d.record.clone();
        Prefs {
            tab: f.or("tab", d.tab),
            volume: f.or("volume", d.volume),
            mutate_count: f.or("mutate_count", d.mutate_count),
            keep_best: f.or("keep_best", d.keep_best),
            generations: f.or("generations", d.generations),
            children: f.or("children", d.children),
            sevenths: f.or("sevenths", d.sevenths),
            tour_done: f.or("tour_done", d.tour_done),
            record: panels::record::Settings {
                input: record.or("input", r.input),
                channel: record.or("channel", r.channel),
                stereo: record.or("stereo", r.stereo),
                count_in: record.or("count_in", r.count_in),
                click: record.or("click", r.click),
                role: record.or("role", r.role),
                offset_ms: record.or("offset_ms", r.offset_ms),
                midi_offset_ms: record.or("midi_offset_ms", r.midi_offset_ms),
                midi: record.or("midi", r.midi),
                sound: record.or("sound", r.sound),
                part_role: record.or("part_role", r.part_role),
                grid: record.or("grid", r.grid),
                style: record.or("style", r.style),
            },
            look: f.or("look", d.look),
            speak: f.or("speak", d.speak),
        }
    }

    /// Keeps these settings in `storage`, as this version, with what was kept there that this studio does not know.
    pub fn save(&self, storage: &mut dyn eframe::Storage) {
        let earlier = storage.get_string(KEY).map(|t| Fields::parse(&t)).unwrap_or_default();
        let mut fields = earlier.with(self, VERSION);
        // the record settings as this studio writes them, over the ones it read, whose fields it may not all know
        let mut record = kept_record(storage);
        if let Ok(Value::Object(mine)) = serde_json::to_value(&self.record) {
            record.extend(mine);
        }
        fields.set("record", Value::Object(record));
        storage.set_string(KEY, fields.to_text());
    }
}

/// what was kept in `storage`: the settings as this studio keeps them, or as one before 0.6 kept them, or none
fn kept(storage: Option<&dyn eframe::Storage>) -> Fields {
    let Some(storage) = storage else { return Fields::default() };
    if let Some(text) = storage.get_string(KEY) {
        return migrated(Fields::parse(&text));
    }
    // before 0.6: RON under eframe's key, and no version
    let Some(text) = storage.get_string(eframe::APP_KEY) else { return Fields::default() };
    let Ok(Value::Object(mut map)) = ron::from_str::<Value>(&text) else { return Fields::default() };
    // RON's tab is a name RON reads as nothing without its type: read with it
    #[derive(serde::Deserialize)]
    struct Tabbed {
        tab: Tab,
    }
    if let Ok(Tabbed { tab }) = ron::from_str::<Tabbed>(&text)
        && let Ok(tab) = serde_json::to_value(tab)
    {
        map.insert("tab".into(), tab);
    }
    migrated(Fields::of(map))
}

/// the record settings as they were kept, every field of them
fn kept_record(storage: &dyn eframe::Storage) -> Map<String, Value> {
    storage.get_string(KEY).and_then(|t| Fields::parse(&t).get("record")).unwrap_or_default()
}

/// The fields of an earlier version as this one names them. Version 0 (before 0.6) and 1 name them alike; a version
/// that renames a field renames it here, from the version before it on.
fn migrated(f: Fields) -> Fields {
    f
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use eframe::Storage;

    use super::{KEY, Prefs, VERSION};
    use crate::panels::Tab;
    use crate::theme::Look;

    #[derive(Default)]
    struct Kept(HashMap<String, String>);

    impl Storage for Kept {
        fn get_string(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }
        fn set_string(&mut self, key: &str, value: String) {
            self.0.insert(key.into(), value);
        }
        fn remove_string(&mut self, key: &str) {
            self.0.remove(key);
        }
        fn flush(&mut self) {}
    }

    fn kept(key: &str, text: &str) -> Kept {
        Kept(HashMap::from([(key.to_string(), text.to_string())]))
    }

    /// the settings a studio of claudius 0.5 kept: RON under eframe's key
    const BEFORE_0_6: &str = r#"(tab:Mixer,volume:0.6,mutate_count:6,keep_best:false,generations:4,children:5,sevenths:true,tour_done:true,record:(input:Some("USB Audio"),channel:1,stereo:true,count_in:2,click:true,role:"lead",offset_ms:12.5,midi_offset_ms:3.0,midi:None,sound:"epiano",part_role:"keys",grid:0.25,style:"played"))"#;

    #[test]
    fn the_settings_any_studio_kept_are_read() {
        let p = Prefs::load(Some(&kept(eframe::APP_KEY, BEFORE_0_6)));
        assert_eq!((p.tab, p.volume, p.mutate_count, p.keep_best), (Tab::Mixer, 0.6, 6, false));
        assert_eq!((p.generations, p.children, p.sevenths, p.tour_done), (4, 5, true, true));
        assert_eq!((p.record.input.as_deref(), p.record.channel, p.record.stereo), (Some("USB Audio"), 1, true));
        assert_eq!((p.record.count_in, p.record.role.as_str(), p.record.offset_ms), (2, "lead", 12.5));
        let mut storage = kept(eframe::APP_KEY, BEFORE_0_6);
        p.save(&mut storage);
        assert_eq!(Prefs::load(Some(&storage)), p, "kept again as JSON, and read as they were");
        assert!(storage.get_string(KEY).is_some_and(|t| t.contains(&format!("\"version\": {VERSION}"))));
        assert_eq!(Prefs::load(None), Prefs::default(), "nothing kept: the defaults");
        assert_eq!(p.look, Look::System, "settings from before the looks wear the system's");
        let light = Prefs::load(Some(&kept(KEY, r#"{"version": 1, "look": "light"}"#)));
        assert_eq!(light.look, Look::Light);
        let unknown = Prefs::load(Some(&kept(KEY, r#"{"version": 1, "look": "sepia"}"#)));
        assert_eq!(unknown.look, Look::System, "a look this studio has not");
    }

    #[test]
    fn a_field_that_does_not_read_costs_only_itself() {
        let later = r#"{"version": 9, "tab": "Timeline", "volume": "loud", "children": 7,
            "record": {"channel": 2, "grid": "fine", "monitor": true}, "theme": "light"}"#;
        let p = Prefs::load(Some(&kept(KEY, later)));
        let d = Prefs::default();
        assert_eq!((p.tab, p.volume, p.children), (d.tab, d.volume, 7), "a tab this studio has not, a volume in words");
        assert_eq!((p.record.channel, p.record.grid), (2, d.record.grid));
        let mut storage = kept(KEY, later);
        p.save(&mut storage);
        let text = storage.get_string(KEY).unwrap_or_default();
        assert!(text.contains("\"theme\": \"light\"") && text.contains("\"monitor\": true"), "{text}");
        let earlier = r#"(tab:Graph,volume:2.0,mutate_count:"six")"#;
        let p = Prefs::load(Some(&kept(eframe::APP_KEY, earlier)));
        assert_eq!((p.tab, p.mutate_count), (Tab::Graph, d.mutate_count));
    }
}

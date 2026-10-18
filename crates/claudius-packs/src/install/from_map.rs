//! Instruments from Strudel sample maps (`strudel.json`), one per sound name: a list of files is one sound with
//! variations (`bd:3`), a map from notes to files is a pitched instrument.

use super::*;

/// does a sample-map key name a drum sound (`bd`, `RolandTR909_hh`, `kick` ...)?
pub(super) fn looks_like_drum(key: &str) -> bool {
    let last = key.rsplit(['_', '-']).next().unwrap_or(key).to_ascii_lowercase();
    notes::drum_canonical(&last).is_some() || notes::drum_canonical(&key.to_ascii_lowercase()).is_some()
}

pub(crate) fn instruments_from_map(
    map: &strudel::SampleMap,
    spec: &InstSpec,
    add: &mut dyn FnMut(&str) -> u32,
) -> Vec<InstrumentDef> {
    let mut out = Vec::new();
    for (key, value) in &map.entries {
        if spec.exclude_keys.iter().any(|k| k == key) {
            continue;
        }
        if let Some(keys) = &spec.map_keys
            && !keys.iter().any(|k| k == key)
        {
            continue;
        }
        match value {
            MapValue::List(files) => {
                // a list is one sound with variants (`bd:3`): played as recorded at pitch 60, to its end
                let kind = if spec.kind.as_deref() == Some("pitched") { Kind::Pitched } else { Kind::Drum };
                let variants = files
                    .iter()
                    .map(|f| {
                        let url = strudel::resolve(&map.base, f);
                        Variant {
                            name: stem(f).to_string(),
                            zones: vec![Zone { file: add(&url), root: 60.0, ..Default::default() }],
                        }
                    })
                    .collect();
                let mut s = spec.clone();
                if s.category.is_none() {
                    s.category = Some(if looks_like_drum(key) { "drums".into() } else { "samples".into() });
                }
                out.push(base_def(&s, key, kind, variants));
            }
            MapValue::Pitched(notes_list) => {
                let n = notes_list.iter().map(|(_, f)| f.len()).max().unwrap_or(1);
                let mut variants = Vec::new();
                for v in 0..n {
                    let mut zones: Vec<Zone> = Vec::new();
                    for (note, files) in notes_list {
                        let Some(midi) = notes::parse_note(note).or_else(|| note.parse::<i32>().ok()) else { continue };
                        let f = &files[v % files.len()];
                        let url = strudel::resolve(&map.base, f);
                        zones.push(Zone { file: add(&url), root: midi as f32, ..Default::default() });
                    }
                    if zones.is_empty() {
                        continue;
                    }
                    spread_keys(&mut zones);
                    variants.push(Variant { name: String::new(), zones });
                }
                if variants.is_empty() {
                    continue;
                }
                let mut s = spec.clone();
                if s.category.is_none() {
                    s.category = Some("pitched".into());
                }
                let mut d = base_def(&s, key, Kind::Pitched, variants);
                d.one_shot = spec.one_shot.unwrap_or(false);
                out.push(d);
            }
        }
    }
    out
}

pub(super) fn build_map(ctx: &mut Ctx, spec: &InstSpec) -> Result<Vec<InstrumentDef>> {
    let url = strudel::map_url(spec.map.as_deref().expect("map"))?;
    let json = fetch::get_text(&url)?;
    let mut map = strudel::parse_map(&json, &url)?;
    map.base = spec.map_base.clone().unwrap_or_else(|| strudel::pin_base(&map.base, &url));
    // a curated pack's files are pinned to commits, as its maps are
    let mut unpinned = None;
    let mut add = |u: &str| {
        if unpinned.is_none() && !strudel::pinned(u) {
            unpinned = Some(u.to_string());
        }
        ctx.add_url_file(u)
    };
    let defs = instruments_from_map(&map, spec, &mut add);
    if let Some(u) = unpinned {
        bail!("{}: {u} is not pinned to a commit, so what it serves could change", spec.name);
    }
    if defs.is_empty() {
        bail!("{url}: no sounds in the sample map");
    }
    Ok(defs)
}

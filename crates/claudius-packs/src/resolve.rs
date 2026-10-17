//! Names claudius scripts use, Strudel style: `piano`, `bd` + bank `tr909`, `tr909:bd:2`, `vsco2:violins`,
//! `RolandTR909_bd`, `kick`, plus friendly aliases chosen from whatever is installed.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::model::PackIndex;
use crate::notes::drum_canonical;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InstrumentRef {
    pub pack: String,
    pub instrument: String,
    pub variant: usize,
}

impl std::fmt::Display for InstrumentRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.variant > 0 {
            write!(f, "{}:{}:{}", self.pack, self.instrument, self.variant)
        } else {
            write!(f, "{}:{}", self.pack, self.instrument)
        }
    }
}

/// kits tried, in order, for a bare drum name (`bd`, `kick`, `hh` ...)
pub const KIT_PREFERENCE: &[&str] = &["tr909", "tr808", "muldjord", "tr707", "uzu"];

/// friendly name -> candidates in order of preference (`pack:instrument`); the first installed one wins
pub const ALIASES: &[(&[&str], &[&str])] = &[
    (
        &["piano", "grand_piano", "acoustic_piano", "grand", "concert_grand"],
        &["salamander:piano", "splendid:piano", "freepats:upright_piano", "piano-lite:piano"],
    ),
    (&["upright_piano", "upright"], &["freepats:upright_piano", "salamander:piano"]),
    (
        &["epiano", "electric_piano", "ep", "rhodes", "e_piano"],
        &["jrhodes:rhodes", "epianos:wurlitzer", "freepats:fm_piano"],
    ),
    (&["wurlitzer", "wurli"], &["epianos:wurlitzer", "jrhodes:rhodes"]),
    (&["cp80", "electric_grand"], &["epianos:cp80"]),
    (&["pianet"], &["epianos:pianet"]),
    (&["fm_piano", "dx_piano", "fm_epiano"], &["freepats:fm_piano", "jrhodes:rhodes"]),
    (
        &["organ", "drawbar_organ", "hammond", "b3", "tonewheel_organ", "jazz_organ"],
        &["freepats:drawbar_organ", "vcsl:pipe_organ"],
    ),
    (&["church_organ", "pipe_organ", "cathedral_organ"], &["vcsl:pipe_organ", "freepats:drawbar_organ"]),
    (
        &["strings", "string_ensemble", "strings_sustain", "ensemble_strings", "orchestra_strings"],
        &["vsco2:strings", "freepats:synth_strings"],
    ),
    (&["violin", "violins", "violin_ensemble"], &["vsco2:violins"]),
    (&["viola", "violas", "viola_ensemble"], &["vsco2:violas"]),
    (&["cello", "cellos", "cello_ensemble"], &["vsco2:cellos"]),
    (&["contrabass", "double_bass_arco", "bass_arco"], &["vsco2:contrabass"]),
    (&["pizzicato", "pizz", "strings_pizz", "pizz_strings", "pizzicato_strings"], &["vsco2:strings_pizz"]),
    (&["spiccato", "strings_spic", "strings_staccato", "staccato_strings"], &["vsco2:strings_spic"]),
    (&["brass", "brass_section", "brass_ensemble"], &["vsco2:brass", "freepats:synth_brass"]),
    (&["brass_stac", "brass_staccato", "brass_stabs", "stabs"], &["vsco2:brass_stac"]),
    (&["trumpet"], &["vsco2:trumpet"]),
    (&["horn", "french_horn", "horns"], &["vsco2:horn"]),
    (&["trombone"], &["vsco2:trombone"]),
    (&["tuba"], &["vsco2:tuba"]),
    (&["woodwinds", "woodwind_section", "winds"], &["vsco2:woodwinds"]),
    (&["flute"], &["vsco2:flute"]),
    (&["oboe"], &["vsco2:oboe"]),
    (&["clarinet"], &["vsco2:clarinet"]),
    (&["bassoon"], &["vsco2:bassoon"]),
    (&["piccolo"], &["vsco2:piccolo"]),
    (&["sax", "saxophone", "tenor_sax"], &["vcsl:tenor_sax"]),
    (&["harmonica"], &["vcsl:harmonica"]),
    (&["harp", "concert_harp"], &["vsco2:harp"]),
    (&["marimba"], &["vcsl:marimba"]),
    (&["vibraphone", "vibes"], &["vcsl:vibraphone"]),
    (&["xylophone", "xylo"], &["vcsl:xylophone"]),
    (&["glockenspiel", "glock"], &["vcsl:glockenspiel"]),
    (&["tubular_bells", "bells", "chimes"], &["vcsl:tubular_bells"]),
    (&["kalimba", "mbira", "thumb_piano"], &["vcsl:kalimba"]),
    (&["timpani", "timps"], &["vsco2:timpani"]),
    (&["gong", "tam_tam"], &["vcsl:gong"]),
    (&["suspended_cymbal", "cymbal_swell", "orchestral_cymbal"], &["vcsl:suspended_cymbal"]),
    (
        &["bass", "electric_bass", "bass_guitar", "finger_bass"],
        &["growlybass:bass", "freepats:finger_bass", "doublebass:upright_bass", "freepats:synth_bass"],
    ),
    (&["pick_bass", "picked_bass"], &["freepats:pick_bass", "freepats:finger_bass"]),
    (
        &["upright_bass", "acoustic_bass", "double_bass", "jazz_bass", "doublebass"],
        &["doublebass:upright_bass", "vsco2:contrabass_pizz"],
    ),
    (&["synth_bass"], &["freepats:synth_bass", "freepats:synth_bass_2"]),
    (
        &["guitar", "acoustic_guitar", "nylon_guitar", "classical_guitar", "spanish_guitar"],
        &["freepats:classical_guitar", "freepats:clean_guitar"],
    ),
    (&["electric_guitar", "clean_guitar", "guitar_clean"], &["freepats:clean_guitar", "freepats:jazz_guitar"]),
    (&["jazz_guitar"], &["freepats:jazz_guitar", "freepats:clean_guitar"]),
    (&["choir", "voices", "choir_aah", "choir_ah", "aah"], &["voice:choir", "freepats:choir_pad"]),
    (&["voice", "vocal", "vox", "solo_voice"], &["voice:voice_ah"]),
    (&["pad", "synth_pad"], &["freepats:sweep_pad", "freepats:new_age_pad", "freepats:bowed_pad"]),
    (&["warm_pad", "new_age_pad"], &["freepats:new_age_pad", "freepats:sweep_pad"]),
    (&["bowed_pad"], &["freepats:bowed_pad"]),
    (&["choir_pad", "synth_choir"], &["freepats:choir_pad", "voice:choir"]),
    (&["synth_strings", "string_machine"], &["freepats:synth_strings", "freepats:synth_strings_2"]),
    (&["synth_brass"], &["freepats:synth_brass"]),
    (&["lead", "synth_lead", "square_lead", "square"], &["freepats:square_lead", "freepats:bass_lead"]),
    (&["bongo", "bongos"], &["vcsl:bongos"]),
    (&["conga", "congas"], &["vcsl:congas", "tr808:lc"]),
    (&["cajon"], &["vcsl:cajon"]),
    (&["darbuka", "doumbek"], &["vcsl:darbuka"]),
    (&["frame_drum"], &["vcsl:frame_drum"]),
    (&["tambourine"], &["vcsl:tambourine", "tr707:tb"]),
    (&["shaker"], &["vcsl:shaker", "tr808:sh"]),
    (&["cabasa"], &["vcsl:cabasa"]),
    (&["claves", "clave"], &["vcsl:claves", "tr808:cl"]),
    (&["cowbell"], &["tr808:cb", "vcsl:cowbells", "tr707:cb"]),
    (&["woodblock", "wood_block"], &["vcsl:woodblock"]),
    (&["agogo"], &["vcsl:agogo"]),
    (&["guiro"], &["vcsl:guiro"]),
    (&["triangle"], &["vcsl:triangle"]),
    (&["sleigh_bells", "sleighbells", "jingle_bells"], &["vcsl:sleigh_bells"]),
    (&["vibraslap"], &["vcsl:vibraslap"]),
    (&["mark_tree", "wind_chimes"], &["vcsl:mark_tree"]),
    (&["claps", "hand_claps", "real_clap"], &["vcsl:claps"]),
    (&["concert_bass_drum", "gran_cassa"], &["vcsl:bass_drum"]),
];

pub fn norm(s: &str) -> String {
    s.trim().to_ascii_lowercase().replace([' ', '-'], "_")
}

fn find_pack<'a>(indexes: &'a [Arc<PackIndex>], name: &str) -> Option<&'a Arc<PackIndex>> {
    let n = norm(name);
    indexes
        .iter()
        .find(|p| norm(&p.name) == n)
        .or_else(|| indexes.iter().find(|p| p.aliases.iter().any(|a| norm(a) == n)))
}

/// an instrument of one pack by name, alias, drum synonym or Strudel bank prefix (`RolandTR909_bd`)
fn find_inst(p: &PackIndex, name: &str, bank: Option<&str>) -> Option<usize> {
    let n = norm(name);
    if let Some(i) = p.instruments.iter().position(|d| norm(&d.name) == n) {
        return Some(i);
    }
    if let Some(b) = bank {
        let full = format!("{}_{}", norm(b), n);
        if let Some(i) = p.instruments.iter().position(|d| norm(&d.name) == full) {
            return Some(i);
        }
        if let Some(c) = drum_canonical(&n) {
            let full = format!("{}_{}", norm(b), c);
            if let Some(i) = p.instruments.iter().position(|d| norm(&d.name) == full) {
                return Some(i);
            }
        }
    }
    if let Some(i) = p.instruments.iter().position(|d| d.aliases.iter().any(|a| norm(a) == n)) {
        return Some(i);
    }
    let c = drum_canonical(&n)?;
    p.instruments.iter().position(|d| drum_canonical(&d.name) == Some(c))
}

fn make(p: &PackIndex, i: usize, variant: usize) -> Option<InstrumentRef> {
    let d = &p.instruments[i];
    (variant < d.variants.len()).then(|| InstrumentRef { pack: p.name.clone(), instrument: d.name.clone(), variant })
}

fn by_ref(indexes: &[Arc<PackIndex>], pack: &str, inst: &str, variant: usize) -> Option<InstrumentRef> {
    let p = find_pack(indexes, pack)?;
    let i = p.instruments.iter().position(|d| d.name == inst)?;
    make(p, i, variant)
}

/// Resolve a name against installed packs.
pub fn resolve_in(indexes: &[Arc<PackIndex>], name: &str, bank: Option<&str>) -> Option<InstrumentRef> {
    let mut parts: Vec<&str> = name.trim().split(':').filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        return None;
    }
    let mut variant = 0usize;
    if parts.len() > 1
        && let Ok(v) = parts[parts.len() - 1].trim().parse::<usize>()
    {
        variant = v;
        parts.pop();
    }
    let bank = bank.map(str::trim).filter(|b| !b.is_empty());

    // explicit bank: `s("bd").bank("tr909")`
    if let Some(b) = bank {
        let sound = parts.last().copied().unwrap_or_default();
        if let Some(p) = find_pack(indexes, b)
            && let Some(i) = find_inst(p, sound, None)
        {
            return make(p, i, variant);
        }
        // Strudel bank naming inside map packs: RolandTR909_bd
        for p in indexes {
            if let Some(i) = find_inst(p, &format!("{b}_{sound}"), None).or_else(|| {
                let c = drum_canonical(&norm(sound))?;
                find_inst(p, &format!("{b}_{c}"), None)
            }) && norm(&p.instruments[i].name).starts_with(&norm(b))
            {
                return make(p, i, variant);
            }
        }
        return None;
    }

    // `pack:instrument[:variant]`
    if parts.len() >= 2 {
        let (pk, sound) = (parts[0], parts[1..].join(":"));
        if let Some(p) = find_pack(indexes, pk) {
            return find_inst(p, &sound, None).and_then(|i| make(p, i, variant));
        }
        // `bank:sound` where the bank is a Strudel bank name
        return resolve_in(indexes, &format!("{sound}:{variant}"), Some(pk));
    }
    let n = norm(parts[0]);

    // friendly aliases in preference order
    for (names, cands) in ALIASES {
        if names.iter().any(|a| *a == n) {
            for c in *cands {
                let (pk, inst) = c.split_once(':').expect("alias target");
                if let Some(r) = by_ref(indexes, pk, inst, variant) {
                    return Some(r);
                }
            }
        }
    }
    // bare drum names: the preferred installed kit that has it
    if let Some(c) = drum_canonical(&n) {
        for kit in KIT_PREFERENCE {
            if let Some(p) = find_pack(indexes, kit)
                && let Some(i) = p.instruments.iter().position(|d| d.name == c)
            {
                return make(p, i, variant);
            }
        }
    }
    // exact instrument names and per-instrument aliases, default packs first
    let mut order: Vec<&Arc<PackIndex>> = indexes.iter().collect();
    order.sort_by_key(|p| (!p.default, p.user, p.opt_in));
    for p in &order {
        if let Some(i) = p.instruments.iter().position(|d| norm(&d.name) == n) {
            return make(p, i, variant);
        }
    }
    for p in &order {
        if let Some(i) = p.instruments.iter().position(|d| d.aliases.iter().any(|a| norm(a) == n)) {
            return make(p, i, variant);
        }
    }
    // a drum synonym in any other kit (user-added maps): `kick` -> some pack's `bd`
    if let Some(c) = drum_canonical(&n) {
        for p in &order {
            if let Some(i) = p.instruments.iter().position(|d| drum_canonical(&d.name) == Some(c)) {
                return make(p, i, variant);
            }
        }
    }
    // Strudel combined names: RolandTR909_bd -> bank RolandTR909, sound bd
    if let Some((b, s)) = parts[0].rsplit_once('_')
        && let Some(p) = find_pack(indexes, b)
        && let Some(i) = find_inst(p, s, None)
    {
        return make(p, i, variant);
    }
    // a pack name alone: its first instrument
    if let Some(p) = find_pack(indexes, parts[0])
        && !p.instruments.is_empty()
    {
        return make(p, 0, variant);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{INDEX_FORMAT, InstrumentDef, Kind, Variant, Zone};

    fn def(name: &str, variants: usize, aliases: &[&str]) -> InstrumentDef {
        InstrumentDef {
            name: name.into(),
            category: "x".into(),
            kind: Kind::Drum,
            description: String::new(),
            one_shot: true,
            release: 0.1,
            attack: 0.0,
            gain_db: 0.0,
            veltrack: 0.5,
            vel_curve: Vec::new(),
            mono: false,
            retrigger: false,
            unison: None,
            aliases: aliases.iter().map(|s| s.to_string()).collect(),
            variants: (0..variants).map(|_| Variant { name: String::new(), zones: vec![Zone::default()] }).collect(),
        }
    }

    fn pack(name: &str, aliases: &[&str], insts: Vec<InstrumentDef>, default: bool) -> Arc<PackIndex> {
        Arc::new(PackIndex {
            format: INDEX_FORMAT,
            name: name.into(),
            title: name.into(),
            description: String::new(),
            categories: Vec::new(),
            licence: "CC0-1.0".into(),
            licence_url: String::new(),
            attribution: name.into(),
            attribution_required: false,
            licence_note: String::new(),
            source: String::new(),
            version: "1".into(),
            default,
            opt_in: false,
            warning: None,
            user: false,
            lazy: false,
            installed_at: 0,
            aliases: aliases.iter().map(|s| s.to_string()).collect(),
            files: Vec::new(),
            instruments: insts,
        })
    }

    fn r(p: &str, i: &str, v: usize) -> Option<InstrumentRef> {
        Some(InstrumentRef { pack: p.into(), instrument: i.into(), variant: v })
    }

    #[test]
    fn names_banks_and_aliases() {
        let idx = vec![
            pack("tr909", &["909", "RolandTR909"], vec![def("bd", 5, &[]), def("sd", 3, &[]), def("hh", 2, &[])], true),
            pack("tr808", &["808"], vec![def("bd", 25, &[]), def("cb", 1, &[]), def("sh", 1, &[])], true),
            pack("salamander", &[], vec![def("piano", 1, &["grand_piano"])], true),
            pack("vsco2", &[], vec![def("violins", 1, &["violin_ensemble"]), def("strings", 1, &[])], true),
            pack(
                "tidal-drum-machines",
                &["tdm"],
                vec![def("LinnDrum_bd", 3, &[]), def("RolandTR909_bd", 4, &[])],
                false,
            ),
        ];
        assert_eq!(resolve_in(&idx, "piano", None), r("salamander", "piano", 0));
        assert_eq!(resolve_in(&idx, "bd", Some("tr909")), r("tr909", "bd", 0));
        assert_eq!(resolve_in(&idx, "bd:2", Some("909")), r("tr909", "bd", 2));
        assert_eq!(resolve_in(&idx, "kick", Some("808")), r("tr808", "bd", 0));
        assert_eq!(resolve_in(&idx, "tr909:bd:2", None), r("tr909", "bd", 2));
        assert_eq!(resolve_in(&idx, "tr909:kick", None), r("tr909", "bd", 0));
        assert_eq!(resolve_in(&idx, "bd", None), r("tr909", "bd", 0));
        assert_eq!(resolve_in(&idx, "snare", None), r("tr909", "sd", 0));
        assert_eq!(resolve_in(&idx, "cowbell", None), r("tr808", "cb", 0));
        assert_eq!(resolve_in(&idx, "vsco2:violin_ensemble", None), r("vsco2", "violins", 0));
        assert_eq!(resolve_in(&idx, "violin", None), r("vsco2", "violins", 0));
        assert_eq!(resolve_in(&idx, "Strings", None), r("vsco2", "strings", 0));
        // Strudel banks that only exist in a map pack
        assert_eq!(resolve_in(&idx, "bd", Some("LinnDrum")), r("tidal-drum-machines", "LinnDrum_bd", 0));
        assert_eq!(resolve_in(&idx, "LinnDrum_bd:1", None), r("tidal-drum-machines", "LinnDrum_bd", 1));
        assert_eq!(resolve_in(&idx, "LinnDrum:bd:2", None), r("tidal-drum-machines", "LinnDrum_bd", 2));
        // a curated pack wins over a map pack for the same bank alias
        assert_eq!(resolve_in(&idx, "bd", Some("RolandTR909")), r("tr909", "bd", 0));
        // out of range variants and unknown names
        assert_eq!(resolve_in(&idx, "tr909:bd:9", None), None);
        assert_eq!(resolve_in(&idx, "theremin", None), None);
        assert_eq!(resolve_in(&idx, "bd", Some("nonexistent")), None);
        assert_eq!(resolve_in(&idx, "grand_piano", None), r("salamander", "piano", 0));
    }
}

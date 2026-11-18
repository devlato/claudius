//! The search fields' search by meaning against the embedding model itself, on the genres claudius ships
//! (`crates/claudius/data/genres`): what people ask for in their own words, which the genres' names, aliases and
//! descriptions do not all hold, so that the words alone find nothing, and the genres that answer it. It needs the
//! model's file as the app keeps it (`potion-base-8M.clde`) at `CLAUDIUS_INTENT_CLDE`; without it it is skipped.

use claudius_api::GenreInfo;
use claudius_intent::Model;
use claudius_intent::search::search;

/// the genres claudius ships, as the studio has them
fn genres() -> Vec<GenreInfo> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../claudius/data/genres");
    let mut out: Vec<GenreInfo> = std::fs::read_dir(dir)
        .expect("the genres")
        .filter_map(|e| std::fs::read_to_string(e.ok()?.path()).ok())
        .filter_map(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .map(|g| {
            let s = |k: &str| g[k].as_str().unwrap_or_default().to_string();
            let aliases = g["aliases"]
                .as_array()
                .map_or(Vec::new(), |a| a.iter().filter_map(|x| x.as_str()).map(str::to_string).collect());
            GenreInfo {
                name: s("name"),
                family: s("family"),
                aliases,
                description: s("description"),
                ..GenreInfo::default()
            }
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// what is asked, and the genres any of which answers it
const ASKED: &[(&str, &[&str])] = &[
    ("something to study to", &["lofi"]),
    ("chill music for relaxing", &["ambient", "lofi"]),
    ("sad songs about heartbreak", &["emo_rap", "rnb", "alt_rnb"]),
    ("driving at night in the eighties", &["synthwave"]),
    ("music for a rave in a warehouse", &["techno", "hard_techno"]),
    ("jamaican vibes", &["reggae"]),
    ("brazilian party music", &["baile_funk", "slap_house"]),
    ("old school rap from new york", &["boom_bap", "gangsta_rap", "ny_drill"]),
    ("festival anthem with a huge drop", &["big_room", "progressive_house", "dubstep"]),
    ("spanish club music", &["reggaeton"]),
];

#[test]
fn a_search_field_finds_by_meaning_what_its_words_do_not() {
    let Some(file) = std::env::var_os("CLAUDIUS_INTENT_CLDE") else {
        eprintln!("skipped: CLAUDIUS_INTENT_CLDE names no model file");
        return;
    };
    let model = Model::from_bytes(&std::fs::read(file).expect("the model's file")).expect("the model");
    let genres = genres();
    let (mut found, mut missed) = (0, Vec::new());
    for (asked, answers) in ASKED {
        let by_words = search(asked, "genres", 5, &genres, None, None).unwrap();
        let by_meaning = search(asked, "genres", 5, &genres, None, Some(&model)).unwrap();
        eprintln!("'{asked}': words {:?}, meaning {:?}", by_words.names, by_meaning.names);
        if by_meaning.names.iter().any(|n| answers.contains(&n.as_str())) {
            found += 1;
        } else {
            missed.push(format!("'{asked}': {:?}, not any of {answers:?}", by_meaning.names));
        }
    }
    eprintln!("{found} of {} found in the first five", ASKED.len());
    assert!(found * 10 >= ASKED.len() * 8, "{found} of {}: {missed:#?}", ASKED.len());
}

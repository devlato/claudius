//! What the studio's search fields find, by their words and by what they mean, as the palette finds the actions: the
//! genres (New song's cards, the style a track is written in) and the sounds (the sound pickers). The words find first,
//! as the fields find them themselves (`claudius_api::search_genres`; a sound's name, or what it is); then, once the
//! embedding model is there, those near what the words mean ("a sad song for night driving", "something warm and
//! dusty", "a bright bell") that no word of them names.

use claudius_api::{GenreInfo, Searched, SoundList};

use crate::find::{Item, nearness};
use crate::model::Model;

/// how near in meaning a genre or a sound has to be, besides those the words find, to be offered (`nearness`, 0 to 1)
const NEAR: f32 = 0.25;

/// What `query` finds `among` the genres ("genres") or the sounds ("sounds", of `sounds`), at most `limit`, best first:
/// those its words find, then those near what it means (with `model`).
pub fn search(
    query: &str,
    among: &str,
    limit: usize,
    genres: &[GenreInfo],
    sounds: Option<&SoundList>,
    model: Option<&Model>,
) -> Result<Searched, String> {
    let q = query.trim();
    let (mut names, by_meaning) = match among {
        "genres" => of_genres(q, genres, model),
        "sounds" => of_sounds(q, sounds.ok_or("the sounds are not listed yet")?, model),
        other => return Err(format!("no search among '{other}': genres or sounds")),
    };
    names.truncate(limit);
    Ok(Searched { query: query.to_string(), among: among.to_string(), names, by_meaning })
}

/// the genres `q` finds by its words, then those near what it means
fn of_genres(q: &str, genres: &[GenreInfo], model: Option<&Model>) -> (Vec<String>, bool) {
    let mut names: Vec<String> = claudius_api::search_genres(genres, q).into_iter().map(|g| g.name.clone()).collect();
    // a genre's name as people write it, and each of its other names, its family and each thing its sound is said to
    // be, apart (`nearness` takes the nearest): one of five names, or one line of a long description, is diluted in
    // the rest of them
    let texts: Vec<(String, String)> = genres
        .iter()
        .map(|g| {
            let sound = g.description.split([':', ',', ';']).map(str::trim).filter(|p| !p.is_empty());
            let parts: Vec<&str> =
                g.aliases.iter().map(String::as_str).chain([g.family.as_str()]).chain(sound).collect();
            (g.name.replace('_', " "), parts.join("; "))
        })
        .collect();
    let items: Vec<Item> =
        genres.iter().zip(&texts).map(|(g, (name, words))| Item { id: &g.name, name, words }).collect();
    let near = nearness(q, &items, model);
    then_near(&mut names, &items, near.as_deref());
    (names, near.is_some())
}

/// the sounds `q` finds by their names or what they are, then those near what it means
fn of_sounds(q: &str, sounds: &SoundList, model: Option<&Model>) -> (Vec<String>, bool) {
    let mut all: Vec<&str> = Vec::new();
    for n in sounds.drums.iter().chain(&sounds.kit).chain(&sounds.synths).chain(&sounds.instruments) {
        if !all.contains(&n.as_str()) {
            all.push(n);
        }
    }
    let spoken = |n: &str| n.replace(['_', ':', '-'], " ");
    let lower = q.to_lowercase();
    let says = |s: &str| s.to_lowercase().contains(&lower);
    let mut names: Vec<String> = all
        .iter()
        .filter(|n| says(n) || says(&spoken(n)) || sounds.sound_about(n).is_some_and(says))
        .map(|n| n.to_string())
        .collect();
    // what each is, a thing at a time, as a genre's sound is read
    let about = |n: &str| {
        let parts = sounds.sound_about(n).unwrap_or_default().split([':', ',', ';']).map(str::trim);
        parts.filter(|p| !p.is_empty()).collect::<Vec<_>>().join("; ")
    };
    let texts: Vec<(String, String)> = all.iter().map(|n| (spoken(n), about(n))).collect();
    let items: Vec<Item> = all.iter().zip(&texts).map(|(n, (name, words))| Item { id: n, name, words }).collect();
    let near = nearness(q, &items, model);
    then_near(&mut names, &items, near.as_deref());
    (names, near.is_some())
}

/// `names` and after them, the nearest first, the items near enough what was asked (`NEAR`) that are not among them
fn then_near(names: &mut Vec<String>, items: &[Item], near: Option<&[f32]>) {
    let Some(near) = near else { return };
    let mut order: Vec<usize> =
        (0..items.len()).filter(|&i| near[i] >= NEAR && !names.iter().any(|n| n == items[i].id)).collect();
    order.sort_by(|&a, &b| near[b].total_cmp(&near[a]).then(a.cmp(&b)));
    names.extend(order.into_iter().map(|i| items[i].id.to_string()));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn genre(name: &str, family: &str, aliases: &[&str], description: &str) -> GenreInfo {
        GenreInfo {
            name: name.into(),
            family: family.into(),
            aliases: aliases.iter().map(|a| a.to_string()).collect(),
            description: description.into(),
            ..GenreInfo::default()
        }
    }

    #[test]
    fn the_words_find_first_as_the_fields_find_them_and_without_the_model_only_they_do() {
        let genres = [
            genre("boom_bap", "hip hop", &["90s hip hop"], "boom bap: dusty drums under a sampled loop"),
            genre("lofi", "hip hop", &["lo-fi"], "lofi: a dusty, warm loop to study to"),
            genre("techno", "electronic", &[], "techno: a driving kick, hypnotic and dark"),
        ];
        let found = search("dusty", "genres", 10, &genres, None, None).unwrap();
        assert_eq!((found.names, found.by_meaning), (vec!["boom_bap".to_string(), "lofi".to_string()], false));
        let none = search("dusty", "drums", 10, &genres, None, None).unwrap_err();
        assert!(none.contains("genres or sounds"), "{none}");
        let unlisted = search("bell", "sounds", 10, &genres, None, None).unwrap_err();
        assert!(unlisted.contains("not listed"), "{unlisted}");
    }
}

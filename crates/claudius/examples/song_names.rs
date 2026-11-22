//! The names claudius gives new songs of each genre (`claudius::titles`), a few in a minor key and a few in a major,
//! for whoever tunes the words they are made of.
//!
//!   cargo run -p claudius --example song_names -- [COUNT] [GENRE ...]

use claudius::rng::Rng;
use claudius::theory::scale::{Key, Mode};

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let count: usize = args.first().and_then(|a| a.parse().ok()).unwrap_or(8);
    let only: Vec<&String> = args.iter().filter(|a| a.parse::<usize>().is_err()).collect();
    let mut genres = claudius::genres::all().map_err(|e| e.to_string())?;
    genres.sort_by(|a, b| (&a.family, &a.name).cmp(&(&b.family, &b.name)));
    let mut family = String::new();
    for g in genres.iter().filter(|g| only.is_empty() || only.iter().any(|o| **o == g.name)) {
        if g.family != family {
            family = g.family.clone();
            println!("\n## {family}");
        }
        // each genre its own draw, as each song has its own seed
        let mut rng = Rng::new(g.name.bytes().fold(1u64, |h, b| h.wrapping_mul(31).wrapping_add(u64::from(b))));
        let names: Vec<String> = (0..count)
            .map(|i| {
                let mode = if i % 2 == 0 { Mode::Minor } else { Mode::Major };
                claudius::titles::title(g, Key::new(0, mode), &mut rng)
            })
            .collect();
        println!("- {}: {}", g.name.replace('_', " "), names.join(" · "));
    }
    Ok(())
}

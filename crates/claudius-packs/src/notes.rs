//! Note names, velocity / dynamic markers and round-robin markers, as they appear in sample file names, Strudel
//! map keys and SFZ opcodes. MIDI numbers use scientific pitch: C4 = 60, A4 = 69.

/// Parse a whole string as a note name: `C4`, `c#3`, `Db2`, `As4` / `Ds1` (Tidal `s` = sharp), `Bf1` / `ef3`
/// (Tidal `f` = flat), `F♯2`, `C-1`. Returns the MIDI note number.
pub fn parse_note(s: &str) -> Option<i32> {
    match note_prefix(s) {
        Some((n, "")) => Some(n),
        _ => None,
    }
}

/// Parse a note name at the start of `s` and return it with the unparsed remainder (`A0v8` -> 21, "v8").
pub fn note_prefix(s: &str) -> Option<(i32, &str)> {
    let mut chars = s.char_indices().peekable();
    let (_, letter) = chars.next()?;
    let base = match letter.to_ascii_lowercase() {
        'c' => 0,
        'd' => 2,
        'e' => 4,
        'f' => 5,
        'g' => 7,
        'a' => 9,
        'b' => 11,
        _ => return None,
    };
    let mut semis = base;
    let mut pos = letter.len_utf8();
    // up to two accidentals; `b` / `f` / `s` only count as accidentals when an octave number follows
    for _ in 0..2 {
        let rest = &s[pos..];
        let Some(c) = rest.chars().next() else { break };
        let delta = match c {
            '#' | '♯' => 1,
            '♭' => -1,
            's' | 'S' | 'b' | 'f'
                if starts_with_octave(&rest[c.len_utf8()..]) || is_accidental_then_octave(&rest[c.len_utf8()..]) =>
            {
                if c == 's' || c == 'S' { 1 } else { -1 }
            }
            _ => break,
        };
        semis += delta;
        pos += c.len_utf8();
    }
    let rest = &s[pos..];
    let (oct, used) = parse_octave(rest)?;
    let midi = (oct + 1) * 12 + semis;
    if !(-12..=140).contains(&midi) {
        return None;
    }
    Some((midi, &rest[used..]))
}

fn starts_with_octave(s: &str) -> bool {
    parse_octave(s).is_some()
}

fn is_accidental_then_octave(s: &str) -> bool {
    let mut it = s.chars();
    matches!(it.next(), Some('#' | '♯' | '♭' | 'b' | 's')) && starts_with_octave(it.as_str())
}

/// octave number: optional '-', one or two digits
fn parse_octave(s: &str) -> Option<(i32, usize)> {
    let b = s.as_bytes();
    let mut i = 0;
    let neg = b.first() == Some(&b'-');
    if neg {
        i += 1;
    }
    let start = i;
    while i < b.len() && b[i].is_ascii_digit() && i - start < 2 {
        i += 1;
    }
    if i == start {
        return None;
    }
    let v: i32 = s[start..i].parse().ok()?;
    if v > 10 {
        return None;
    }
    Some((if neg { -v } else { v }, i))
}

/// An SFZ key: a MIDI number or a note name (SFZ also uses C4 = 60).
pub fn parse_sfz_key(s: &str) -> Option<i32> {
    let t = s.trim();
    if let Ok(v) = t.parse::<i32>() {
        return Some(v);
    }
    parse_note(t)
}

pub fn note_name(midi: i32) -> String {
    const N: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    format!("{}{}", N[midi.rem_euclid(12) as usize], midi.div_euclid(12) - 1)
}

/// What a file name says about a sample.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NameInfo {
    pub note: Option<i32>,
    /// ordinal of the dynamic / velocity marker (higher = louder); comparable within one instrument
    pub vel: Option<f32>,
    pub rr: Option<u32>,
}

/// Dynamic words in increasing loudness.
fn dynamic_rank(t: &str) -> Option<f32> {
    Some(match t {
        "pppp" => 0.5,
        "ppp" => 1.0,
        "pp" => 2.0,
        "p" | "piano" => 3.0,
        "mp" => 4.0,
        "mf" => 5.0,
        "f" | "forte" => 6.0,
        "ff" => 7.0,
        "fff" => 8.0,
        "ffff" => 8.5,
        "soft" | "quiet" | "light" | "gentle" | "lo" | "low" => 2.0,
        "med" | "medium" | "mid" | "normal" | "norm" => 5.0,
        "hard" | "loud" | "heavy" | "hi" | "high" | "strong" => 7.0,
        _ => return None,
    })
}

fn split_alpha_num(t: &str) -> Option<(&str, u32)> {
    let i = t.find(|c: char| c.is_ascii_digit())?;
    let (a, n) = t.split_at(i);
    if n.chars().all(|c| c.is_ascii_digit()) && n.len() <= 4 { Some((a, n.parse().ok()?)) } else { None }
}

fn vel_token(t: &str) -> Option<f32> {
    let l = t.to_ascii_lowercase();
    if let Some((a, n)) = split_alpha_num(&l)
        && matches!(a, "v" | "vl" | "vel" | "velo" | "velocity" | "layer" | "dyn" | "accent" | "acc")
    {
        return Some(n as f32);
    }
    None
}

fn rr_token(t: &str) -> Option<u32> {
    let l = t.to_ascii_lowercase();
    if let Some((a, n)) = split_alpha_num(&l)
        && matches!(a, "rr" | "seq" | "take" | "alt" | "var" | "rnd")
    {
        return Some(n);
    }
    None
}

/// Analyse a file stem (no directory, no extension): `VlnEns_Pizz_A2_v1_rr1`, `JHPiano_Sus_Close_A#0_vl4_rr1`,
/// `A0v8`, `Marimba_hit_Outrigger_B2_soft_01`, `LDFlute_expvib_A3_v1_1`, `vowel_a_eb3`.
pub fn analyze_name(stem: &str) -> NameInfo {
    let mut info = NameInfo::default();
    let tokens: Vec<&str> = stem.split(['_', ' ', '.', '-']).filter(|t| !t.is_empty()).collect();
    let mut after_note_numbers: Vec<u32> = Vec::new();
    for (i, tok) in tokens.iter().enumerate() {
        let t = *tok;
        if let Some(v) = vel_token(t) {
            info.vel = Some(v);
            continue;
        }
        if let Some(r) = rr_token(t) {
            info.rr = Some(r);
            continue;
        }
        let low = t.to_ascii_lowercase();
        // single-letter dynamics (`p`, `f`) are too ambiguous unless they sit after the note
        // "piano", "low", "high" ... before the note are part of the instrument name
        let name_word = info.note.is_none() && matches!(low.as_str(), "piano" | "lo" | "low" | "hi" | "high" | "mid");
        if let Some(d) = dynamic_rank(&low)
            && (low.len() > 1 || info.note.is_some())
            && !name_word
        {
            info.vel = Some(d);
            continue;
        }
        // a negative octave is split off by '-' ("C-1"); glue it back
        if t.len() <= 2 && tokens.get(i + 1).is_some_and(|n| n.len() == 1 && n.chars().all(|c| c.is_ascii_digit())) {
            let glued = format!("{t}-{}", tokens[i + 1]);
            if stem.contains(&glued)
                && let Some(n) = parse_note(&glued)
                && n < 12
            {
                info.note = Some(n);
                continue;
            }
        }
        if let Some((n, rest)) = note_prefix(t) {
            if rest.is_empty() {
                info.note = Some(n);
                after_note_numbers.clear();
                continue;
            }
            // fused markers after the note: A0v8, C4_vl2 handled above, D#3rr2
            let rl = rest.to_ascii_lowercase();
            if let Some(v) = vel_token(&rl) {
                info.note = Some(n);
                info.vel = Some(v);
                continue;
            }
            if let Some(r) = rr_token(&rl) {
                info.note = Some(n);
                info.rr = Some(r);
                continue;
            }
            if let Some(d) = dynamic_rank(&rl) {
                info.note = Some(n);
                info.vel = Some(d);
                continue;
            }
        }
        if info.note.is_some() && t.chars().all(|c| c.is_ascii_digit()) && t.len() <= 3 {
            after_note_numbers.push(t.parse().unwrap_or(0));
        }
    }
    if info.rr.is_none()
        && let Some(&n) = after_note_numbers.last()
    {
        info.rr = Some(n);
    }
    info
}

/// Canonical drum abbreviations (Strudel / Tidal) and their synonyms.
pub const DRUM_NAMES: &[(&str, &[&str])] = &[
    ("bd", &["kick", "kd", "bassdrum", "bass_drum", "kickdrum", "kick_drum", "bdrum"]),
    ("sd", &["snare", "snaredrum", "snare_drum", "sn"]),
    ("rim", &["rimshot", "rim_shot", "rs", "sidestick", "side_stick", "stick", "cross_stick"]),
    ("cp", &["clap", "claps", "handclap", "hand_clap", "hc_clap"]),
    ("hh", &["hihat", "hi_hat", "ch", "chh", "closed_hat", "closedhat", "closed_hihat", "hat", "hats"]),
    ("oh", &["open_hat", "openhat", "ohh", "open_hihat"]),
    ("cr", &["crash", "cymbal", "cy", "crash_cymbal"]),
    ("rd", &["ride", "ride_cymbal"]),
    ("rb", &["ride_bell", "bell_ride"]),
    ("lt", &["low_tom", "tom_low", "floor_tom", "ft", "lowtom"]),
    ("mt", &["mid_tom", "tom_mid", "tom", "midtom"]),
    ("ht", &["high_tom", "hi_tom", "tom_high", "hitom", "hightom"]),
    ("cb", &["cowbell", "cow_bell"]),
    ("sh", &["shaker", "maracas", "ma", "maraca"]),
    ("tb", &["tambourine", "tamb", "tambo"]),
    ("cl", &["clave", "claves"]),
    ("perc", &["percussion"]),
    ("misc", &[]),
    ("fx", &["sfx", "effect"]),
];

/// Canonical drum abbreviation for a drum name or synonym (`kick` -> `bd`).
pub fn drum_canonical(name: &str) -> Option<&'static str> {
    let n = name.to_ascii_lowercase();
    for (c, syn) in DRUM_NAMES {
        if n == *c || syn.contains(&n.as_str()) {
            return Some(c);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_names() {
        assert_eq!(parse_note("C4"), Some(60));
        assert_eq!(parse_note("c4"), Some(60));
        assert_eq!(parse_note("A4"), Some(69));
        assert_eq!(parse_note("A0"), Some(21));
        assert_eq!(parse_note("C8"), Some(108));
        assert_eq!(parse_note("c#3"), Some(49));
        assert_eq!(parse_note("Db2"), Some(37));
        assert_eq!(parse_note("db2"), Some(37));
        assert_eq!(parse_note("As4"), Some(70));
        assert_eq!(parse_note("Ds1"), Some(27));
        assert_eq!(parse_note("Bf1"), Some(34));
        assert_eq!(parse_note("ef3"), Some(51));
        assert_eq!(parse_note("bb3"), Some(58));
        assert_eq!(parse_note("Ab1"), Some(32));
        assert_eq!(parse_note("F♯2"), Some(42));
        assert_eq!(parse_note("C-1"), Some(0));
        assert_eq!(parse_note("G9"), Some(127));
        assert_eq!(parse_note("B#3"), Some(60));
        assert_eq!(parse_note("Cb4"), Some(59));
        assert_eq!(parse_note("H4"), None);
        assert_eq!(parse_note("C"), None);
        assert_eq!(parse_note("bd"), None);
        assert_eq!(parse_note("C4x"), None);
        assert_eq!(parse_note("f"), None);
        assert_eq!(parse_note("f2"), Some(41));
        assert_eq!(parse_sfz_key("60"), Some(60));
        assert_eq!(parse_sfz_key("c4"), Some(60));
        assert_eq!(parse_sfz_key("Gb5"), Some(78));
        assert_eq!(note_name(60), "C4");
        assert_eq!(note_name(21), "A0");
        assert_eq!(note_name(0), "C-1");
    }

    #[test]
    fn note_prefixes() {
        assert_eq!(note_prefix("A0v8"), Some((21, "v8")));
        assert_eq!(note_prefix("Ds1v8"), Some((27, "v8")));
        assert_eq!(note_prefix("C#4rr2"), Some((61, "rr2")));
        assert_eq!(note_prefix("Fs5v12"), Some((78, "v12")));
    }

    #[test]
    fn file_names() {
        let i = analyze_name("VlnEns_Pizz_A2_v1_rr2");
        assert_eq!((i.note, i.vel, i.rr), (Some(45), Some(1.0), Some(2)));
        let i = analyze_name("JHPiano_Sus_Close_A#0_vl4_rr1");
        assert_eq!((i.note, i.vel, i.rr), (Some(22), Some(4.0), Some(1)));
        let i = analyze_name("A0v8");
        assert_eq!((i.note, i.vel, i.rr), (Some(21), Some(8.0), None));
        let i = analyze_name("C4v16");
        assert_eq!((i.note, i.vel), (Some(60), Some(16.0)));
        let i = analyze_name("Marimba_hit_Outrigger_B2_soft_01");
        assert_eq!((i.note, i.vel, i.rr), (Some(47), Some(2.0), Some(1)));
        let i = analyze_name("Marimba_hit_Outrigger_B2_loud_01");
        assert_eq!(i.vel, Some(7.0));
        let i = analyze_name("LDFlute_expvib_A3_v1_1");
        assert_eq!((i.note, i.vel, i.rr), (Some(57), Some(1.0), Some(1)));
        let i = analyze_name("vowel_a_eb3");
        assert_eq!(i.note, Some(51));
        let i = analyze_name("Piano 1_C0_vl3");
        assert_eq!((i.note, i.vel), (Some(12), Some(3.0)));
        let i = analyze_name("BassDrum909-accent070-tune100-attack100-decay100");
        assert_eq!((i.note, i.vel), (None, Some(70.0)));
        let i = analyze_name("Sum_SHTrumpet_harmonM-sus_A#2_v1_rr1");
        assert_eq!((i.note, i.vel, i.rr), (Some(46), Some(1.0), Some(1)));
        let i = analyze_name("piano_C4_ff");
        assert_eq!((i.note, i.vel), (Some(60), Some(7.0)));
        let i = analyze_name("Bassdrum-01");
        assert_eq!(i.note, None);
    }

    #[test]
    fn drum_synonyms() {
        assert_eq!(drum_canonical("kick"), Some("bd"));
        assert_eq!(drum_canonical("BD"), Some("bd"));
        assert_eq!(drum_canonical("snare"), Some("sd"));
        assert_eq!(drum_canonical("closed_hat"), Some("hh"));
        assert_eq!(drum_canonical("open_hat"), Some("oh"));
        assert_eq!(drum_canonical("clap"), Some("cp"));
        assert_eq!(drum_canonical("rimshot"), Some("rim"));
        assert_eq!(drum_canonical("piano"), None);
    }
}

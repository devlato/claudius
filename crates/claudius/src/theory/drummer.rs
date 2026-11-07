//! Drummers: how each genre's drummers end a phrase. A drummer's turns end a four-bar phrase (a kick pushed into the
//! next bar, a ghost note dragged into the backbeat, the kick left out of the last beat); its fills end an eight-bar
//! phrase and a section (kick and snare trading eighths, triplet snares, a snare rush, toms down the kit, everything
//! stopped for a beat). Each is written lane by lane, in cells that end where the phrase does:
//!
//! | step | what sounds |
//! |---|---|
//! | `X`, `x`, `o` | a hit: accented, played, a ghost note |
//! | `h`, `m`, `l`, `f` | a hit pitched high, mid, low, on the floor, as a drummer goes down the toms |
//! | `.` | a rest: what the lane played there is left out |
//!
//! A cell is in sixteenths, unless its steps follow `3:` (eighth-note triplets), `6:` (sixteenth-note triplets) or
//! `8:` (thirty-seconds); a `<` at its start plays them louder and louder. A fill names its lanes by what they play
//! (`kick`, `snare`, `clap`, `hat`, `ohat`, `ride`, `tom`, `rim`, `perc`, `shaker`) and plays on the song's lane that
//! plays it, or the nearest it has: a snare fill on the clap or the rim, toms on the congas or pitched on the snare.
//! While it plays, the lanes it leaves alone play on, or the drummer's hands leave the hats and the ride
//! (`Rest::Hands`), or they all stop (`Rest::All`).

use crate::rng::Rng;

/// what the lanes a fill leaves alone do while it plays
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rest {
    /// they play on
    None,
    /// the hats and the ride stop: the drummer's hands are on the fill
    Hands,
    /// they all stop: a break
    All,
}

/// a phrase's ending, lane by lane
#[derive(Clone, Copy, Debug)]
pub struct Fill {
    pub name: &'static str,
    /// (lane, cell): what each lane it plays on plays, ending where the phrase does
    pub lanes: &'static [(&'static str, &'static str)],
    pub rest: Rest,
}

/// how a genre's drummers end their phrases
#[derive(Clone, Copy, Debug)]
pub struct Drummer {
    pub name: &'static str,
    pub about: &'static str,
    /// the endings of a four-bar phrase: small, a lane or two
    pub turns: &'static [Fill],
    /// the endings of an eight-bar phrase, and of a section
    pub fills: &'static [Fill],
    /// the chance a four-bar phrase ends with a turn, and an eight-bar one or a section with a fill
    pub turn: f64,
    pub fill: f64,
}

const fn fill(name: &'static str, lanes: &'static [(&'static str, &'static str)], rest: Rest) -> Fill {
    Fill { name, lanes, rest }
}

const KICK_OUT: Fill = fill("kick out", &[("kick", "....")], Rest::None);
const KICK_DROP: Fill = fill("kick drop", &[("kick", "........")], Rest::None);
const TAG: Fill = fill("tag", &[("kick", "....")], Rest::All);
const RUSH: Fill = fill("rush", &[("snare", "<xxxxxxxx")], Rest::None);
const STUTTER: Fill = fill("stutter", &[("snare", "<8:xxxxxxxx")], Rest::None);
const TRIPLETS: Fill = fill("triplets", &[("snare", "3:xxx")], Rest::None);
const GHOSTS: Fill = fill("ghosts", &[("snare", "Xo.o")], Rest::None);
const TOMS: Fill = fill("toms", &[("tom", "hhmmllff")], Rest::Hands);
const FOUR_ON_THE_SNARE: Fill = fill("four on the snare", &[("snare", "X...X...X...X...")], Rest::None);
const EDIT: Fill = fill("edit", &[("kick", "x..x...."), ("snare", ".x..Xoxx")], Rest::Hands);

/// The drummers, each as its genre's play.
pub const DRUMMERS: &[Drummer] = &[
    Drummer {
        name: "boom_bap",
        about: "boom bap's: an MPC chopping a break; a kick pushed into the next bar, a ghost dragged into the backbeat, kick and snare trading eighths, the beat stopped for a beat",
        turns: &[
            fill("pickup", &[("kick", "...x")], Rest::None),
            fill("double", &[("kick", "x.x.....")], Rest::None),
            fill("drag", &[("snare", "oX...")], Rest::None),
        ],
        fills: &[
            fill("trade", &[("kick", "x...x..."), ("snare", "..X...X.")], Rest::Hands),
            fill("snare triple", &[("snare", "X.xx")], Rest::None),
            fill("stop", &[("kick", "x.......")], Rest::All),
            TAG,
        ],
        turn: 0.5,
        fill: 0.75,
    },
    Drummer {
        name: "trap",
        about: "trap's: hi-hat rolls in thirty-seconds and triplets, snares rolled into the next bar, the beat dropped out for the producer's tag",
        turns: &[
            fill("hat roll", &[("hat", "<8:xxxxxxxx")], Rest::None),
            fill("triplet hats", &[("hat", "6:xxxxxx")], Rest::None),
            fill("skip", &[("kick", "..xx")], Rest::None),
        ],
        fills: &[
            fill("triplet snares", &[("snare", "3:xxx"), ("hat", "6:xxxxxx")], Rest::None),
            fill("snare roll", &[("snare", "<6:xxxxxxxxxxxx")], Rest::None),
            TAG,
            fill("drop out", &[("kick", "........")], Rest::All),
        ],
        turn: 0.6,
        fill: 0.8,
    },
    Drummer {
        name: "drill",
        about: "drill's: hats skipping in triplets, snares flammed late in the bar, a kick sliding with the 808, the beat dropped before the next",
        turns: &[
            fill("triplet hats", &[("hat", "3:xxx")], Rest::None),
            fill("hat skip", &[("hat", "6:x.xx.x")], Rest::None),
            fill("slide", &[("kick", "..x.")], Rest::None),
        ],
        fills: &[
            fill("flams", &[("snare", "6:...xxX")], Rest::None),
            fill("kick and snares", &[("kick", "3:x.."), ("snare", "3:.xx")], Rest::None),
            TAG,
        ],
        turn: 0.6,
        fill: 0.8,
    },
    Drummer {
        name: "house",
        about: "house's: a clap pushed onto the last sixteenth, the open hat lifted, the kick left out for a beat or two, claps rolled up into the next phrase, a conga run",
        turns: &[
            fill("clap pickup", &[("clap", "X..x")], Rest::None),
            KICK_OUT,
            fill("open lift", &[("ohat", "..xx")], Rest::None),
        ],
        fills: &[
            fill("clap roll", &[("clap", "<xxxxxxxx")], Rest::None),
            KICK_DROP,
            fill("conga run", &[("perc", "h.hm.ml.")], Rest::None),
        ],
        turn: 0.45,
        fill: 0.7,
    },
    Drummer {
        name: "techno",
        about: "techno's: hardly a fill; the kick left out for a beat, half a bar or a bar while the hats and percussion run on, a soft clap rising",
        turns: &[KICK_OUT, fill("perc skip", &[("perc", ".x.x")], Rest::None)],
        fills: &[
            KICK_DROP,
            fill("bar drop", &[("kick", "................")], Rest::None),
            fill("clap rise", &[("clap", "<oooooooo")], Rest::None),
        ],
        turn: 0.35,
        fill: 0.6,
    },
    Drummer {
        name: "trance",
        about: "trance's: a snare rolled up through the phrase's last bar, in eighths and then sixteenths, the kick dropped out under it",
        turns: &[KICK_OUT, fill("clap pickup", &[("clap", "X..x")], Rest::None)],
        fills: &[
            fill("snare roll", &[("snare", "<xxxxxxxxxxxxxxxx")], Rest::None),
            fill("eighths to sixteenths", &[("snare", "<x.x.x.x.xxxxxxxx")], Rest::None),
            fill("roll and drop", &[("snare", "<xxxxxxxx"), ("kick", "........")], Rest::None),
        ],
        turn: 0.4,
        fill: 0.7,
    },
    Drummer {
        name: "dnb",
        about: "drum and bass's and jungle's: a break edited; snare rushes, ghost notes after the backbeat, the kick pushed onto the and, the last beat stuttered",
        turns: &[fill("ghosts", &[("snare", "X.oo")], Rest::None), fill("kick push", &[("kick", "..x.")], Rest::None)],
        fills: &[RUSH, EDIT, STUTTER],
        turn: 0.6,
        fill: 0.85,
    },
    Drummer {
        name: "reggae",
        about: "reggae's: a rim pushed into the next bar, the open hat on the and, the snare (or the rim) rolled in triplets into the drop, toms down in triplets",
        turns: &[fill("rim pickup", &[("rim", "..xx")], Rest::None), fill("open and", &[("ohat", "..x.")], Rest::None)],
        fills: &[
            fill("triplet roll", &[("snare", "3:xxX")], Rest::Hands),
            fill("toms in triplets", &[("tom", "3:hhmmlf")], Rest::Hands),
        ],
        turn: 0.5,
        fill: 0.65,
    },
    Drummer {
        name: "dembow",
        about: "reggaeton's: the timbal run into the next bar, the rim pushing ahead, a stop on the last beat",
        turns: &[fill("timbal", &[("snare", "xxxx")], Rest::None), fill("rim push", &[("rim", "...x")], Rest::None)],
        fills: &[
            fill("timbal roll", &[("tom", "hhmmhhll")], Rest::None),
            fill("stop", &[("kick", "x...")], Rest::All),
            RUSH,
        ],
        turn: 0.5,
        fill: 0.75,
    },
    Drummer {
        name: "afro",
        about: "afrobeats' and amapiano's: the congas and talking drum running into the next bar, the shaker left out for a beat, the kick dropped under the log drum",
        turns: &[
            fill("conga pickup", &[("perc", "..hm")], Rest::None),
            fill("shaker out", &[("shaker", "....")], Rest::None),
            fill("rim pickup", &[("rim", ".x.x")], Rest::None),
        ],
        fills: &[
            fill("conga run", &[("perc", "h.hmh.ml")], Rest::None),
            KICK_DROP,
            fill("talking drum", &[("perc", "<6:hhmhml")], Rest::None),
        ],
        turn: 0.5,
        fill: 0.65,
    },
    Drummer {
        name: "garage",
        about: "UK garage's and grime's: the snare and rim skipping, the kick pushed onto the and, a shuffled run, a beat stopped",
        turns: &[
            fill("skip", &[("snare", ".o.x")], Rest::None),
            fill("kick skip", &[("kick", "..x.")], Rest::None),
            fill("rim skip", &[("rim", "x..x")], Rest::None),
        ],
        fills: &[fill("shuffle", &[("snare", "x.xx.xx.")], Rest::None), TAG],
        turn: 0.5,
        fill: 0.65,
    },
    Drummer {
        name: "halftime",
        about: "dubstep's and future bass's half-time: the kick doubled, snare triplets, rushes and stutters into the drop, a beat of silence before it",
        turns: &[fill("kick pickup", &[("kick", "..xx")], Rest::None), fill("ghost", &[("snare", "...o")], Rest::None)],
        fills: &[TRIPLETS, RUSH, STUTTER, TAG],
        turn: 0.45,
        fill: 0.8,
    },
    Drummer {
        name: "rock",
        about: "a rock drummer's: the snare doubled, the kick pushed ahead, toms down the kit from the high tom to the floor, the hands off the hats while they play",
        turns: &[fill("double", &[("snare", "X.x.")], Rest::None), fill("push", &[("kick", "...x")], Rest::None)],
        fills: &[
            TOMS,
            fill("snare to toms", &[("snare", "xxxx...."), ("tom", "....mmff")], Rest::Hands),
            fill("bar", &[("snare", "x.x.xxxx........"), ("tom", "........hhmmllff")], Rest::Hands),
            fill("snare sixteenths", &[("snare", "<xxxx")], Rest::None),
        ],
        turn: 0.5,
        fill: 0.8,
    },
    Drummer {
        name: "funk",
        about: "funk's: ghost notes around the backbeat, kicks on the and and the a, sixteenths on the snare accented in threes, four on the snare into the next section",
        turns: &[
            GHOSTS,
            fill("kicks", &[("kick", "..xx")], Rest::None),
            fill("drag", &[("snare", "oX.o.")], Rest::None),
        ],
        fills: &[fill("threes", &[("snare", "XooXooXo")], Rest::Hands), FOUR_ON_THE_SNARE, TOMS],
        turn: 0.55,
        fill: 0.75,
    },
    Drummer {
        name: "disco",
        about: "disco's: the snare pushed onto the last sixteenth, the open hat on the and, sixteenths on the snare, four on the snare, a tom run",
        turns: &[fill("pickup", &[("snare", "X..x")], Rest::None), fill("open", &[("ohat", "..x.")], Rest::None)],
        fills: &[fill("snare sixteenths", &[("snare", "<xxxxxxxx")], Rest::None), FOUR_ON_THE_SNARE, TOMS],
        turn: 0.4,
        fill: 0.7,
    },
    Drummer {
        name: "jazzy",
        about: "lo-fi's, jazz rap's and neo-soul's: lazy and soft; ghost notes around the backbeat, a kick late on the and, the hats left out for a beat of space, a soft roll",
        turns: &[
            GHOSTS,
            fill("lazy kick", &[("kick", "..x.")], Rest::None),
            fill("space", &[("hat", "....")], Rest::None),
        ],
        fills: &[
            fill("drag", &[("snare", "X.oo")], Rest::None),
            fill("soft roll", &[("snare", "<oooooooo")], Rest::Hands),
            TAG,
        ],
        turn: 0.45,
        fill: 0.55,
    },
    Drummer {
        name: "synthwave",
        about: "synthwave's drum machine: the gated snare doubled, the kick pushed, toms down the kit in sixteenths, a snare roll into the chorus",
        turns: &[fill("double", &[("snare", "X..x")], Rest::None), fill("push", &[("kick", "..x.")], Rest::None)],
        fills: &[
            TOMS,
            fill("snare to toms", &[("snare", "xxxx...."), ("tom", "....hmlf")], Rest::Hands),
            fill("snare sixteenths", &[("snare", "<xxxxxxxx")], Rest::None),
        ],
        turn: 0.4,
        fill: 0.8,
    },
    Drummer {
        name: "breakbeat",
        about: "big beat's, trip hop's and IDM's breaks, chopped: ghost notes after the backbeat, kick and snare edits, the snare rushed or stuttered, a stop",
        turns: &[fill("ghosts", &[("snare", "X.oo")], Rest::None), fill("push", &[("kick", "..x.")], Rest::None)],
        fills: &[EDIT, RUSH, STUTTER, TAG],
        turn: 0.5,
        fill: 0.75,
    },
    Drummer {
        name: "rnb",
        about: "R&B's drum programmers: hat rolls in triplets, a ghost snare before the one, the snare dragged or in triplets, a stop on the last beat",
        turns: &[
            fill("hat triplets", &[("hat", "6:xxxxxx")], Rest::None),
            fill("ghost", &[("snare", "...o")], Rest::None),
        ],
        fills: &[fill("drag", &[("snare", "X.ox")], Rest::None), TRIPLETS, TAG],
        turn: 0.45,
        fill: 0.65,
    },
    Drummer {
        name: "steady",
        about: "a drummer who keeps time and fills now and then: the kick left out for a beat, a roll on the snare, triplets",
        turns: &[KICK_OUT],
        fills: &[fill("roll", &[("snare", "<xxxx")], Rest::None), TRIPLETS, RUSH],
        turn: 0.3,
        fill: 0.6,
    },
];

/// the drummer of that name
pub fn get(name: &str) -> Option<&'static Drummer> {
    DRUMMERS.iter().find(|d| d.name == name)
}

/// The drummer of genre `genre` (of a fusion, its last genre's, which plays its drums), else one who keeps time and
/// fills now and then.
pub fn of(genre: &str) -> &'static Drummer {
    let name = match genre.rsplit('+').next().unwrap_or(genre).trim() {
        "boom_bap" | "alt_hip_hop" | "chipmunk_soul" | "gangsta_rap" | "hip_hop" => "boom_bap",
        "trap" | "cloud_rap" | "emo_rap" | "rage" | "plugg" | "phonk" | "crunk" => "trap",
        "uk_drill" | "ny_drill" | "chicago_drill" => "drill",
        "house" | "deep_house" | "tech_house" | "filter_house" | "electro_house" => "house",
        "techno" | "dub_techno" => "techno",
        "trance" => "trance",
        "dnb" | "jungle" => "dnb",
        "reggae" => "reggae",
        "reggaeton" => "dembow",
        "afrobeats" | "amapiano" => "afro",
        "uk_garage" | "grime" => "garage",
        "dubstep" | "future_bass" => "halftime",
        "art_rock" | "neo_psychedelia" => "rock",
        "funk" | "g_funk" | "new_jack_swing" => "funk",
        "nu_disco" | "dance_pop" => "disco",
        "lofi" | "jazz_rap" | "neo_soul" => "jazzy",
        "synthwave" => "synthwave",
        "big_beat" | "idm" | "trip_hop" => "breakbeat",
        "rnb" | "rnb_90s" | "alt_rnb" => "rnb",
        _ => "steady",
    };
    get(name).unwrap_or(&DRUMMERS[DRUMMERS.len() - 1])
}

/// a hit of a cell: when it sounds from the cell's start, in beats; how hard; how many semitones off the lane's pitch
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stroke {
    pub t: f64,
    pub v: f32,
    pub pitch: i32,
}

/// A cell's strokes, the beats it lasts and its grid, or what is wrong with it.
fn parse(cell: &str) -> Result<(Vec<Stroke>, f64, f64), String> {
    let (rising, rest) = match cell.strip_prefix('<') {
        Some(r) => (true, r),
        None => (false, cell),
    };
    let (grid, rest) = match rest.split_once(':') {
        Some(("3", r)) => (1.0 / 3.0, r),
        Some(("6", r)) => (1.0 / 6.0, r),
        Some(("8", r)) => (0.125, r),
        Some((g, _)) => return Err(format!("'{cell}': no grid '{g}:' (3: 6: 8:, or none for sixteenths)")),
        None => (0.25, rest),
    };
    let steps: Vec<char> = rest.chars().filter(|c| !c.is_whitespace()).collect();
    let n = steps.len();
    let beats = n as f64 * grid;
    if n == 0 || ((beats * 4.0) - (beats * 4.0).round()).abs() > 1e-9 {
        return Err(format!("'{cell}': {n} steps, {beats:.3} beats, where a cell lasts whole sixteenths"));
    }
    let mut out = Vec::new();
    for (i, c) in steps.iter().enumerate() {
        let (v, pitch) = match c {
            'X' => (1.0, 0),
            'x' => (0.8, 0),
            'o' => (0.4, 0),
            'h' => (0.85, 7),
            'm' => (0.85, 3),
            'l' => (0.85, -2),
            'f' => (0.9, -6),
            '.' => continue,
            other => return Err(format!("'{cell}': no step '{other}' (X x o h m l f .)")),
        };
        let v = if rising { v * (0.5 + 0.5 * (i + 1) as f32 / n as f32) } else { v };
        out.push(Stroke { t: i as f64 * grid, v, pitch });
    }
    Ok((out, beats, grid))
}

const LANES: &[&str] = &["kick", "snare", "clap", "hat", "ohat", "ride", "tom", "rim", "perc", "shaker"];

/// Whether every turn and fill of `drummer` reads, or the first that does not and why.
pub fn check(drummer: &Drummer) -> Result<(), String> {
    for f in drummer.turns.iter().chain(drummer.fills) {
        if f.lanes.is_empty() {
            return Err(format!("{}: {} plays on no lane", drummer.name, f.name));
        }
        for (lane, cell) in f.lanes {
            if !LANES.contains(lane) {
                return Err(format!("{}: {}: no lane '{lane}' ({})", drummer.name, f.name, LANES.join(" ")));
            }
            parse(cell).map_err(|e| format!("{}: {}: {e}", drummer.name, f.name))?;
        }
    }
    Ok(())
}

/// The lane of the song's `lanes` (name and role, as `DrumLane::role` says) that a fill's lane `word` plays on: the
/// one that plays it, else the nearest the song has.
fn lane_for(word: &str, lanes: &[(String, String)]) -> Option<usize> {
    let role = |r: &str| lanes.iter().position(|(_, role)| role == r);
    let name = |n: &str| lanes.iter().position(|(name, _)| name == n);
    match word {
        "kick" => role("kick"),
        "snare" => role("snare").or_else(|| role("clap")).or_else(|| name("rim")),
        "clap" => role("clap").or_else(|| role("snare")),
        "hat" => role("hat").or_else(|| role("ride")),
        "ohat" => role("ohat").or_else(|| role("hat")),
        "ride" => role("ride").or_else(|| role("hat")),
        "tom" => role("tom").or_else(|| name("conga")).or_else(|| name("perc")).or_else(|| role("snare")),
        "rim" => name("rim").or_else(|| role("snare")),
        "perc" => name("conga").or_else(|| name("perc")).or_else(|| role("perc")).or_else(|| role("tom")),
        "shaker" => role("shaker"),
        _ => None,
    }
}

/// what one of the song's lanes plays of a fill: the lane (its index), from when, its strokes at beats of the song, on
/// what grid
#[derive(Clone, Debug)]
pub struct Play {
    pub lane: usize,
    pub from: f64,
    pub strokes: Vec<Stroke>,
    pub grid: f64,
}

/// How a phrase ending at beat `at` ends: what the lanes a fill plays on play, and which lanes stop from `at - span`
/// to `at`.
#[derive(Clone, Debug)]
pub struct Ending {
    pub name: &'static str,
    pub at: f64,
    pub span: f64,
    pub plays: Vec<Play>,
    pub stops: Vec<usize>,
}

/// `f` on the song's `lanes`, ending at beat `at`, or None when it names a lane the song has nothing like
fn place(f: &'static Fill, lanes: &[(String, String)], at: f64) -> Option<Ending> {
    let mut plays = Vec::new();
    for (word, cell) in f.lanes {
        let lane = lane_for(word, lanes)?;
        let (strokes, beats, grid) = parse(cell).ok()?;
        let from = at - beats;
        let strokes = strokes.into_iter().map(|s| Stroke { t: from + s.t, ..s }).collect();
        plays.push(Play { lane, from, strokes, grid });
    }
    let span = plays.iter().map(|p| at - p.from).fold(0.0, f64::max);
    let hands = |r: &str| matches!(r, "hat" | "ohat" | "ride");
    let stops = (0..lanes.len())
        .filter(|i| !plays.iter().any(|p| p.lane == *i))
        .filter(|&i| match f.rest {
            Rest::None => false,
            Rest::Hands => hands(&lanes[i].1),
            Rest::All => true,
        })
        .collect();
    Some(Ending { name: f.name, at, span, plays, stops })
}

/// How `drummer` ends the phrases of a section `bars` long from beat `start`, on the song's drum `lanes` (name and
/// role), drawn from `rng`: a turn at the end of each four bars, a fill at the end of each eight and of the section,
/// at `last` (where its drums stop: its end, or the gap before the next section; None for a section that builds,
/// whose roll ends it), each as often as the drummer plays one, and never the same twice running when it knows
/// another.
pub fn endings(
    drummer: &Drummer,
    lanes: &[(String, String)],
    start: f64,
    bars: u32,
    last: Option<f64>,
    rng: &mut Rng,
) -> Vec<Ending> {
    let mut out: Vec<Ending> = Vec::new();
    for k in (1..=bars).filter(|&k| k.is_multiple_of(4) || k == bars) {
        let at = match last {
            _ if k < bars => start + k as f64 * 4.0,
            Some(at) => at,
            None => continue,
        };
        let big = k.is_multiple_of(8) || k == bars;
        let kind = if big && rng.chance(drummer.fill) {
            drummer.fills
        } else if rng.chance(drummer.turn) {
            drummer.turns
        } else {
            continue;
        };
        let mut can: Vec<Ending> = kind.iter().filter_map(|f| place(f, lanes, at)).collect();
        if can.len() > 1
            && let Some(last) = out.last()
        {
            can.retain(|e| e.name != last.name);
        }
        if !can.is_empty() {
            let i = rng.below(can.len());
            out.push(can.swap_remove(i));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kit(lanes: &[(&str, &str)]) -> Vec<(String, String)> {
        lanes.iter().map(|(n, r)| (n.to_string(), r.to_string())).collect()
    }

    #[test]
    fn every_drummer_reads_and_every_genre_has_its_own() {
        for d in DRUMMERS {
            check(d).unwrap();
            assert!(!d.about.is_empty() && !d.fills.is_empty() && !d.turns.is_empty(), "{}", d.name);
        }
        let mut names: Vec<&str> = DRUMMERS.iter().map(|d| d.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), DRUMMERS.len());
        for g in crate::genres::builtin_names().filter(|g| *g != "ambient") {
            assert_ne!(of(g).name, "steady", "{g} has no drummer of its own");
        }
        assert_eq!(of("trip_hop+reggaeton").name, "dembow", "a fusion's drums are its last genre's");
        assert!(parse("6:x").unwrap_err().contains("whole sixteenths"));
        assert!(parse("3:xx").unwrap_err().contains("whole sixteenths"));
        assert!(parse("5:xxxxx").unwrap_err().contains("no grid '5:'"));
        assert!(parse("x.Q.").unwrap_err().contains("no step 'Q'"));
    }

    #[test]
    fn a_cell_ends_where_the_phrase_does_on_its_grid() {
        let lanes = kit(&[("kick", "kick"), ("snare", "snare"), ("hat", "hat")]);
        let tag = place(&TAG, &lanes, 32.0).unwrap();
        assert_eq!((tag.span, tag.stops.clone()), (1.0, vec![1, 2]), "everything stops for the last beat");
        assert!(tag.plays[0].strokes.is_empty(), "the kick too: a beat of silence");
        let triplets = place(&TRIPLETS, &lanes, 32.0).unwrap();
        let t: Vec<f64> = triplets.plays[0].strokes.iter().map(|s| s.t).collect();
        assert!(
            t.len() == 3 && t.iter().zip([31.0, 31.0 + 1.0 / 3.0, 31.0 + 2.0 / 3.0]).all(|(a, b)| (a - b).abs() < 1e-9)
        );
        let rush = place(&RUSH, &lanes, 16.0).unwrap();
        let v: Vec<f32> = rush.plays[0].strokes.iter().map(|s| s.v).collect();
        assert!(rush.plays[0].from == 14.0 && v.windows(2).all(|w| w[0] < w[1]), "two beats, louder and louder");
        let toms = place(&TOMS, &lanes, 8.0).unwrap();
        let pitches: Vec<i32> = toms.plays[0].strokes.iter().map(|s| s.pitch).collect();
        assert!(pitches.windows(2).all(|w| w[0] >= w[1]) && pitches[0] > pitches[7], "down the kit: {pitches:?}");
        assert_eq!((toms.plays[0].lane, toms.stops.clone()), (1, vec![2]), "on the snare, pitched; hands off the hat");
    }

    #[test]
    fn a_fill_plays_on_the_nearest_lane_the_song_has_or_not_at_all() {
        // reggae's kit: the rim plays the snare's fills, and nothing plays toms
        let reggae = kit(&[("kick", "kick"), ("rim", "perc"), ("hat", "hat"), ("ohat", "ohat")]);
        assert_eq!(lane_for("snare", &reggae), Some(1));
        assert_eq!(lane_for("tom", &reggae), None);
        assert!(place(&TOMS, &reggae, 4.0).is_none());
        let house = kit(&[("kick", "kick"), ("clap", "clap"), ("hat", "hat"), ("perc", "perc")]);
        assert_eq!((lane_for("snare", &house), lane_for("tom", &house)), (Some(1), Some(3)));
        assert_eq!(lane_for("ohat", &house), Some(2), "an open hat's part on the closed one");
    }

    #[test]
    fn phrases_end_with_turns_every_four_bars_and_fills_every_eight_and_at_the_section_end() {
        const ALWAYS: Drummer = Drummer { turn: 1.0, fill: 1.0, ..DRUMMERS[0] };
        let lanes = kit(&[("kick", "kick"), ("snare", "snare"), ("hat", "hat")]);
        let ends = endings(&ALWAYS, &lanes, 0.0, 16, Some(64.0), &mut Rng::new(3));
        let at: Vec<f64> = ends.iter().map(|e| e.at).collect();
        assert_eq!(at, vec![16.0, 32.0, 48.0, 64.0]);
        let is_fill = |e: &Ending| ALWAYS.fills.iter().any(|f| f.name == e.name);
        assert_eq!(ends.iter().map(is_fill).collect::<Vec<_>>(), vec![false, true, false, true]);
        assert!(ends.windows(2).all(|w| w[0].name != w[1].name));
        // a section that builds ends on its roll; one of six bars ends on a fill, and one with a gap before the
        // next section fills up to it
        assert_eq!(endings(&ALWAYS, &lanes, 0.0, 8, None, &mut Rng::new(3)).len(), 1);
        let six = endings(&ALWAYS, &lanes, 0.0, 6, Some(24.0), &mut Rng::new(3));
        assert!(six.len() == 2 && six[1].at == 24.0 && is_fill(&six[1]));
        let gap = endings(&ALWAYS, &lanes, 32.0, 8, Some(63.0), &mut Rng::new(3));
        assert_eq!(gap.iter().map(|e| e.at).collect::<Vec<_>>(), vec![48.0, 63.0]);
    }
}

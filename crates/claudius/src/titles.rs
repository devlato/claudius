//! A song's title, as the songs of its genre are titled. Each family of genres has words of its own, and a genre with a
//! world of its own (lo-fi, phonk, drill, ambient, synthwave, reggaeton and others) has its own: a word or two alone, a
//! line someone might say, "The ...", what is said of something, a place, a time. A song in a minor key leans to the
//! darker words and one in a major key to the brighter; a genre whose songs are written in lowercase or in capitals is
//! written so. Never a band's name, and none of the best-known songs' titles (the tests hold a list).

use crate::genres::GenreProfile;
use crate::rng::Rng;
use crate::theory::scale::Key;

/// The words a family of genres, or a genre, titles its songs with.
struct Words {
    /// titles alone: a word or a few
    alone: &'static [&'static str],
    /// what a title is about, darker and brighter
    dark: &'static [&'static str],
    bright: &'static [&'static str],
    /// what is said of what it is about ("Heavy", "Slow")
    said: &'static [&'static str],
    /// lines someone might say, or sing
    lines: &'static [&'static str],
    /// where, and when
    places: &'static [&'static str],
    times: &'static [&'static str],
    /// English, which "The ..." and what is said before a word need: a language whose adjectives follow the word
    /// titles with its words alone, its lines, its places and its times
    english: bool,
}

/// the ways a title is put, and how often each
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shape {
    Alone,
    Said,
    The,
    Line,
    Place,
    Time,
}

const SHAPES: &[(Shape, f64)] = &[
    (Shape::Alone, 3.0),
    (Shape::Said, 2.0),
    (Shape::The, 1.0),
    (Shape::Line, 2.0),
    (Shape::Place, 1.0),
    (Shape::Time, 1.0),
];

/// how often a title is about the words of the song's own mood (dark in a minor key, bright in a major)
const OWN_MOOD: f64 = 0.75;

/// A title for a song of genre `g` in `key`, drawn with `rng`.
pub fn title(g: &GenreProfile, key: Key, rng: &mut Rng) -> String {
    let w = words(g);
    let shapes: Vec<(Shape, f64)> = SHAPES
        .iter()
        .copied()
        .filter(|(s, _)| w.english || matches!(s, Shape::Alone | Shape::Line | Shape::Place | Shape::Time))
        .collect();
    let minor = key.mode.is_minor();
    let own = rng.chance(OWN_MOOD);
    let about = if own == minor { w.dark } else { w.bright };
    let text = match *rng.pick_weighted(&shapes) {
        Shape::Alone => rng.pick(w.alone).to_string(),
        // what is said of a word is never the word itself ("Midnight Midnight")
        Shape::Said => {
            let said = *rng.pick(w.said);
            let nouns: Vec<&str> = about.iter().copied().filter(|n| !n.eq_ignore_ascii_case(said)).collect();
            match nouns.is_empty() {
                true => rng.pick(w.alone).to_string(),
                false => format!("{said} {}", rng.pick(&nouns)),
            }
        }
        Shape::The => format!("The {}", rng.pick(about)),
        Shape::Line => rng.pick(w.lines).to_string(),
        Shape::Place => rng.pick(w.places).to_string(),
        Shape::Time => rng.pick(w.times).to_string(),
    };
    match case(&genre_of(g), rng) {
        Case::Lower => text.to_lowercase(),
        Case::Upper => text.to_uppercase(),
        Case::AsWritten => capitalized(&text),
    }
}

/// the genre a song's words come from: a fusion's first
fn genre_of(g: &GenreProfile) -> String {
    g.name.split('+').next().unwrap_or_default().to_string()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Case {
    AsWritten,
    Lower,
    Upper,
}

/// how a genre's songs are written: lo-fi, cloud rap, emo rap and plugg in lowercase; hyperpop and alternative R&B
/// half the time; phonk and rage in capitals half the time
fn case(genre: &str, rng: &mut Rng) -> Case {
    match genre {
        "lofi" | "cloud_rap" | "emo_rap" | "plugg" => Case::Lower,
        "hyperpop" | "alt_rnb" if rng.chance(0.5) => Case::Lower,
        "phonk" | "rage" if rng.chance(0.5) => Case::Upper,
        _ => Case::AsWritten,
    }
}

/// `s` with its first letter a capital ("the Back Room" begins a title as "The Back Room")
fn capitalized(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
}

/// the words of `g`: its own where it has a world of its own, else its family's
fn words(g: &GenreProfile) -> &'static Words {
    own_words(&genre_of(g)).or_else(|| family_words(&g.family)).unwrap_or(&ELECTRONIC)
}

fn own_words(genre: &str) -> Option<&'static Words> {
    Some(match genre {
        "lofi" => &LOFI,
        "phonk" => &PHONK,
        "chicago_drill" | "ny_drill" | "uk_drill" => &DRILL,
        "cloud_rap" | "emo_rap" | "plugg" => &CLOUD,
        "rage" => &RAGE,
        "ambient" => &AMBIENT,
        "synthwave" => &SYNTHWAVE,
        "trip_hop" => &TRIP_HOP,
        "trance" => &TRANCE,
        "reggae" => &REGGAE,
        "reggaeton" => &REGGAETON,
        "baile_funk" => &BAILE_FUNK,
        "amapiano" => &AMAPIANO,
        _ => return None,
    })
}

fn family_words(family: &str) -> Option<&'static Words> {
    Some(match family {
        "hip hop" => &HIP_HOP,
        "house" => &HOUSE,
        "techno and trance" => &TECHNO,
        "drum and bass" => &DNB,
        "bass music" => &BASS,
        "edm" => &EDM,
        "hard dance" => &HARD_DANCE,
        "pop and disco" => &POP,
        "r&b, soul and funk" => &SOUL,
        "rock" => &ROCK,
        "afro and latin" => &AFROBEATS,
        "electronic" => &ELECTRONIC,
        _ => return None,
    })
}

const HIP_HOP: Words = Words {
    alone: &[
        "Overtime",
        "Receipts",
        "Concrete",
        "Day Ones",
        "Paperwork",
        "Mileage",
        "Hometown",
        "Payday",
        "North End",
        "Corner Store",
        "Momentum",
        "Track Record",
        "Bodega",
        "Rooftops",
        "Long Story Short",
        "Loose Change",
        "Fine Print",
        "Full Circle",
        "Ground Floor",
        "Cold Front",
        "Back Seat",
        "Brick by Brick",
        "Small Talk",
        "Big Picture",
        "Side Streets",
        "Blue Collar",
        "Two Cents",
        "Dusty Crates",
        "Old Tapes",
        "Late Fees",
    ],
    dark: &[
        "Smoke", "Static", "Sirens", "Shadows", "Gravel", "Ashes", "Fog", "Pressure", "Storms", "Debt", "Ghosts",
        "Rain",
    ],
    bright: &[
        "Sunlight",
        "Summer",
        "Gold",
        "Daylight",
        "Blessings",
        "Weekends",
        "Wins",
        "Laughter",
        "Sundays",
        "Family",
    ],
    said: &[
        "Heavy", "Quiet", "Loud", "Slow", "Cold", "Golden", "Late", "Real", "Young", "Lucky", "Tired", "Hungry",
        "Restless",
    ],
    lines: &[
        "Keep Going",
        "Still Here",
        "No Days Off",
        "Hold It Down",
        "Pull Up Slow",
        "Long Way Home",
        "Count It Up",
        "Say Less",
        "Took a While",
        "Made It Out",
        "Watch Me Move",
        "Never Folded",
        "Let 'Em Talk",
        "On the Way Up",
        "Don't Wait Up",
    ],
    places: &[
        "Route 9",
        "Exit 14",
        "Block 22",
        "the Back Room",
        "Fourth and Main",
        "Platform 3",
        "the Ninth Floor",
        "the East End",
        "Southside",
        "the Corner",
    ],
    times: &[
        "3:17",
        "Friday Night",
        "Late Summer",
        "'96",
        "Tuesday",
        "Early Hours",
        "Midweek",
        "Winter '03",
        "Half Past Two",
        "Last Call",
    ],
    english: true,
};

const LOFI: Words = Words {
    alone: &[
        "rainy window",
        "study hall",
        "tea at four",
        "slow mornings",
        "paper cranes",
        "warm socks",
        "late bus",
        "library corner",
        "sleepy cat",
        "porch light",
        "notebook doodles",
        "cassette dust",
        "after class",
        "blanket fort",
        "radiator hum",
        "foggy glasses",
        "laundry day",
        "quiet street",
        "last train",
        "window seat",
    ],
    dark: &["rain", "fog", "puddles", "grey skies", "dusk", "static", "sleep", "winter", "clouds", "shadows"],
    bright: &["sunbeams", "coffee", "spring", "petals", "daydreams", "honey", "mornings", "gardens", "kites", "breeze"],
    said: &["sleepy", "warm", "soft", "slow", "hazy", "quiet", "lazy", "dusty", "faded", "little"],
    lines: &[
        "stay in",
        "five more minutes",
        "don't wake me",
        "maybe later",
        "deep breaths",
        "take it slow",
        "come back soon",
        "let it pour",
        "not yet",
        "see you sunday",
    ],
    places: &[
        "kyoto",
        "room 4b",
        "the attic",
        "the bakery",
        "rooftop garden",
        "old bookstore",
        "the park bench",
        "seaside town",
        "grandma's kitchen",
        "the laundromat",
    ],
    times: &[
        "4pm",
        "sunday",
        "november",
        "past midnight",
        "rainy tuesday",
        "dawn",
        "half past six",
        "early spring",
        "snow day",
        "last bus home",
    ],
    english: true,
};

const PHONK: Words = Words {
    alone: &[
        "Drift",
        "Memphis Fog",
        "Night Drive",
        "Tokyo Run",
        "Burnout",
        "Red Line",
        "Overdrive",
        "Ghost Ride",
        "Sideways",
        "Smoke Show",
        "Ice Road",
        "Underpass",
        "Toll Road",
        "Handbrake",
        "Back Alley",
        "Street Racer",
        "Smoke Screen",
        "Burnt Rubber",
        "Dead End",
        "Lowered",
        "Cold Engine",
    ],
    dark: &[
        "Fog",
        "Smoke",
        "Ghosts",
        "Sirens",
        "Chrome",
        "Ashes",
        "Venom",
        "Phantoms",
        "Skulls",
        "Nights",
        "Shadows",
        "Blood Moons",
    ],
    bright: &["Headlights", "Speed", "Gold", "Fire", "Sparks", "Neon", "Chrome Lights"],
    said: &["Cold", "Dead", "Midnight", "Wicked", "Haunted", "Low", "Slow", "Evil", "Dark", "Cursed"],
    lines: &[
        "Hit the Gas",
        "Ride Out",
        "Stay Low",
        "No Brakes",
        "Let It Slide",
        "Run It Back",
        "Pull Up",
        "Go Dark",
        "Floor It",
        "Pull Off",
        "Stay Gone",
        "Ride Slow",
    ],
    places: &["Memphis", "Route 44", "the Docks", "Lot 13", "the Underpass", "Exit 9", "Lot 7", "the Overpass"],
    times: &["4AM", "Midnight", "After Dark", "Witching Hour", "1993", "3:33", "Late Night"],
    english: true,
};

const DRILL: Words = Words {
    alone: &[
        "Cold Hands",
        "Winter Coat",
        "Black Gloves",
        "No Talking",
        "Back Roads",
        "Night Watch",
        "Grey Skies",
        "Cold Snap",
        "Postcode",
        "Head Down",
        "Ends",
        "Last Stop",
        "Corner Shop",
        "Hoods Up",
        "Low Profile",
        "Cold Breath",
        "Overcast",
    ],
    dark: &["Ice", "Frost", "Smoke", "Steel", "Shadows", "Nights", "Rain", "Fog", "Concrete", "Winter"],
    bright: &["Gold", "Wins", "Day Ones", "Family", "Daylight"],
    said: &["Cold", "Grey", "Silent", "Heavy", "Late", "Real", "Frozen", "Hard"],
    lines: &[
        "Say Nothing",
        "Stay Ready",
        "Keep It Moving",
        "Mind Your Business",
        "Don't Text Me",
        "Eyes Peeled",
        "Move Smart",
        "Two Steps Ahead",
        "Never Switch",
    ],
    places: &[
        "Southside",
        "the Ends",
        "Block 9",
        "Flat 12",
        "Tower Block",
        "the Estate",
        "Bus 43",
        "Ward Road",
        "Station Road",
    ],
    times: &["4AM", "Winter", "Late Night", "Friday", "January", "Half Four"],
    english: true,
};

const CLOUD: Words = Words {
    alone: &[
        "half asleep",
        "pink skies",
        "starlit",
        "headspace",
        "no signal",
        "glass heart",
        "hoodie weather",
        "ghosting",
        "airplane mode",
        "low battery",
        "mood ring",
        "orbit",
        "cotton candy",
        "skyline",
        "vapor",
    ],
    dark: &["rain", "ghosts", "smoke", "tears", "static", "void", "winter", "dark", "nightmares", "scars"],
    bright: &[
        "clouds",
        "stars",
        "sunsets",
        "angels",
        "butterflies",
        "heaven",
        "candy",
        "cherries",
        "bubbles",
        "glitter",
    ],
    said: &["lonely", "faded", "numb", "floating", "pink", "blurry", "sleepy", "cold", "empty", "dreamy"],
    lines: &[
        "don't call me",
        "i'm fine",
        "left on read",
        "can't sleep",
        "stay up",
        "float away",
        "come back",
        "let me be",
        "still on you",
        "miss u",
    ],
    places: &[
        "cloud 8",
        "the backseat",
        "my room",
        "the rooftop",
        "space",
        "the mall",
        "the lake",
        "nowhere",
        "the void",
    ],
    times: &[
        "4am",
        "forever",
        "tonight",
        "summer '19",
        "last summer",
        "midnight",
        "december",
        "after school",
        "sunrise",
    ],
    english: true,
};

const RAGE: Words = Words {
    alone: &[
        "Supernova",
        "Overload",
        "Warp Speed",
        "Flatline",
        "Shockwave",
        "Static Shock",
        "Zero Gravity",
        "Mosh Pit",
        "Afterburn",
        "Alarm",
        "Full Throttle",
        "Meltdown",
        "Launch Pad",
        "Voltage",
        "Kill Switch",
        "Mosh",
        "Glitch Mode",
        "Rager",
        "Static Rage",
        "Rocket Fuel",
    ],
    dark: &["Venom", "Chaos", "Riots", "Static", "Voltage", "Smoke", "Sirens", "Thunder", "Rage", "Fire"],
    bright: &["Stars", "Lasers", "Diamonds", "Rockets", "Lightning", "Neon", "Glitter"],
    said: &["Toxic", "Electric", "Loud", "Wild", "Feral", "Hyper", "Crazy", "Insane", "Fast"],
    lines: &[
        "Turn It Up",
        "Go Wild",
        "Lose It",
        "Sleep Later",
        "Jump Out",
        "Run It Up",
        "Shake the Room",
        "All Gas",
        "Jump Off",
        "Turn Up Now",
        "Break the Speaker",
    ],
    places: &["Outer Space", "Mars", "the Pit", "Area 51", "the Roof", "the Main Stage", "Venus"],
    times: &["Tonight", "Midnight", "Countdown", "Day Zero", "Overtime", "Late Night", "2AM"],
    english: true,
};

const SOUL: Words = Words {
    alone: &[
        "Slow Burn",
        "Velvet",
        "Honeycomb",
        "Silk",
        "Candlelight",
        "Late Talk",
        "Late Checkout",
        "Low Light",
        "Room Service",
        "Body Language",
        "Love Notes",
        "Sweet Talk",
        "Falling Easy",
        "Satin",
        "Unspoken",
        "Soft Spot",
        "Overflow",
        "Mind Wandering",
        "Warm Hands",
        "Second Chances",
    ],
    dark: &["Rain", "Heartache", "Shadows", "Distance", "Silence", "Midnight", "Tears", "Goodbyes", "Storms", "Regret"],
    bright: &["Sunshine", "Honey", "Sugar", "Kisses", "Roses", "Summer", "Butterflies", "Laughter", "Heaven", "Glow"],
    said: &["Sweet", "Slow", "Soft", "Warm", "Tender", "Lonely", "Golden", "Late", "Gentle", "Easy"],
    lines: &[
        "Stay Till Morning",
        "Call Me Back",
        "Closer Still",
        "No Rush",
        "Slow Down Love",
        "Come Over",
        "Say It Again",
        "Love Me Slow",
        "Drive With Me",
        "Wait for Me",
        "Right Here",
    ],
    places: &[
        "Room 304",
        "the Back Booth",
        "Your Place",
        "the Balcony",
        "the Seventh Floor",
        "the Lobby",
        "Harbor Road",
        "Uptown",
    ],
    times: &[
        "2AM",
        "Sunday Night",
        "Late July",
        "After the Party",
        "Saturday",
        "Rainy Season",
        "Twilight",
        "Half Past One",
    ],
    english: true,
};

const HOUSE: Words = Words {
    alone: &[
        "Floorwork",
        "Warehouse",
        "Backroom",
        "Lightshow",
        "Basement",
        "Afterparty",
        "Lift",
        "Groove Science",
        "Body Heat",
        "Soul Shaker",
        "Low End",
        "Deep End",
        "Jack Box",
        "Sunrise Set",
        "Night Bus",
        "Guest List",
        "Disco Ball",
        "Dancer",
        "Moving Floors",
        "Released",
    ],
    dark: &["Smoke", "Fog", "Shadows", "Basements", "Dark Rooms", "Midnight", "Echoes", "Strobes"],
    bright: &["Sunrise", "Lights", "Summer", "Love", "Freedom", "Joy", "Gold", "Daylight"],
    said: &["Deep", "Warm", "Slow", "Higher", "Late", "Endless", "Sweet", "Rough", "Raw", "Soulful"],
    lines: &[
        "Don't Stop Dancing",
        "Feel It",
        "Keep Moving",
        "One More Track",
        "Let Loose",
        "Take Me Higher",
        "Till the Lights Come On",
        "Get Lost",
        "Stay Up Late",
    ],
    places: &["Warehouse 9", "the Basement", "Room 2", "Studio B", "Pier 4", "the Loft", "Dockside", "Canal Street"],
    times: &["6AM", "Sunrise", "Saturday", "Last Hour", "Late Summer", "Early Morning", "Half Past Five", "Friday"],
    english: true,
};

const TECHNO: Words = Words {
    alone: &[
        "Machine Room",
        "Monolith",
        "Concrete",
        "Signal Path",
        "Steelworks",
        "Pulse Width",
        "Substation",
        "Floodlight",
        "Transit",
        "Interface",
        "Cold Storage",
        "Corridor",
        "Relay",
        "Vector",
        "Ground Loop",
    ],
    dark: &["Smoke", "Steel", "Concrete", "Fog", "Rust", "Voltage", "Static", "Nights", "Void", "Iron"],
    bright: &["Light", "Horizon", "Skies", "Daybreak", "Prisms", "Polar Lights", "Sun"],
    said: &["Endless", "Hollow", "Distant", "Cold", "Infinite", "Raw", "Deep", "Parallel", "Inner", "Silent"],
    lines: &[
        "Keep Running",
        "Don't Look Down",
        "Go Further",
        "Stay Awake",
        "Never Stop",
        "Fall Into It",
        "Let It Run",
        "Hold Still",
    ],
    places: &["Hall 4", "Tunnel 9", "Platform 2", "the Bunker", "Sector 7", "Grid 12", "the Substation", "Unit 3"],
    times: &["5AM", "Sunday 6AM", "Hour Nine", "Daybreak", "No Daylight", "Midnight", "Afterhours"],
    english: true,
};

const TRANCE: Words = Words {
    alone: &[
        "Skyward",
        "Polar Light",
        "Glow",
        "Horizons",
        "Lighthouse",
        "Daybreak",
        "Higher Still",
        "Starfall",
        "Ocean Air",
        "Weight of Air",
        "Silver Sky",
        "Tidal",
        "Ascent",
        "Solstice",
        "Open Skies",
    ],
    dark: &["Shadows", "Tides", "Rain", "Night", "Storms", "Silence", "Distance", "Echoes"],
    bright: &["Light", "Stars", "Sun", "Wings", "Skies", "Dawn", "Heaven", "Colours"],
    said: &["Endless", "Eternal", "Bright", "Silver", "Golden", "Distant", "Infinite", "Blue"],
    lines: &[
        "Lift Me Away",
        "Never Let Go",
        "Fly With Me",
        "Hold On to Me",
        "Awaken",
        "Carry Me",
        "Rise Again",
        "Find Me",
        "Breathe Deep",
    ],
    places: &["Sea of Glass", "the Lighthouse", "Island 9", "the Cliffs", "Outer Bay", "the Coast"],
    times: &["Sunrise", "First Light", "Midsummer", "Daybreak", "4AM", "Equinox"],
    english: true,
};

const DNB: Words = Words {
    alone: &[
        "Night Bus",
        "City Limits",
        "Pressure",
        "Tower Block",
        "Rolling Stock",
        "Overground",
        "Soundclash",
        "Sub Zero",
        "Rewind",
        "Liquid Gold",
        "Rush Hour",
        "Junction",
        "Rooftop Run",
        "Frequency",
        "Afterburner",
        "Skyline",
        "Riverside",
        "Breakbeat",
        "Pressure Point",
        "Junglist",
        "Bassline Pressure",
        "Sub Rumble",
        "Late Train",
        "Concrete Rain",
    ],
    dark: &[
        "Sirens", "Smoke", "Fog", "Static", "Shadows", "Steel", "Rain", "Darkness", "Pressure", "Midnight", "Thunder",
    ],
    bright: &["Sunshine", "Lights", "Summer", "Gold", "Skylines", "Colours", "Sunrise", "Daylight"],
    said: &["Rolling", "Heavy", "Deep", "Fast", "Late", "Liquid", "Raw", "Urban", "Electric", "Dark", "Cold"],
    lines: &[
        "Rewind It",
        "Pull It Up",
        "Keep Rolling",
        "Run It",
        "Don't Stop Now",
        "Hold Tight",
        "Lock It Down",
        "Come Alive",
        "Gone Too Far",
        "Bring It Back",
        "Keep It Moving",
        "Let It Roll",
    ],
    places: &[
        "Zone 2",
        "the Underground",
        "Platform 9",
        "Southbank",
        "Night Bus 88",
        "Tower 4",
        "the Viaduct",
        "Bristol",
        "Elephant and Castle",
    ],
    times: &["2AM", "Friday", "Late Summer", "'94", "Midnight", "Monday Morning", "6AM", "Late November"],
    english: true,
};

const BASS: Words = Words {
    alone: &[
        "Wobble Room",
        "Sub Low",
        "Sidechain",
        "Pirate Radio",
        "Two-Step",
        "Wavelength",
        "Bassline",
        "Club Night",
        "Power Down",
        "Deep Cuts",
        "B2B",
        "Riddim",
        "Low Frequency",
        "Shockwave",
        "Soundsystem",
        "Dubplate",
        "Rinse Out",
        "Speaker Box",
        "Basement Session",
        "Sub Station",
        "Halfstep",
        "Pirate Signal",
        "Bass Face",
    ],
    dark: &["Smoke", "Static", "Sirens", "Thunder", "Fog", "Pressure", "Shadows", "Rumble", "Gloom", "Grit"],
    bright: &["Sunshine", "Colours", "Lights", "Summer", "Candy", "Sparks", "Glow", "Gold"],
    said: &["Heavy", "Deep", "Low", "Wild", "Sweet", "Late", "Loud", "Raw", "Rough", "Dirty"],
    lines: &[
        "Bring It Back",
        "Run the Track",
        "Pull Up",
        "Drop It",
        "Wind Up",
        "Shout It",
        "No Stopping",
        "Hold Tight",
        "Turn It Over",
        "Rewind That",
        "Bring the Bass",
        "Hold Up",
    ],
    places: &[
        "the Rave",
        "Studio 3",
        "the Basement",
        "the Car Park",
        "Eastside",
        "Croydon",
        "Peckham",
        "the Warehouse",
    ],
    times: &["Sunday", "2AM", "Half Two", "Midnight", "Summer '02", "Friday Night", "4AM"],
    english: true,
};

const EDM: Words = Words {
    alone: &[
        "Mainstage",
        "T-Minus Ten",
        "Liftoff",
        "Skyline",
        "Tidal Wave",
        "Stadium Lights",
        "Detonate",
        "Afterburn",
        "Freefall",
        "Voltage",
        "Star Field",
        "Rising",
        "Full Power",
        "Momentum",
        "Summit",
        "Big Sky",
        "Festival Grounds",
        "Laser Show",
        "Confetti",
        "Tidal Pull",
        "Crowd Surf",
        "Main Event",
    ],
    dark: &["Thunder", "Shadows", "Storms", "Fire", "Smoke", "Night", "Darkness", "Storm Clouds"],
    bright: &["Lights", "Stars", "Sun", "Summer", "Sky", "Colours", "Gold", "Sunrise", "Fireworks"],
    said: &["Bigger", "Brighter", "Electric", "Endless", "Wild", "Golden", "Higher", "Burning", "Massive", "Epic"],
    lines: &[
        "Hands Up",
        "Go Time",
        "Feel Alive",
        "Never Sleep",
        "Louder",
        "Turn It Up",
        "Once Again",
        "Take Off",
        "Light the Sky",
        "Jump Higher",
        "Hands to the Sky",
        "Feel the Drop",
    ],
    places: &["the Arena", "Ocean Front", "the Summit", "Zone 9", "the Main Stage", "the Main Tent", "Festival Hill"],
    times: &["Midnight", "Zero Hour", "Summer", "Tonight", "New Year", "Sunrise Set", "Last Song"],
    english: true,
};

const HARD_DANCE: Words = Words {
    alone: &[
        "Hammer",
        "Pressure Plate",
        "Ironclad",
        "Steel Toe",
        "Pounding",
        "Afterburner",
        "Rampage",
        "Reactor",
        "Breakpoint",
        "Tremor",
        "Full Force",
        "Bulldozer",
        "Turbine",
        "Riot Gear",
        "Kick Drum",
        "Distortion",
        "Jackhammer",
        "Pressure Cooker",
        "Bass Bin",
        "Hard Reset",
    ],
    dark: &["Thunder", "Steel", "Chaos", "Fire", "Smoke", "Voltage", "Rage", "Night", "Darkness", "Noise"],
    bright: &["Lights", "Glory", "Sun", "Fire", "Stars", "Lasers", "Glow"],
    said: &["Hard", "Raw", "Brutal", "Relentless", "Loud", "Heavy", "Endless", "Savage", "Twisted", "Dark"],
    lines: &[
        "Stand Your Ground",
        "Go Harder",
        "Louder Now",
        "Break the Floor",
        "Keep Pushing",
        "No Mercy",
        "Keep Going Harder",
        "Feel the Kick",
        "Hands in the Air",
        "Never Stop Raving",
    ],
    places: &["the Warehouse", "Hall 1", "Rotterdam", "Sector 0", "the Pit", "the Bunker", "Hall 7"],
    times: &["Midnight", "5AM", "Doomsday", "Zero Hour", "Sunrise", "Last Hour"],
    english: true,
};

const POP: Words = Words {
    alone: &[
        "Glitter",
        "Disco Mirror",
        "Sugar Rush",
        "Heartbeat",
        "Starstruck",
        "Roller Rink",
        "Daydream",
        "Lemon Drop",
        "Disco Lights",
        "Kaleidoscope",
        "Spotlight",
        "Polaroid",
        "Bubblegum",
        "Sequins",
        "Cherry Cola",
    ],
    dark: &["Heartbreak", "Tears", "Midnight", "Shadows", "Goodbyes", "Rain"],
    bright: &["Sunshine", "Glitter", "Sugar", "Stars", "Cherries", "Candy", "Summer", "Diamonds"],
    said: &["Sweet", "Electric", "Shiny", "Golden", "Crazy", "Wild", "Pink", "Brand New"],
    lines: &[
        "Dance With Me",
        "Keep Dancing",
        "Call Me Tonight",
        "Come Dance",
        "Turn the Lights Up",
        "Spin Me",
        "All Yours",
        "Kiss and Tell",
    ],
    places: &["the Roller Rink", "Studio 9", "the Dancefloor", "Paris", "Mirror Hall", "Club Venus"],
    times: &["Friday Night", "Midnight", "Summer", "Tonight", "1983"],
    english: true,
};

const ROCK: Words = Words {
    alone: &[
        "Static",
        "Fault Lines",
        "Paper Walls",
        "Weathervane",
        "Undertow",
        "Fire Escape",
        "Rust Belt",
        "Shoreline",
        "Power Cut",
        "Echo Chamber",
        "Cul-de-sac",
        "Driveway",
        "Tailspin",
        "Overpass",
        "Jet Lag",
        "Second Hand",
        "Back Row",
        "Lighter Fluid",
        "Bad Reception",
    ],
    dark: &["Ashes", "Rust", "Ghosts", "Storms", "Wires", "Smoke", "Bruises", "Static", "Shadows", "Ruins"],
    bright: &["Sunlight", "Summer", "Sparklers", "Daylight", "Comets", "Satellites", "Gold", "Oceans"],
    said: &["Broken", "Quiet", "Electric", "Hollow", "Restless", "Faded", "Loud", "Wild", "Paper", "Borrowed"],
    lines: &[
        "Sleep It Off",
        "Call It Off",
        "Drive Slower",
        "Keep Me Up",
        "Don't Wait Up",
        "Waste My Time",
        "Nothing Left to Say",
        "Not Tonight",
        "Tear It Down",
        "Over It",
    ],
    places: &[
        "the Basement",
        "Exit 27",
        "Route 1",
        "the Back Porch",
        "Motel 9",
        "the Gas Station",
        "the Parking Lot",
        "the Overpass",
    ],
    times: &["Last Summer", "Tuesday Night", "'97", "Autumn", "Senior Year", "2:46", "5:13"],
    english: true,
};

const ELECTRONIC: Words = Words {
    alone: &[
        "Circuitry",
        "Fractals",
        "Clockwork",
        "Phase Shift",
        "Signal Loss",
        "Lattice",
        "Spiral",
        "Interference",
        "Rocket Fuel",
        "Static Field",
        "Wire Frame",
        "Overclock",
        "Echo Location",
        "Feedback Loop",
        "Small Hours",
    ],
    dark: &["Static", "Smoke", "Noise", "Shadows", "Rust", "Night", "Void", "Glitches"],
    bright: &["Light", "Colour", "Prisms", "Sparks", "Sun", "Bloom"],
    said: &["Broken", "Strange", "Electric", "Tiny", "Folded", "Inverted", "Analogue", "Distant"],
    lines: &[
        "Turn It Inside Out",
        "Start Again",
        "Take It Apart",
        "Rewind",
        "Keep Counting",
        "Press Play",
        "Wake Up Now",
    ],
    places: &["Lab 4", "Room 12", "the Workshop", "Grid 9", "Sector 3"],
    times: &["2:46", "Tuesday", "Late Hours", "Midnight", "Hour Zero"],
    english: true,
};

const AMBIENT: Words = Words {
    alone: &[
        "Tidal Flats",
        "Low Clouds",
        "Lichen",
        "Driftwood",
        "Fjord",
        "Salt Marsh",
        "Slow Light",
        "Snowfield",
        "Overcast",
        "Moss",
        "Still Water",
        "Long Shadows",
        "Estuary",
        "Glacier",
        "Pale Blue",
        "Mist",
        "Hillside",
        "Distant Bells",
        "Breathing",
        "Reverie",
    ],
    dark: &["Fog", "Dusk", "Winter", "Shadows", "Silence", "Night", "Ice", "Rain", "Embers", "Depths"],
    bright: &["Light", "Dawn", "Meadows", "Sky", "Sunlight", "Bloom", "Spring", "Clearings", "Petals", "Horizon"],
    said: &["Slow", "Distant", "Quiet", "Pale", "Endless", "Soft", "Hollow", "Drifting", "Still", "Faint"],
    lines: &[
        "Let It Settle",
        "Breathe Out",
        "Stay a While",
        "Drift Off",
        "Wait Here",
        "Let Go Slowly",
        "Rest Now",
        "Hold Still",
    ],
    places: &[
        "the Fjord",
        "Cape North",
        "the Salt Flats",
        "Lake Shore",
        "Island Station",
        "the Lighthouse",
        "High Moor",
        "Glass Bay",
    ],
    times: &["First Light", "Dusk", "Late Autumn", "Night Tide", "Winter Solstice", "5AM", "Evening Hours", "Low Tide"],
    english: true,
};

const SYNTHWAVE: Words = Words {
    alone: &[
        "Neon Nights",
        "Night Drive",
        "Outrun",
        "Chrome",
        "Laser Grid",
        "Palm Drive",
        "Cassette Future",
        "Turbo",
        "Sunset Glow",
        "Magnetic",
        "Coastline",
        "Highway Dreams",
        "Video Heart",
        "Starlight Motel",
    ],
    dark: &["Shadows", "Midnight", "Storms", "Chrome", "Static", "Nights"],
    bright: &["Sunsets", "Neon", "Palms", "Lights", "Stars", "Horizons"],
    said: &["Electric", "Neon", "Midnight", "Endless", "Magnetic", "Lost", "Distant", "Cosmic"],
    lines: &["Drive On", "Take the Wheel", "Run Away Tonight", "Chase the Light", "Ride Into Night", "Stay Out Late"],
    places: &["Ocean Highway", "Palm Avenue", "Route 80", "Coast Road", "Downtown 1985", "Skyline Drive"],
    times: &["1986", "Midnight", "Summer '84", "After Dark", "Sunset", "11:59"],
    english: true,
};

const TRIP_HOP: Words = Words {
    alone: &[
        "Smoke Rings",
        "Red Curtain",
        "Grey Water",
        "Slowed Down",
        "Undertow",
        "Bristol Rain",
        "Backstreet",
        "Hazy Days",
        "Half Light",
        "Wallpaper",
        "Ashtray",
        "Static Bloom",
        "Silk Thread",
        "Low Tide",
        "Sleepwalker",
        "Cold Tea",
        "Dim Lights",
        "Overcast",
    ],
    dark: &["Smoke", "Rain", "Shadows", "Fog", "Dust", "Ghosts", "Silence", "Night"],
    bright: &["Velvet", "Honey", "Amber", "Candlelight", "Silk", "Daydreams"],
    said: &["Slow", "Hazy", "Heavy", "Dusty", "Faded", "Lonely", "Quiet", "Grey"],
    lines: &[
        "Hold Your Breath",
        "Come Down Slow",
        "Fall Apart",
        "Stay Low",
        "Let It Burn",
        "Breathe In",
        "Don't Wake Up",
        "Sink",
    ],
    places: &["Bristol", "the Back Room", "Room 11", "Harbourside", "the Station", "Platform 4"],
    times: &["4AM", "Sunday Afternoon", "Late November", "Past Midnight", "Grey Morning"],
    english: true,
};

const REGGAE: Words = Words {
    alone: &[
        "Roots Rock",
        "Sound System",
        "Dub Plate",
        "Rockers",
        "Sweet Water",
        "Hillside",
        "Bamboo",
        "Riddim Time",
        "Island Time",
        "Steppers",
        "Dub Wise",
        "Sunday Market",
        "Riverbank",
        "Higher Meditation",
        "Yard Life",
        "Rocksteady",
        "Dub Station",
        "Roots Revival",
        "Yard Style",
        "Riddim Wise",
    ],
    dark: &["Babylon", "Rain", "Fire", "Struggle", "Smoke", "Storms", "Thunder", "Dust"],
    bright: &["Sunshine", "Love", "Freedom", "Unity", "Harvest", "Blessings", "Mornings", "Sunsets", "Laughter"],
    said: &["Natural", "Sweet", "Irie", "Mellow", "Easy", "Righteous", "Humble", "Peaceful", "Cool", "Gentle"],
    lines: &[
        "Stand Firm",
        "Hold Firm",
        "Rise and Shine",
        "Easy Now",
        "Lift Your Head",
        "Never Give Up",
        "Walk Good",
        "Live Up",
        "Free Up",
        "Give Thanks",
        "Hold On Strong",
    ],
    places: &[
        "the Hills",
        "Spanish Town",
        "Port Antonio",
        "the Yard",
        "Riverside",
        "St. Ann",
        "the Market",
        "Hill Top",
    ],
    times: &["Sunday", "Harvest Time", "Early Morning", "Sundown", "Rainy Season", "Sunset", "Night Time"],
    english: true,
};

const AFROBEATS: Words = Words {
    alone: &[
        "Lagos Nights",
        "Owambe",
        "Wahala Free",
        "Danfo Ride",
        "Jollof",
        "Lekki Sunset",
        "Sweet Like Honey",
        "Palm Wine",
        "Gidi Vibes",
        "Faji",
        "Accra Calling",
        "Waistline",
        "Beach Party",
        "Sunday Jollof",
        "Blessings",
        "Afro Love",
        "Island Groove",
        "Sweet Rhythm",
        "Lagos Island",
        "Gbedu",
    ],
    dark: &["Rain", "Night", "Shadows", "Smoke", "Tears", "Storms", "Dust"],
    bright: &["Sunshine", "Honey", "Sugar", "Gold", "Blessings", "Summer", "Joy", "Laughter", "Lights"],
    said: &["Sweet", "Golden", "Easy", "Slow", "Gentle", "Sunny", "Lovely", "Wavy", "Fine", "Soft"],
    lines: &[
        "Whine It Slow",
        "Dance for Me",
        "Shine Your Eye",
        "Gimme Love",
        "Hold My Waist",
        "Carry Go",
        "Steady",
        "Come Dey",
        "Move Your Waist",
        "Love Me Well",
        "Dance Am",
    ],
    places: &["Lagos", "Accra", "Lekki", "Ikoyi", "Victoria Island", "Osu", "the Beach", "Ajah", "Surulere", "Yaba"],
    times: &[
        "Friday Night",
        "Detty December",
        "Sunday",
        "Midnight",
        "Harmattan",
        "Sunset",
        "Owambe Saturday",
        "December",
    ],
    english: true,
};

const AMAPIANO: Words = Words {
    alone: &[
        "Log Drum",
        "Soweto Nights",
        "Kasi Vibes",
        "Shisa Nyama",
        "Joburg Sunset",
        "Piano Hands",
        "Taxi Rank",
        "Township Rain",
        "Yanos",
        "Mzansi",
        "Slow Wine",
        "Long Weekend",
        "Sunday Lunch",
        "Pretoria Heat",
        "Log Drum Theory",
        "Amapiano Sunday",
        "Kasi Lights",
        "Deep Piano",
        "Shebeen Nights",
        "Jozi Lights",
    ],
    dark: &["Rain", "Night", "Smoke", "Shadows", "Winter", "Storms", "Dust"],
    bright: &["Sunshine", "Gold", "Summer", "Joy", "Stars", "Lights", "Sunsets"],
    said: &["Deep", "Slow", "Soulful", "Mellow", "Smooth", "Late", "Warm", "Easy", "Gentle", "Golden"],
    lines: &[
        "Take It Slow",
        "Shake It Down",
        "Hamba Nami",
        "Keep It Deep",
        "Let It Groove",
        "Dance Slow",
        "One More Song",
        "Feel the Log",
        "Move Slow",
        "Keep It Mellow",
    ],
    places: &["Soweto", "Pretoria", "Joburg", "Durban", "Tembisa", "the Rooftop", "Kasi", "Umlazi", "Mamelodi"],
    times: &["Sunday", "Friday", "Month End", "Summer", "Midnight", "Long Weekend", "Payday Friday", "Sunday Night"],
    english: true,
};

const REGGAETON: Words = Words {
    alone: &[
        "Perreo Lento",
        "Noche Larga",
        "Fuego",
        "Calor",
        "Mi Barrio",
        "Luna Llena",
        "Sin Prisa",
        "Callejero",
        "Tu Mirada",
        "Cintura",
        "Ritmo Caliente",
        "Dembow",
        "Bajo la Luna",
        "Sudor",
        "Bellaqueo",
        "Perreo Intenso",
        "Noche de Calor",
        "Calle Oscura",
        "Fuego Lento",
        "Bajo Cero",
        "Cuerpo a Cuerpo",
        "Tu Perfume",
        "Mil Noches",
        "La Esquina",
        "Vicio",
        "Desvelo",
        "Playa y Sol",
        "Medianoche en la Playa",
    ],
    dark: &[],
    bright: &[],
    said: &[],
    lines: &[
        "Dale Suave",
        "Toda la Noche",
        "Vente Conmigo",
        "No Pares",
        "Baila Lento",
        "Quédate Aquí",
        "Dime Que Sí",
        "Sigue Así",
        "Mírame",
        "Una Vez Más",
        "Sube el Volumen",
        "Vente Pa' Acá",
        "No Te Vayas",
        "Muévelo Así",
        "Quiero Verte",
        "Déjame Ser",
    ],
    places: &[
        "San Juan",
        "Medellín",
        "la Playa",
        "el Caserío",
        "Calle 9",
        "Santurce",
        "la Disco",
        "el Barrio",
        "Bogotá",
        "Panamá",
        "la Azotea",
        "Río Piedras",
    ],
    times: &[
        "Medianoche",
        "Viernes",
        "Verano",
        "Madrugada",
        "Esta Noche",
        "Sábado",
        "Domingo",
        "Diciembre",
        "Al Amanecer",
    ],
    english: false,
};

const BAILE_FUNK: Words = Words {
    alone: &[
        "Pancadão",
        "Tamborzão",
        "Noite no Morro",
        "Batidão",
        "Quebradeira",
        "Rio Quente",
        "Baile da Praia",
        "Paredão",
        "Calor do Rio",
        "Sextou",
        "Mandelão",
        "Fluxo",
        "Baile de Domingo",
        "Morro Acima",
        "Batuque",
        "Swing do Rio",
        "Bonde",
        "Tô na Pista",
        "Rolê",
        "Pista Quente",
        "Fim de Semana",
        "Laje",
        "Calçadão",
        "Grave Pesado",
    ],
    dark: &[],
    bright: &[],
    said: &[],
    lines: &[
        "Vai Descendo",
        "Joga Pra Cima",
        "Sem Parar",
        "Bota Pra Tocar",
        "Desce Devagar",
        "Vem Dançar",
        "Tá Tranquilo",
        "Mexe Assim",
        "Chama no Grave",
        "Sobe o Som",
        "Não Para",
        "Só Vem",
        "Dança Comigo",
        "Bate Forte",
    ],
    places: &[
        "Rio",
        "o Morro",
        "Madureira",
        "a Praia",
        "Niterói",
        "o Baile",
        "Lapa",
        "Penha",
        "Zona Norte",
        "Leblon",
        "São Gonçalo",
        "Duque de Caxias",
    ],
    times: &[
        "Sexta à Noite",
        "Meia-Noite",
        "Verão",
        "Domingo",
        "Madrugada",
        "Sábado",
        "Carnaval",
        "Fim de Tarde",
        "Cinco da Manhã",
    ],
    english: false,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theory::scale::Mode;

    /// every title the words can give, as written
    fn every(w: &Words) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        out.extend(w.alone.iter().chain(w.lines).chain(w.places).chain(w.times).map(|s| s.to_string()));
        if w.english {
            for about in w.dark.iter().chain(w.bright) {
                out.push(format!("The {about}"));
                out.extend(w.said.iter().filter(|s| !s.eq_ignore_ascii_case(about)).map(|s| format!("{s} {about}")));
            }
        }
        out
    }

    /// The best-known songs' titles, or a good many of them, that words like these could come to: a title of claudius
    /// is never one of them.
    const FAMOUS: &[&str] = &[
        "1979",
        "1999",
        "3AM",
        "After Hours",
        "After Midnight",
        "All Night Long",
        "Bailando",
        "Big Fun",
        "Black Hole Sun",
        "Blue Monday",
        "Born Slippy",
        "Burn It Down",
        "Can You Feel It",
        "Closing Time",
        "Cold Heart",
        "Come Closer",
        "Concrete Jungle",
        "Copacabana",
        "Daylight",
        "Don't Let Go",
        "Don't Look Back",
        "Don't Stop Me Now",
        "Euphoria",
        "Firework",
        "Glory Box",
        "Golden Hour",
        "Halcyon",
        "Higher Ground",
        "Hold the Line",
        "Inner City Life",
        "Kingston Town",
        "Landslide",
        "Last Dance",
        "Let It Go",
        "Lose Control",
        "Lose Yourself",
        "Midnight City",
        "Midnight Rain",
        "Move Your Body",
        "Neon Pulse",
        "Night Shift",
        "Nightcall",
        "Ocean Drive",
        "One Love",
        "One More Time",
        "Paper Planes",
        "Paradise",
        "Porcelain",
        "Pressure Drop",
        "Purple Rain",
        "Red Red Wine",
        "Route 66",
        "Say My Name",
        "Strings of Life",
        "Sunday Morning",
        "Supersonic",
        "Teardrop",
        "The Bells",
        "Thunderstruck",
        "Unfinished Sympathy",
        "Windowlicker",
        "Wonderwall",
        "Yesterday",
        "Smells Like Teen Spirit",
        "Bitter Sweet Symphony",
        "Smooth Operator",
        "Midnight Train",
        "Summertime",
        "Sweet Dreams",
        "Heroes",
        "Creep",
        "Hello",
        "Halo",
        "Umbrella",
        "Stay",
        "Faded",
        "Levels",
        "Animals",
        "Titanium",
        "Clarity",
        "Wake Me Up",
        "Starboy",
        "Blinding Lights",
        "Shape of You",
        "Bad Guy",
        "Hotline Bling",
        "God's Plan",
        "Mask Off",
        "Sicko Mode",
        "Rockstar",
        "Gold Digger",
        "Lemonade",
        "Champagne Poetry",
        "Superstition",
        "September",
    ];

    fn sets() -> Vec<(&'static str, &'static Words)> {
        vec![
            ("hip hop", &HIP_HOP),
            ("lofi", &LOFI),
            ("phonk", &PHONK),
            ("drill", &DRILL),
            ("cloud", &CLOUD),
            ("rage", &RAGE),
            ("soul", &SOUL),
            ("house", &HOUSE),
            ("techno", &TECHNO),
            ("trance", &TRANCE),
            ("dnb", &DNB),
            ("bass", &BASS),
            ("edm", &EDM),
            ("hard dance", &HARD_DANCE),
            ("pop", &POP),
            ("rock", &ROCK),
            ("electronic", &ELECTRONIC),
            ("ambient", &AMBIENT),
            ("synthwave", &SYNTHWAVE),
            ("trip hop", &TRIP_HOP),
            ("reggae", &REGGAE),
            ("afrobeats", &AFROBEATS),
            ("amapiano", &AMAPIANO),
            ("reggaeton", &REGGAETON),
            ("baile funk", &BAILE_FUNK),
        ]
    }

    #[test]
    fn no_title_is_one_of_the_best_known_songs_titles() {
        let famous: Vec<String> = FAMOUS.iter().map(|t| t.to_lowercase()).collect();
        for (name, w) in sets() {
            for t in every(w) {
                assert!(!famous.contains(&t.to_lowercase()), "{name}: {t} is a famous song's title");
            }
        }
    }

    #[test]
    fn every_genre_titles_its_songs_with_its_own_family_s_words_or_its_own() {
        for g in crate::genres::all().expect("the genres") {
            assert!(
                own_words(&g.name).is_some() || family_words(&g.family).is_some(),
                "{} ({}): no words of its own or its family's",
                g.name,
                g.family
            );
            let mut rng = Rng::new(7);
            let titles: Vec<String> = (0..60).map(|_| title(&g, Key::new(0, Mode::Minor), &mut rng)).collect();
            assert!(titles.iter().all(|t| !t.trim().is_empty() && t.trim() == t), "{}: {titles:?}", g.name);
            let twice =
                |t: &String| t.split(' ').collect::<Vec<_>>().windows(2).any(|p| p[0].eq_ignore_ascii_case(p[1]));
            assert!(!titles.iter().any(twice), "{}: a word said twice in a row: {titles:?}", g.name);
            let mut distinct = titles.clone();
            distinct.sort();
            distinct.dedup();
            // a language whose adjectives follow the word names its songs with its words alone: fewer of them
            let most = if words(&g).english { 40 } else { 35 };
            assert!(distinct.len() >= most, "{}: {} of 60 titles differ: {titles:?}", g.name, distinct.len());
        }
    }

    #[test]
    fn a_genre_written_in_lowercase_or_capitals_is_written_so_and_the_rest_begin_with_a_capital() {
        let genres = crate::genres::all().expect("the genres");
        let of = |name: &str| genres.iter().find(|g| g.name == name).expect(name).clone();
        let titles = |name: &str| {
            let (g, mut rng) = (of(name), Rng::new(3));
            (0..80).map(|_| title(&g, Key::new(2, Mode::Major), &mut rng)).collect::<Vec<_>>()
        };
        assert!(titles("lofi").iter().all(|t| *t == t.to_lowercase()), "{:?}", titles("lofi"));
        assert!(titles("cloud_rap").iter().all(|t| *t == t.to_lowercase()));
        let phonk = titles("phonk");
        let caps = phonk.iter().filter(|t| **t == t.to_uppercase()).count();
        assert!((20..=60).contains(&caps), "phonk in capitals half the time: {caps} of 80");
        for name in ["house", "techno", "rnb", "alt_rock", "ambient", "reggaeton"] {
            assert!(
                titles(name)
                    .iter()
                    .all(|t| t.starts_with(|c: char| c.is_uppercase() || c.is_ascii_digit() || c == '\'')),
                "{name}"
            );
        }
    }

    #[test]
    fn a_song_in_a_minor_key_leans_to_the_darker_words_and_one_in_a_major_key_to_the_brighter() {
        let g = crate::genres::get("ambient").expect("ambient");
        // the names made of what is said of a word, or "The" and a word: those the song's mood chooses the word of
        let of_mood = |t: &str| t.starts_with("The ") || AMBIENT.said.iter().any(|s| t.starts_with(&format!("{s} ")));
        let count = |mode: Mode, pool: &[&str]| {
            let mut rng = Rng::new(11);
            (0..3000)
                .map(|_| title(&g, Key::new(9, mode), &mut rng))
                .filter(|t| of_mood(t) && pool.iter().any(|w| t.ends_with(&format!(" {w}"))))
                .count()
        };
        let (minor_dark, minor_bright) = (count(Mode::Minor, AMBIENT.dark), count(Mode::Minor, AMBIENT.bright));
        let (major_dark, major_bright) = (count(Mode::Major, AMBIENT.dark), count(Mode::Major, AMBIENT.bright));
        assert!(minor_dark > 2 * minor_bright, "minor: {minor_dark} dark, {minor_bright} bright");
        assert!(major_bright > 2 * major_dark, "major: {major_bright} bright, {major_dark} dark");
    }

    #[test]
    fn a_language_whose_adjectives_follow_the_word_is_titled_with_its_own_words_lines_places_and_times() {
        let g = crate::genres::get("reggaeton").expect("reggaeton");
        let mut rng = Rng::new(5);
        let all = every(&REGGAETON);
        for _ in 0..100 {
            let t = title(&g, Key::new(4, Mode::Minor), &mut rng);
            assert!(all.iter().any(|w| capitalized(w) == t), "{t}");
            assert!(!t.starts_with("The "), "{t}");
        }
    }
}

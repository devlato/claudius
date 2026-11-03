//! The mock's songs: a plausible arrangement per genre (form, drum patterns, parts, mix), its script, and a critic
//! that reacts to edits (energy arc, mix balance, harmony) the way claudius' verdict reads.

use claudius_api::{
    DrumView, EarsView, GenreInfo, PackInfo, PartView, SectionView, SongView, SoundList, TrackView, VerdictView,
};

use super::Rng;
use super::sample::{Cut, Fx, Role, Spec, Want};
use crate::music::{self, Key};

#[derive(Clone, Copy, PartialEq)]
pub enum Feel {
    FourFloor,
    HalfTime,
    Breakbeat,
    BoomBap,
    Straight,
    TwoStep,
    Afro,
    Dembow,
}

type PartSpec = (&'static str, &'static str, &'static [&'static str], &'static [&'static str], (i32, i32));

pub struct Genre {
    pub name: &'static str,
    pub description: &'static str,
    pub bpm: (f64, f64),
    pub swing: (f32, f32),
    pub feel: Feel,
    pub drums: &'static [&'static str],
    pub parts: &'static [PartSpec],
    pub form: &'static [(&'static str, &'static str, u32)],
    pub modes: &'static [&'static str],
    pub query: &'static str,
    pub alts: &'static [&'static str],
}

const BASS_HOUSE: &[&str] = &["pluck_bass", "organ_bass", "sub_bass", "saw_bass"];
const CHORD_PRESETS: &[&str] = &["organ", "stab", "epiano", "keys", "pluck", "strings"];
const PAD_PRESETS: &[&str] = &["strings", "warm_pad", "pad"];
const LEAD_PRESETS: &[&str] = &["supersaw", "pluck", "bell", "square_lead", "chip"];
const ARP_PRESETS: &[&str] = &["pluck", "bell", "chip"];

pub const GENRES: &[Genre] = &[
    Genre {
        name: "house",
        description: "House: four-on-the-floor kick, offbeat hats, claps on 2 and 4, soulful 7th chords, a bouncing offbeat or syncopated bass, breakdown and build into the drop",
        bpm: (120.0, 126.0),
        swing: (0.5, 0.56),
        feel: Feel::FourFloor,
        drums: &["kick", "clap", "hat", "ohat", "shaker", "perc"],
        parts: &[
            ("bass", "bass", &["offbeat", "syncopated"], BASS_HOUSE, (33, 52)),
            ("chords", "chords", &["stabs", "offbeat", "syncopated", "charleston"], CHORD_PRESETS, (55, 76)),
            ("pad", "pad", &["sustain"], PAD_PRESETS, (50, 76)),
            ("lead", "lead", &["motif"], LEAD_PRESETS, (64, 86)),
            ("arp", "arp", &["up", "updown", "pinky"], ARP_PRESETS, (62, 86)),
        ],
        form: &[
            ("intro", "intro", 16),
            ("groove", "groove", 16),
            ("break", "break", 16),
            ("build", "build", 8),
            ("drop", "drop", 32),
            ("break2", "break", 8),
            ("drop2", "drop", 16),
            ("outro", "outro", 16),
        ],
        modes: &["minor", "minor", "dorian", "major"],
        query: "house music",
        alts: &["techno music", "minimal electronic music", "upbeat corporate pop music"],
    },
    Genre {
        name: "deep_house",
        description: "Deep house: softer four-on-the-floor, shuffled hats and shakers, minor 9th chords on electric piano and organ, a round syncopated bass, long warm reverbs",
        bpm: (118.0, 124.0),
        swing: (0.53, 0.58),
        feel: Feel::FourFloor,
        drums: &["kick", "clap", "hat", "shaker", "perc"],
        parts: &[
            ("bass", "bass", &["syncopated", "offbeat"], BASS_HOUSE, (33, 50)),
            (
                "chords",
                "chords",
                &["charleston", "syncopated", "stabs", "half"],
                &["epiano", "organ", "keys"],
                (52, 74),
            ),
            ("pad", "pad", &["sustain"], PAD_PRESETS, (50, 76)),
            ("lead", "lead", &["motif"], &["pluck", "bell", "epiano"], (62, 84)),
        ],
        form: &[
            ("intro", "intro", 16),
            ("groove", "groove", 16),
            ("break", "break", 16),
            ("build", "build", 8),
            ("drop", "drop", 32),
            ("outro", "outro", 16),
        ],
        modes: &["minor", "dorian"],
        query: "deep house music",
        alts: &["house music", "lounge music", "jazz"],
    },
    Genre {
        name: "techno",
        description: "Techno: a driving distorted kick, offbeat hats and rides, rolling bass locked between the kicks, one- or two-chord stabs in minor or phrygian, long builds of layers, optional acid line",
        bpm: (126.0, 134.0),
        swing: (0.5, 0.52),
        feel: Feel::FourFloor,
        drums: &["kick", "hat", "ohat", "clap", "ride", "perc"],
        parts: &[
            ("bass", "bass", &["rolling", "offbeat", "pulse"], &["reese", "saw_bass", "sub_bass"], (28, 45)),
            ("acid", "bass", &["acid"], &["acid"], (36, 55)),
            ("chords", "chords", &["stabs", "offbeat"], &["stab", "organ"], (50, 70)),
            ("pad", "pad", &["sustain"], PAD_PRESETS, (48, 72)),
            ("lead", "lead", &["motif"], &["square_lead", "bell", "acid"], (60, 82)),
        ],
        form: &[
            ("intro", "intro", 32),
            ("groove", "groove", 32),
            ("break", "break", 16),
            ("build", "build", 8),
            ("peak", "drop", 32),
            ("outro", "outro", 16),
        ],
        modes: &["minor", "phrygian"],
        query: "techno music",
        alts: &["house music", "industrial music", "minimal electronic music"],
    },
    Genre {
        name: "trap",
        description: "Trap: half-time feel with the snare on beat 3, syncopated kicks, 808s gliding on the kick hits, rolling hi-hats, dark minor or harmonic-minor keys and bells, hook / verse form",
        bpm: (130.0, 150.0),
        swing: (0.5, 0.54),
        feel: Feel::HalfTime,
        drums: &["kick", "snare", "clap", "hat", "ohat", "perc"],
        parts: &[
            ("bass", "bass", &["kick_follow"], &["808", "sub_bass"], (28, 45)),
            ("keys", "keys", &["sustain", "half"], &["bell", "keys", "epiano"], (55, 79)),
            ("pad", "pad", &["sustain"], PAD_PRESETS, (48, 72)),
            ("lead", "lead", &["motif"], &["bell", "pluck", "square_lead"], (64, 88)),
        ],
        form: &[
            ("intro", "intro", 8),
            ("hook", "hook", 16),
            ("verse", "verse", 16),
            ("hook2", "hook", 16),
            ("verse2", "verse", 8),
            ("hook3", "hook", 16),
            ("outro", "outro", 8),
        ],
        modes: &["minor", "harmonic_minor"],
        query: "trap music",
        alts: &["hip hop music", "dubstep", "drill music"],
    },
    Genre {
        name: "lofi",
        description: "Lo-fi hip hop: a lazy swung boom-bap beat, jazzy 7th and 9th chords on electric piano, a walking or sustained bass, tape wobble, vinyl crackle and a dusty lowpassed drum bus",
        bpm: (70.0, 90.0),
        swing: (0.56, 0.63),
        feel: Feel::BoomBap,
        drums: &["kick", "snare", "hat", "shaker"],
        parts: &[
            (
                "bass",
                "bass",
                &["walking", "sustain", "syncopated"],
                &["sub_bass", "organ_bass", "pluck_bass"],
                (31, 50),
            ),
            ("keys", "keys", &["sustain", "charleston", "half"], &["epiano", "keys"], (52, 76)),
            ("lead", "lead", &["motif"], &["bell", "epiano", "pluck"], (62, 84)),
            ("pad", "pad", &["sustain"], PAD_PRESETS, (50, 74)),
        ],
        form: &[
            ("intro", "intro", 4),
            ("a", "verse", 16),
            ("b", "hook", 16),
            ("a2", "verse", 16),
            ("outro", "outro", 8),
        ],
        modes: &["dorian", "minor", "major"],
        query: "lo-fi hip hop music",
        alts: &["jazz", "hip hop music", "ambient music"],
    },
    Genre {
        name: "dnb",
        description: "Drum and bass: 174 BPM two-step breakbeat (kick on 1 and the and-of-3, snares on 2 and 4 with ghost notes), a growling reese bass, atmospheric pads, long intro and breakdown",
        bpm: (170.0, 176.0),
        swing: (0.5, 0.51),
        feel: Feel::Breakbeat,
        drums: &["kick", "snare", "hat", "ride", "shaker"],
        parts: &[
            ("bass", "bass", &["reese", "sustain", "rolling"], &["reese", "sub_bass", "wobble"], (26, 45)),
            ("pad", "pad", &["sustain"], PAD_PRESETS, (48, 76)),
            ("lead", "lead", &["motif"], LEAD_PRESETS, (62, 86)),
            ("arp", "arp", &["up", "updown"], ARP_PRESETS, (60, 86)),
        ],
        form: &[
            ("intro", "intro", 16),
            ("build", "build", 16),
            ("drop", "drop", 32),
            ("break", "break", 16),
            ("drop2", "drop", 32),
            ("outro", "outro", 16),
        ],
        modes: &["minor", "dorian"],
        query: "drum and bass music",
        alts: &["jungle music", "dubstep", "breakbeat"],
    },
    Genre {
        name: "future_bass",
        description: "Future bass: half-time drums, lush 7th / 9th supersaw chords chopped in syncopated rhythms and pumped hard by the sidechain, a bright plucky hook, builds into drops",
        bpm: (140.0, 160.0),
        swing: (0.5, 0.53),
        feel: Feel::HalfTime,
        drums: &["kick", "snare", "hat", "ohat", "perc"],
        parts: &[
            ("chords", "chords", &["syncopated", "pump", "sustain"], &["supersaw", "pad", "pluck"], (55, 79)),
            ("bass", "bass", &["sustain", "kick_follow"], &["sub_bass", "808", "wobble"], (28, 47)),
            ("lead", "lead", &["motif"], &["pluck", "supersaw", "chip"], (64, 88)),
            ("pad", "pad", &["sustain"], PAD_PRESETS, (50, 76)),
            ("arp", "arp", &["up", "updown", "random"], ARP_PRESETS, (62, 88)),
        ],
        form: &[
            ("intro", "intro", 8),
            ("verse", "verse", 16),
            ("build", "build", 8),
            ("drop", "drop", 16),
            ("break", "break", 8),
            ("build2", "build", 8),
            ("drop2", "drop", 16),
            ("outro", "outro", 8),
        ],
        modes: &["major", "minor"],
        query: "future bass music",
        alts: &["electronic dance music", "pop music", "dubstep"],
    },
    Genre {
        name: "synthwave",
        description: "Synthwave: 80s-style straight drums with a big reverberant snare on 2 and 4, a driving eighth-note saw bass, arpeggios, lush pads and a chorused square or supersaw lead over minor progressions",
        bpm: (85.0, 118.0),
        swing: (0.5, 0.5),
        feel: Feel::Straight,
        drums: &["kick", "snare", "hat"],
        parts: &[
            ("bass", "bass", &["root8", "octave"], &["saw_bass", "sub_bass"], (31, 50)),
            ("arp", "arp", &["up", "updown"], ARP_PRESETS, (60, 84)),
            ("pad", "pad", &["sustain"], PAD_PRESETS, (48, 74)),
            ("lead", "lead", &["motif"], &["square_lead", "supersaw"], (62, 86)),
            ("chords", "chords", &["half", "sustain"], &["supersaw", "strings", "pad"], (52, 76)),
        ],
        form: &[
            ("intro", "intro", 8),
            ("verse", "verse", 16),
            ("chorus", "hook", 16),
            ("verse2", "verse", 16),
            ("chorus2", "hook", 16),
            ("bridge", "break", 8),
            ("chorus3", "hook", 16),
            ("outro", "outro", 8),
        ],
        modes: &["minor", "minor", "dorian"],
        query: "synthwave music",
        alts: &["new wave music", "electronic music", "video game music"],
    },
    Genre {
        name: "trip_hop",
        description: "Trip-hop, built on samples: a slowed drum break, a dark phrase from a recording that the song takes its key and chords from, vinyl crackle, a deep sub bass, strings that swell in the hooks, a high lonely melody, dub delays and long reverbs",
        bpm: (78.0, 94.0),
        swing: (0.51, 0.56),
        feel: Feel::BoomBap,
        drums: &["kick", "snare", "hat", "ohat", "perc"],
        parts: &[
            ("bass", "bass", &["sustain", "pulse", "syncopated"], &["sub_bass", "organ_bass"], (28, 46)),
            ("keys", "keys", &["sustain", "half"], &["epiano", "keys"], (50, 74)),
            ("strings", "pad", &["sustain"], &["strings", "warm_pad"], (48, 79)),
            ("lead", "lead", &["motif"], &["bell", "pluck"], (62, 79)),
        ],
        form: &[
            ("intro", "intro", 8),
            ("verse", "verse", 16),
            ("hook", "hook", 16),
            ("break", "break", 8),
            ("verse2", "verse", 16),
            ("hook2", "hook", 16),
            ("outro", "outro", 8),
        ],
        modes: &["minor", "minor", "phrygian", "dorian"],
        query: "trip hop music",
        alts: &["lo-fi hip hop beat", "hip hop beat with heavy bass", "ambient synth pad music"],
    },
    Genre {
        name: "big_beat",
        description: "Big beat: loud, crunched breakbeats at a fast clip, a squelching acid or fuzzed bass riff, a dirty synth hook, stabs and sirens, filter sweeps that open into huge drops, punk energy in an electronic production",
        bpm: (124.0, 138.0),
        swing: (0.5, 0.54),
        feel: Feel::Breakbeat,
        drums: &["kick", "snare", "hat", "ride", "perc"],
        parts: &[
            ("bass", "bass", &["root8", "syncopated", "octave"], &["saw_bass", "reese"], (31, 48)),
            ("acid", "bass", &["acid"], &["acid"], (36, 55)),
            ("stabs", "chords", &["stabs", "offbeat"], &["stab", "organ"], (52, 72)),
            ("riff", "lead", &["motif"], &["square_lead", "supersaw", "reese"], (55, 76)),
        ],
        form: &[
            ("intro", "intro", 8),
            ("groove", "groove", 16),
            ("build", "build", 8),
            ("drop", "drop", 16),
            ("break", "break", 8),
            ("build2", "build", 8),
            ("drop2", "drop", 16),
            ("outro", "outro", 8),
        ],
        modes: &["minor", "mixolydian", "phrygian", "dorian"],
        query: "big beat breakbeat music with distorted drums",
        alts: &["drum and bass music", "techno music", "rock music with electric guitars"],
    },
    Genre {
        name: "electro_house",
        description: "Electro house: a punchy four-on-the-floor kick, big claps and offbeat hats, a dirty distorted bass riff pumped hard by the sidechain, a screaming supersaw hook, long snare-roll builds that open the filters into huge drops",
        bpm: (126.0, 130.0),
        swing: (0.5, 0.52),
        feel: Feel::FourFloor,
        drums: &["kick", "clap", "hat", "ohat", "perc"],
        parts: &[
            ("bass", "bass", &["syncopated", "offbeat"], &["saw_bass", "wobble", "reese"], (38, 57)),
            ("chords", "chords", &["stabs", "pump", "offbeat"], &["stab", "supersaw"], (55, 74)),
            ("lead", "lead", &["motif"], &["supersaw", "square_lead"], (64, 84)),
            ("pad", "pad", &["sustain"], PAD_PRESETS, (50, 76)),
            ("arp", "arp", &["up", "updown"], ARP_PRESETS, (60, 84)),
        ],
        form: &[
            ("intro", "intro", 16),
            ("groove", "groove", 16),
            ("build", "build", 8),
            ("drop", "drop", 16),
            ("break", "break", 16),
            ("build2", "build", 8),
            ("drop2", "drop", 16),
            ("outro", "outro", 16),
        ],
        modes: &["minor", "minor", "dorian"],
        query: "dirty electro house dance music with big synth leads",
        alts: &["house music", "dubstep music", "techno music"],
    },
    Genre {
        name: "nu_disco",
        description: "Nu-disco: a four-on-the-floor kick with open hats on the offbeats and a clap on 2 and 4, an octave-jumping disco bass, funky muted guitar and electric piano chords, lush strings, a brassy hook, a warm and glossy mix with a touch of phaser",
        bpm: (112.0, 124.0),
        swing: (0.5, 0.56),
        feel: Feel::FourFloor,
        drums: &["kick", "clap", "hat", "ohat", "shaker", "perc"],
        parts: &[
            ("bass", "bass", &["octave", "walking", "syncopated"], &["pluck_bass", "saw_bass"], (33, 52)),
            ("guitar", "chords", &["stabs", "syncopated", "charleston"], &["pluck"], (52, 76)),
            ("keys", "keys", &["offbeat", "half", "syncopated"], &["epiano", "keys"], (52, 74)),
            ("strings", "pad", &["sustain", "half"], &["strings"], (55, 82)),
            ("lead", "lead", &["motif"], &["stab", "square_lead"], (62, 82)),
        ],
        form: &[
            ("intro", "intro", 8),
            ("verse", "verse", 16),
            ("hook", "hook", 16),
            ("break", "break", 8),
            ("verse2", "verse", 16),
            ("hook2", "hook", 16),
            ("outro", "outro", 8),
        ],
        modes: &["minor", "dorian", "major"],
        query: "disco music",
        alts: &["house music", "upbeat corporate pop music", "synthwave music"],
    },
    Genre {
        name: "filter_house",
        description: "Filter house: a looped disco phrase of chords and bass sweeping through a resonant lowpass that opens and closes with the arrangement, a hard sidechain pump, a phaser on the loop, four-on-the-floor drums and a squashed, saturated mix",
        bpm: (118.0, 126.0),
        swing: (0.5, 0.55),
        feel: Feel::FourFloor,
        drums: &["kick", "clap", "hat", "ohat", "shaker", "perc"],
        parts: &[
            ("loop", "chords", &["stabs", "offbeat", "charleston", "pump"], &["stab", "organ", "epiano"], (52, 74)),
            ("bass", "bass", &["octave", "offbeat", "syncopated"], &["organ_bass", "saw_bass", "pluck_bass"], (33, 50)),
            ("keys", "keys", &["half", "sustain", "offbeat"], &["epiano", "organ"], (55, 76)),
        ],
        form: &[
            ("intro", "intro", 16),
            ("groove", "groove", 16),
            ("break", "break", 8),
            ("drop", "drop", 32),
            ("break2", "break", 8),
            ("drop2", "drop", 16),
            ("outro", "outro", 16),
        ],
        modes: &["dorian", "minor", "mixolydian"],
        query: "french house music with a filtered disco loop",
        alts: &["house music", "funk music", "techno music"],
    },
    Genre {
        name: "dance_pop",
        description: "Dance-pop: a song first, verse, pre-chorus and a big chorus, over a four-on-the-floor or pop beat with bright claps, a pumping synth bass, plucked chords and a hook a voice could sing, bright major and minor pop progressions, a glossy, loud and wide mix",
        bpm: (116.0, 128.0),
        swing: (0.5, 0.52),
        feel: Feel::FourFloor,
        drums: &["kick", "clap", "hat", "ohat", "shaker"],
        parts: &[
            ("bass", "bass", &["offbeat", "pulse", "syncopated"], &["pluck_bass", "saw_bass", "sub_bass"], (33, 52)),
            ("chords", "chords", &["offbeat", "syncopated", "stabs"], &["pluck", "keys", "stab"], (55, 76)),
            ("pad", "pad", &["sustain"], PAD_PRESETS, (50, 76)),
            ("hook", "lead", &["motif"], &["pluck", "bell", "square_lead"], (60, 77)),
            ("lead", "lead", &["motif"], &["supersaw", "pluck", "square_lead"], (67, 88)),
        ],
        form: &[
            ("intro", "intro", 8),
            ("verse", "verse", 16),
            ("pre", "build", 8),
            ("chorus", "hook", 16),
            ("verse2", "verse", 16),
            ("pre2", "build", 8),
            ("chorus2", "hook", 16),
            ("bridge", "break", 8),
            ("chorus3", "hook", 16),
            ("outro", "outro", 8),
        ],
        modes: &["major", "minor", "major"],
        query: "upbeat dance pop music",
        alts: &["upbeat corporate pop music", "house music", "synthwave music"],
    },
    Genre {
        name: "neo_psychedelia",
        description: "Neo-psychedelia: a loose live drum kit, a warm fuzzy electric bass, guitars swirling through phasers, flangers and choruses, organ and dreamy synths, sunny mixolydian and dorian chords, long reverbs and tape wobble, a hazy wall of sound that keeps opening up",
        bpm: (96.0, 122.0),
        swing: (0.5, 0.55),
        feel: Feel::Straight,
        drums: &["kick", "snare", "hat", "ride", "tom"],
        parts: &[
            ("bass", "bass", &["root8", "sustain", "pulse"], &["saw_bass", "pluck_bass"], (31, 50)),
            ("guitar", "chords", &["half", "syncopated", "sustain"], &["pluck"], (52, 76)),
            ("organ", "keys", &["sustain", "half"], &["organ", "epiano"], (50, 74)),
            ("synth", "pad", &["sustain"], &["warm_pad", "pad"], (52, 79)),
            ("lead", "lead", &["motif"], &["bell", "pluck"], (62, 81)),
        ],
        form: &[
            ("intro", "intro", 8),
            ("verse", "verse", 16),
            ("chorus", "hook", 16),
            ("verse2", "verse", 16),
            ("chorus2", "hook", 16),
            ("bridge", "break", 16),
            ("chorus3", "hook", 16),
            ("outro", "outro", 8),
        ],
        modes: &["mixolydian", "dorian", "major", "lydian"],
        query: "dreamy psychedelic pop music with phased guitars and organ",
        alts: &["rock music with electric guitars", "indie pop music", "synthwave music"],
    },
    Genre {
        name: "art_rock",
        description: "Art rock with electronics: a song that grows from a quiet piano and a glitchy, arpeggiated synth into a loud climax, live drums, a dark electric bass, strings, guitars that turn from clean to distorted, minor and modal chords, stuttered edits, a long arc rather than a loop",
        bpm: (70.0, 100.0),
        swing: (0.5, 0.53),
        feel: Feel::Straight,
        drums: &["kick", "snare", "hat", "ride", "tom"],
        parts: &[
            ("piano", "keys", &["half", "sustain"], &["keys"], (48, 76)),
            ("arp", "arp", &["up", "random", "pinky"], &["pluck"], (60, 84)),
            ("bass", "bass", &["sustain", "root8", "syncopated"], &["saw_bass", "sub_bass"], (28, 47)),
            ("strings", "pad", &["sustain"], &["strings"], (45, 81)),
            ("guitar", "chords", &["sustain", "half", "stabs"], &["pluck"], (50, 74)),
            ("lead", "lead", &["motif"], &["strings", "square_lead"], (62, 81)),
        ],
        form: &[
            ("intro", "intro", 8),
            ("verse", "verse", 16),
            ("verse2", "verse", 16),
            ("rise", "build", 8),
            ("climax", "hook", 16),
            ("fall", "break", 8),
            ("climax2", "hook", 16),
            ("outro", "outro", 8),
        ],
        modes: &["minor", "dorian", "phrygian", "harmonic_minor"],
        query: "progressive rock music with piano, strings and synthesizers",
        alts: &["rock music with electric guitars", "cinematic orchestral music", "ambient synth pad music"],
    },
    Genre {
        name: "idm",
        description: "IDM: intricate broken beats with ratchets, ghost notes and shifting accents, glitches, stutters and gated bursts, crystalline bells and detuned pads, an acid or sub bass, odd modal harmony, every part processed into something new as the track goes on",
        bpm: (96.0, 140.0),
        swing: (0.5, 0.56),
        feel: Feel::Breakbeat,
        drums: &["kick", "snare", "hat", "perc"],
        parts: &[
            ("bass", "bass", &["syncopated", "sustain", "acid"], &["sub_bass", "acid", "808"], (28, 50)),
            ("bells", "lead", &["motif"], &["bell", "pluck"], (64, 91)),
            ("pad", "pad", &["sustain"], &["warm_pad", "pad"], (48, 79)),
            ("arp", "arp", &["random", "updown", "pinky"], ARP_PRESETS, (60, 88)),
        ],
        form: &[
            ("intro", "intro", 8),
            ("groove", "groove", 16),
            ("groove2", "groove", 16),
            ("break", "break", 8),
            ("peak", "drop", 16),
            ("outro", "outro", 8),
        ],
        modes: &["lydian", "dorian", "minor", "phrygian"],
        query: "glitchy intelligent dance music with complex broken beats and bells",
        alts: &["glitch hop music", "drum and bass music", "ambient synth pad music"],
    },
    Genre {
        name: "dub_techno",
        description: "Dub techno: a soft, steady four-on-the-floor, rimshots and hiss-light hats, one or two minor chords stabbed and drowned in long echoes that feed back and swell, a filter on them that breathes with the arrangement, a deep sub, a fog of reverb, hypnotic and slowly changing",
        bpm: (116.0, 126.0),
        swing: (0.5, 0.54),
        feel: Feel::FourFloor,
        drums: &["kick", "hat", "ohat", "perc", "shaker"],
        parts: &[
            ("bass", "bass", &["sustain", "pulse", "offbeat"], &["sub_bass", "organ_bass"], (28, 43)),
            ("chords", "chords", &["stabs", "offbeat"], &["stab", "organ", "epiano"], (50, 70)),
            ("pad", "pad", &["sustain"], &["warm_pad", "pad"], (48, 74)),
        ],
        form: &[
            ("intro", "intro", 16),
            ("groove", "groove", 32),
            ("break", "break", 16),
            ("deep", "drop", 32),
            ("outro", "outro", 16),
        ],
        modes: &["minor", "dorian", "phrygian"],
        query: "dub techno music with echoing chords",
        alts: &["techno music", "minimal electronic music", "ambient synth pad music"],
    },
    Genre {
        name: "boom_bap",
        description: "Boom bap: 90s hip hop at 86 to 96 BPM, a hard kick and a cracking snare on 2 and 4 with a lazy sampler swing, dusty hats and a rim, a piano or Rhodes loop over jazzy minor chords, an upright or sub bass, horn or flute phrases in the hooks, three verses and hooks",
        bpm: (86.0, 96.0),
        swing: (0.55, 0.62),
        feel: Feel::BoomBap,
        drums: &["kick", "snare", "hat", "ohat", "rim"],
        parts: &[
            ("bass", "bass", &["syncopated", "sustain", "walking"], &["sub_bass", "organ_bass"], (31, 48)),
            ("keys", "keys", &["charleston", "sustain", "half"], &["keys", "epiano"], (50, 74)),
            ("horns", "lead", &["motif"], &["square_lead", "bell"], (60, 82)),
            ("pad", "pad", &["sustain"], &["strings", "warm_pad"], (52, 76)),
        ],
        form: &[
            ("intro", "intro", 4),
            ("verse", "verse", 16),
            ("hook", "hook", 8),
            ("verse2", "verse", 16),
            ("hook2", "hook", 8),
            ("verse3", "verse", 16),
            ("hook3", "hook", 8),
            ("outro", "outro", 8),
        ],
        modes: &["minor", "dorian", "phrygian"],
        query: "boom bap hip hop beat",
        alts: &["lo-fi hip hop beat", "jazz music", "funk music"],
    },
    Genre {
        name: "uk_drill",
        description: "UK drill: 138 to 146 BPM felt at half time, a snare on the third beat answered late in every other bar, skippy hats in threes with rolls, sliding 808s that follow a sparse kick, dark minor and harmonic minor chords on piano, strings or choir, a bell or flute line in the hooks",
        bpm: (138.0, 146.0),
        swing: (0.5, 0.53),
        feel: Feel::HalfTime,
        drums: &["kick", "snare", "hat", "ohat", "perc"],
        parts: &[
            ("bass", "bass", &["kick_follow"], &["808"], (28, 43)),
            ("keys", "keys", &["sustain", "half"], &["keys", "strings", "bell"], (52, 76)),
            ("lead", "lead", &["motif"], &["bell", "pluck", "square_lead"], (64, 88)),
            ("pad", "pad", &["sustain"], &["strings", "pad", "warm_pad"], (48, 72)),
        ],
        form: &[
            ("intro", "intro", 8),
            ("hook", "hook", 16),
            ("verse", "verse", 16),
            ("hook2", "hook", 16),
            ("verse2", "verse", 16),
            ("hook3", "hook", 16),
            ("outro", "outro", 8),
        ],
        modes: &["minor", "harmonic_minor", "phrygian"],
        query: "uk drill beat",
        alts: &["trap beat", "hip hop beat with heavy bass", "grime music"],
    },
    Genre {
        name: "dubstep",
        description: "Dubstep: 138 to 142 BPM at half time, a big snare on the third beat over a sparse kick, a wobbling mid bass over a clean sub, dark minor and phrygian chords on pads and strings, risers into heavy drops and a stripped back break",
        bpm: (138.0, 142.0),
        swing: (0.5, 0.52),
        feel: Feel::HalfTime,
        drums: &["kick", "snare", "hat", "ohat", "rim"],
        parts: &[
            ("sub", "bass", &["sustain"], &["sub_bass"], (24, 38)),
            ("wobble", "bass", &["syncopated", "pulse", "sustain"], &["wobble", "reese", "saw_bass"], (36, 52)),
            ("pad", "pad", &["sustain"], PAD_PRESETS, (48, 74)),
            ("lead", "lead", &["motif"], &["supersaw", "bell", "square_lead"], (62, 86)),
        ],
        form: &[
            ("intro", "intro", 16),
            ("build", "build", 8),
            ("drop", "drop", 32),
            ("break", "break", 16),
            ("drop2", "drop", 32),
            ("outro", "outro", 16),
        ],
        modes: &["minor", "phrygian", "harmonic_minor"],
        query: "dubstep music",
        alts: &["drum and bass music", "trap beat", "electro house music"],
    },
    Genre {
        name: "uk_garage",
        description: "UK garage: 130 to 136 BPM with a heavy shuffle, a two-step kick that skips the third beat, snares on 2 and 4 with skippy ghosts, shuffled hats, shakers and a rim, a bouncing sub or organ bass, soulful minor 7th chords on organ, Rhodes or stabs, a vocal-like lead in the hooks",
        bpm: (130.0, 136.0),
        swing: (0.58, 0.66),
        feel: Feel::TwoStep,
        drums: &["kick", "snare", "hat", "ohat", "shaker", "rim"],
        parts: &[
            ("bass", "bass", &["syncopated", "offbeat", "pulse"], &["sub_bass", "organ_bass", "reese"], (28, 46)),
            ("chords", "chords", &["stabs", "syncopated", "offbeat"], &["organ", "epiano", "stab"], (55, 76)),
            ("pad", "pad", &["sustain"], &["warm_pad", "strings"], (50, 74)),
            ("lead", "lead", &["motif"], &["pluck", "bell", "epiano"], (64, 86)),
        ],
        form: &[
            ("intro", "intro", 8),
            ("verse", "verse", 16),
            ("hook", "hook", 16),
            ("break", "break", 8),
            ("verse2", "verse", 16),
            ("hook2", "hook", 16),
            ("outro", "outro", 16),
        ],
        modes: &["minor", "dorian", "major"],
        query: "uk garage music",
        alts: &["house music", "r&b music", "drum and bass music"],
    },
    Genre {
        name: "jungle",
        description: "Jungle: 160 to 172 BPM, a chopped and rolling breakbeat with ghost snares on an acoustic kit, a deep sub or reese bass moving at half the speed of the drums, dub chords and stabs with long delays, warm pads and a sparse lead",
        bpm: (160.0, 172.0),
        swing: (0.5, 0.53),
        feel: Feel::Breakbeat,
        drums: &["kick", "snare", "hat", "ride"],
        parts: &[
            ("bass", "bass", &["sustain", "reese", "rolling"], &["sub_bass", "reese"], (24, 40)),
            ("chords", "chords", &["offbeat", "stabs", "sustain"], &["organ", "stab", "epiano"], (55, 74)),
            ("pad", "pad", &["sustain"], PAD_PRESETS, (48, 74)),
            ("lead", "lead", &["motif"], &["pluck", "bell"], (64, 86)),
        ],
        form: &[
            ("intro", "intro", 16),
            ("build", "build", 8),
            ("drop", "drop", 32),
            ("break", "break", 16),
            ("drop2", "drop", 32),
            ("outro", "outro", 16),
        ],
        modes: &["minor", "dorian", "phrygian"],
        query: "jungle music",
        alts: &["drum and bass music", "breakbeat music", "reggae music"],
    },
    Genre {
        name: "afrobeats",
        description: "Afrobeats: 100 to 115 BPM, a rolling West African groove (a kick on one and three with a pickup, claps on 2 and 4, a rim on the clave, shakers in sixteenths, congas), bright major and minor chords on guitar plucks and keys, a warm sub, a sung-sounding lead in the hooks",
        bpm: (100.0, 115.0),
        swing: (0.52, 0.58),
        feel: Feel::Afro,
        drums: &["kick", "clap", "rim", "hat", "shaker", "perc"],
        parts: &[
            ("bass", "bass", &["syncopated", "sustain", "offbeat"], &["sub_bass", "pluck_bass"], (31, 48)),
            ("guitar", "chords", &["stabs", "syncopated", "offbeat"], &["pluck"], (55, 76)),
            ("keys", "keys", &["sustain", "half"], &["epiano", "keys"], (50, 74)),
            ("lead", "lead", &["motif"], &["bell", "pluck", "square_lead"], (62, 84)),
            ("pad", "pad", &["sustain"], &["warm_pad", "strings"], (50, 74)),
        ],
        form: &[
            ("intro", "intro", 8),
            ("verse", "verse", 16),
            ("hook", "hook", 16),
            ("verse2", "verse", 16),
            ("hook2", "hook", 16),
            ("break", "break", 8),
            ("hook3", "hook", 16),
            ("outro", "outro", 8),
        ],
        modes: &["major", "minor", "dorian"],
        query: "afrobeats music",
        alts: &["dancehall music", "reggaeton music", "house music"],
    },
    Genre {
        name: "amapiano",
        description: "Amapiano: 110 to 116 BPM, a steady kick under shakers and open hats, the log drum, a pitched and woody bass playing syncopated lines, jazzy minor 7th and 9th chords on piano and Rhodes, airy pads, a whistle or flute line, long grooves that hold back before they drop",
        bpm: (110.0, 116.0),
        swing: (0.54, 0.6),
        feel: Feel::FourFloor,
        drums: &["kick", "clap", "shaker", "ohat", "rim"],
        parts: &[
            ("logdrum", "bass", &["syncopated", "kick_follow", "offbeat"], &["pluck_bass", "808"], (33, 52)),
            ("keys", "keys", &["sustain", "charleston", "half", "stabs"], &["keys", "epiano"], (52, 76)),
            ("pad", "pad", &["sustain"], &["warm_pad", "pad"], (50, 76)),
            ("lead", "lead", &["motif"], &["bell", "pluck"], (64, 86)),
        ],
        form: &[
            ("intro", "intro", 16),
            ("groove", "groove", 16),
            ("break", "break", 8),
            ("drop", "drop", 32),
            ("break2", "break", 16),
            ("drop2", "drop", 32),
            ("outro", "outro", 16),
        ],
        modes: &["minor", "dorian", "major"],
        query: "amapiano music",
        alts: &["deep house music", "afrobeats music", "jazz music"],
    },
    Genre {
        name: "reggaeton",
        description: "Reggaeton: 88 to 100 BPM on the dembow, a kick on every beat and a snare after the first and before the third, minor chords on plucks, pianos and pads, a sub bass on the roots, a synth or voice-like lead, verses and hooks",
        bpm: (88.0, 100.0),
        swing: (0.5, 0.53),
        feel: Feel::Dembow,
        drums: &["kick", "snare", "hat", "rim", "shaker"],
        parts: &[
            ("bass", "bass", &["sustain", "kick_follow", "root8"], &["sub_bass", "808"], (28, 45)),
            ("chords", "chords", &["sustain", "stabs", "syncopated"], &["pluck", "keys", "strings"], (55, 76)),
            ("pad", "pad", &["sustain"], PAD_PRESETS, (48, 72)),
            ("lead", "lead", &["motif"], &["pluck", "square_lead", "supersaw"], (62, 86)),
        ],
        form: &[
            ("intro", "intro", 8),
            ("verse", "verse", 16),
            ("hook", "hook", 16),
            ("verse2", "verse", 16),
            ("hook2", "hook", 16),
            ("break", "break", 8),
            ("hook3", "hook", 16),
            ("outro", "outro", 8),
        ],
        modes: &["minor", "harmonic_minor", "dorian"],
        query: "reggaeton music",
        alts: &["dancehall music", "latin pop music", "trap beat"],
    },
    Genre {
        name: "trance",
        description: "Trance: 134 to 140 BPM, four on the floor with offbeat open hats and a clap on 2 and 4, a rolling sixteenth-note bass, euphoric minor progressions under supersaw chords and a soaring lead, arpeggios, a long breakdown that strips to pads and builds back with risers and a roll into the drop",
        bpm: (134.0, 140.0),
        swing: (0.5, 0.51),
        feel: Feel::FourFloor,
        drums: &["kick", "clap", "hat", "ohat"],
        parts: &[
            ("bass", "bass", &["rolling", "offbeat", "pulse"], &["saw_bass", "pluck_bass"], (33, 50)),
            ("chords", "chords", &["sustain", "pump", "stabs"], &["supersaw", "stab"], (55, 79)),
            ("pad", "pad", &["sustain"], PAD_PRESETS, (48, 76)),
            ("lead", "lead", &["motif"], &["supersaw", "pluck", "square_lead"], (64, 88)),
            ("arp", "arp", &["up", "updown", "random"], ARP_PRESETS, (60, 86)),
        ],
        form: &[
            ("intro", "intro", 16),
            ("groove", "groove", 16),
            ("break", "break", 32),
            ("build", "build", 8),
            ("drop", "drop", 32),
            ("outro", "outro", 16),
        ],
        modes: &["minor", "dorian", "major"],
        query: "trance music",
        alts: &["techno music", "house music", "electro house music"],
    },
    Genre {
        name: "neo_soul",
        description: "Neo-soul: 72 to 92 BPM, a laid-back groove on a live kit with ghost notes and a cross-stick, rich 9th and 11th chords on Rhodes and organ, a round bass that walks and slides between roots, soft strings, a sung-sounding lead, verses, hooks and a bridge",
        bpm: (72.0, 92.0),
        swing: (0.55, 0.62),
        feel: Feel::BoomBap,
        drums: &["kick", "snare", "hat", "rim", "ride"],
        parts: &[
            ("bass", "bass", &["syncopated", "walking", "sustain"], &["pluck_bass", "sub_bass"], (28, 48)),
            ("keys", "keys", &["sustain", "charleston", "half"], &["epiano", "keys", "organ"], (50, 76)),
            ("lead", "lead", &["motif"], &["bell", "square_lead", "pluck"], (62, 84)),
            ("strings", "pad", &["sustain"], &["strings", "warm_pad"], (52, 79)),
        ],
        form: &[
            ("intro", "intro", 4),
            ("verse", "verse", 16),
            ("hook", "hook", 8),
            ("verse2", "verse", 16),
            ("hook2", "hook", 8),
            ("bridge", "break", 8),
            ("hook3", "hook", 8),
            ("outro", "outro", 8),
        ],
        modes: &["dorian", "minor", "major"],
        query: "neo soul music",
        alts: &["r&b music", "jazz music", "lo-fi hip hop beat"],
    },
];

/// trip-hop's samples in the demo: loops, as trip-hop's profile describes its samples, with its colours (the
/// break lowpassed; the phrase high-passed, through tape and a little crushed)
static TRIP_HOP: Spec = Spec {
    phrase: Want {
        search: &[
            "dark piano loop",
            "sad piano loop",
            "strings loop",
            "cello loop",
            "orchestral strings loop",
            "dark melody loop",
            "orchestral melody",
            "sad violin loop",
        ],
        words: &[
            "piano",
            "strings",
            "string",
            "cello",
            "cellos",
            "violin",
            "violins",
            "viola",
            "orchestra",
            "orchestral",
            "harp",
            "harpsichord",
            "mellotron",
            "choir",
            "rhodes",
            "keys",
            "flute",
            "guitar",
        ],
        not: &["cowbell", "percussion", "drumloop"],
        fx: &[Fx::Highpass(160.0), Fx::Tape { drive_db: 3.0, lowpass: 8500.0 }, Fx::Crush { bits: 12, down: 2 }],
    },
    drum_break: Want {
        search: &["hip hop drum loop", "dusty drum loop", "slow drum break", "funk drum loop"],
        words: &["drum", "drums", "drumloop", "break", "breakbeat", "breaks", "beat", "groove"],
        not: &["industrial", "techno", "dubstep", "edm", "trance", "house"],
        fx: &[Fx::Lowpass(10_000.0)],
    },
    texture: Want {
        search: &["vinyl crackle", "record crackle", "vinyl noise"],
        words: &["vinyl", "turntable", "gramophone", "phonograph", "lp", "78rpm", "shellac"],
        not: &["foley", "sfx", "metal", "electric", "welding"],
        fx: &[Fx::Highpass(300.0)],
    },
};

/// the samples a genre is built on, if it is (a fusion: the genre that leads it)
pub fn sampling(name: &str) -> Option<&'static Spec> {
    match genre(name).name {
        "trip_hop" => Some(&TRIP_HOP),
        _ => None,
    }
}

/// a genre of the demo's; a fusion plays the genre that leads it
pub fn genre(name: &str) -> &'static Genre {
    let name = name.split('+').next().unwrap_or(name).trim();
    GENRES.iter().find(|g| g.name == name).unwrap_or(&GENRES[0])
}

/// what the demo offers to scripts
pub fn sounds() -> SoundList {
    let list = |xs: &[&str]| xs.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let pack = |name: &str, description: &str, license: &str, installed: bool, size_mb: f64| PackInfo {
        name: name.into(),
        description: description.into(),
        license: license.into(),
        installed,
        size_mb,
        ..Default::default()
    };
    SoundList {
        drums: list(&[
            "kick", "snare", "clap", "hh", "oh", "ride", "crash", "tom", "rim", "shaker", "cowbell", "snap", "perc",
        ]),
        synths: list(&[
            "sub_bass",
            "808",
            "reese",
            "acid",
            "saw_bass",
            "pluck_bass",
            "organ_bass",
            "wobble",
            "pluck",
            "supersaw",
            "pad",
            "warm_pad",
            "stab",
            "epiano",
            "keys",
            "bell",
            "organ",
            "strings",
            "square_lead",
            "chip",
        ]),
        instruments: list(&[
            "piano", "rhodes", "strings", "tr909:bd", "tr909:sd", "tr909:hh", "tr808:bd", "tr808:cp", "tr808:hh",
        ]),
        packs: vec![
            pack("tr909", "Roland TR-909 one-shots", "CC0", true, 4.2),
            pack("tr808", "Roland TR-808 one-shots", "CC0", true, 3.1),
            pack("salamander-piano", "Salamander Grand Piano, 16 velocity layers", "CC-BY 3.0", false, 380.0),
            pack("vsco2-strings", "VSCO 2 community orchestra strings", "CC0", false, 210.0),
        ],
        kit: list(&["tr909:bd", "tr909:sd", "tr909:hh", "tr808:bd", "tr808:cp", "tr808:hh"]),
        // as the synth presets have them
        synth_roles: [
            ("sub_bass", &["bass"][..]),
            ("808", &["bass"]),
            ("reese", &["bass"]),
            ("acid", &["bass", "lead"]),
            ("saw_bass", &["bass"]),
            ("pluck_bass", &["bass"]),
            ("organ_bass", &["bass"]),
            ("wobble", &["bass"]),
            ("pluck", &["arp", "chords", "lead"]),
            ("supersaw", &["lead", "chords"]),
            ("pad", &["pad", "chords"]),
            ("warm_pad", &["pad"]),
            ("stab", &["chords"]),
            ("epiano", &["keys", "chords"]),
            ("keys", &["keys", "chords"]),
            ("bell", &["lead", "arp"]),
            ("organ", &["chords", "keys"]),
            ("strings", &["pad", "chords"]),
            ("square_lead", &["lead"]),
            ("chip", &["lead", "arp"]),
        ]
        .iter()
        .map(|(n, r)| (n.to_string(), list(r)))
        .collect(),
    }
}

pub fn genre_infos() -> Vec<GenreInfo> {
    GENRES.iter().map(|g| GenreInfo { name: g.name.into(), description: g.description.into(), bpm: g.bpm }).collect()
}

const ADJECTIVES: &[&str] = &[
    "Glass", "Hollow", "Silent", "Liquid", "Neon", "Velvet", "Golden", "Midnight", "Paper", "Electric", "Distant",
    "Crystal", "Static", "Amber", "Lunar", "Copper",
];
const NOUNS: &[&str] = &[
    "Garden", "Harbor", "Circuit", "Avenue", "Echo", "Signal", "Tides", "Mirror", "Engine", "Bloom", "Horizon",
    "Motel", "Lights", "Season", "Arcade", "Orchard",
];

fn progressions(mode: &str, rng: &mut Rng) -> Vec<(String, Vec<String>)> {
    let lib: &[&[&str]] = match mode {
        "major" => &[
            &["Imaj7", "vi7", "ii7", "V7"],
            &["I", "V", "vi", "IV"],
            &["Imaj7", "iii7", "vi7", "IVmaj7"],
            &["IVmaj7", "V7", "iii7", "vi7"],
        ],
        "dorian" => {
            &[&["i7", "IV7"], &["i9", "IV9", "i7", "VIImaj7"], &["i7", "ii7", "IV7", "i7"], &["i7", "IV7", "v7", "IV7"]]
        }
        "phrygian" => &[&["i", "II"], &["i7", "IImaj7", "i7", "vii7"], &["i", "II", "VII", "i"]],
        "harmonic_minor" => &[&["i", "VI", "iv", "V"], &["i", "iv", "V7", "i"], &["i", "VI", "V", "V7"]],
        _ => &[
            &["i7", "VImaj7", "iv7", "v7"],
            &["i7", "iv7", "VImaj7", "V7"],
            &["i", "VI", "III", "VII"],
            &["i9", "VImaj7", "IIImaj7", "VII7"],
            &["i7", "VII7", "VImaj7", "v7"],
        ],
    };
    let a = rng.pick(lib);
    let mut b: Vec<String> = a.iter().map(|s| s.to_string()).collect();
    let n = b.len().max(1);
    b.rotate_left((n / 2).max(1) % n);
    vec![("A".into(), a.iter().map(|s| s.to_string()).collect()), ("B".into(), b)]
}

/// one drum lane's variants over `bars` bars of 16 steps
fn drum_lane(lane: &str, feel: Feel, rng: &mut Rng) -> DrumView {
    let bars: u32 = match lane {
        "ohat" => 1,
        "shaker" | "ride" => 2,
        _ => 4,
    };
    let n = (bars * 16) as usize;
    let mut main = vec![0.0f32; n];
    let mut sparse: Option<Vec<f32>> = None;
    let mut alt: Option<Vec<f32>> = None;
    for bar in 0..bars as usize {
        let at = |s: usize| bar * 16 + s;
        let last = bar + 1 == bars as usize;
        match (lane, feel) {
            ("kick", Feel::FourFloor | Feel::Dembow) => {
                [0, 4, 8, 12].iter().for_each(|&s| main[at(s)] = rng.range(0.93, 1.0))
            }
            ("kick", Feel::Straight) => [0, 4, 8, 12].iter().for_each(|&s| main[at(s)] = rng.range(0.9, 1.0)),
            ("kick", Feel::HalfTime) => {
                main[at(0)] = 1.0;
                main[at(10)] = rng.range(0.85, 0.95);
                if bar % 2 == 1 {
                    main[at(7)] = rng.range(0.7, 0.85);
                }
                if last {
                    main[at(14)] = rng.range(0.7, 0.8);
                }
            }
            ("kick", Feel::Breakbeat) => {
                main[at(0)] = 1.0;
                main[at(10)] = rng.range(0.88, 0.96);
                if bar % 2 == 1 {
                    main[at(11)] = rng.range(0.6, 0.7);
                }
            }
            ("kick", Feel::BoomBap) => {
                main[at(0)] = 1.0;
                main[at(10)] = rng.range(0.85, 0.95);
                if bar % 2 == 0 {
                    main[at(7)] = rng.range(0.55, 0.7);
                }
            }
            ("kick", Feel::Afro) => {
                main[at(0)] = 1.0;
                main[at(8)] = rng.range(0.85, 0.95);
                if bar % 2 == 1 {
                    main[at(7)] = rng.range(0.35, 0.5);
                }
            }
            ("snare", Feel::Dembow) => [3, 6, 11, 14].iter().for_each(|&s| main[at(s)] = rng.range(0.88, 1.0)),
            ("rim", Feel::Afro) => [0, 3, 6, 10, 12].iter().for_each(|&s| main[at(s)] = rng.range(0.5, 0.7)),
            ("kick", Feel::TwoStep) => {
                main[at(0)] = 1.0;
                if bar % 2 == 0 {
                    main[at(10)] = rng.range(0.85, 0.95);
                } else {
                    main[at(11)] = rng.range(0.85, 0.95);
                    if rng.chance(0.5) {
                        main[at(7)] = rng.range(0.3, 0.45);
                    }
                }
            }
            ("clap" | "snare", Feel::HalfTime) => {
                main[at(8)] = rng.range(0.95, 1.0);
                if last && lane == "snare" {
                    main[at(15)] = 0.35;
                }
            }
            ("clap" | "snare", _) => {
                main[at(4)] = rng.range(0.92, 1.0);
                main[at(12)] = rng.range(0.92, 1.0);
                if lane == "snare" && matches!(feel, Feel::Breakbeat | Feel::BoomBap | Feel::TwoStep) {
                    for s in [7, 9, 15] {
                        if rng.chance(0.4) {
                            main[at(s)] = rng.range(0.25, 0.4);
                        }
                    }
                }
                if last && rng.chance(0.5) {
                    main[at(15)] = 0.4;
                }
            }
            ("hat", Feel::FourFloor) => {
                for s in (0..16).step_by(2) {
                    main[at(s)] = if s % 4 == 2 { rng.range(0.72, 0.82) } else { rng.range(0.3, 0.42) };
                }
                for s in [3, 7, 11, 15] {
                    if rng.chance(0.3) {
                        main[at(s)] = rng.range(0.25, 0.35);
                    }
                }
            }
            ("hat", Feel::HalfTime) => {
                for s in (0..16).step_by(2) {
                    main[at(s)] = rng.range(0.6, 0.75);
                }
                let roll = if bar % 2 == 1 { 12 } else { 6 };
                for s in roll..roll + 3 {
                    main[at(s)] = rng.range(0.45, 0.6);
                }
            }
            ("hat", Feel::TwoStep) => {
                for s in [2, 6, 10, 14] {
                    main[at(s)] = rng.range(0.6, 0.75);
                    if rng.chance(0.5) {
                        main[at(s + 1)] = rng.range(0.3, 0.4);
                    }
                }
            }
            ("hat", _) => {
                for s in (0..16).step_by(2) {
                    main[at(s)] = if s % 4 == 0 { rng.range(0.7, 0.8) } else { rng.range(0.45, 0.55) };
                }
            }
            ("ohat", _) => [2, 6, 10, 14].iter().for_each(|&s| main[at(s)] = rng.range(0.74, 0.82)),
            ("shaker", _) => {
                for s in 0..16 {
                    if s % 4 != 0 || rng.chance(0.3) {
                        main[at(s)] = if s % 2 == 1 { rng.range(0.42, 0.5) } else { rng.range(0.3, 0.38) };
                    }
                }
            }
            ("ride", _) => (0..16)
                .step_by(2)
                .for_each(|s| main[at(s)] = if s % 4 == 0 { rng.range(0.6, 0.7) } else { rng.range(0.4, 0.5) }),
            _ => {
                let pool = [3, 6, 7, 10, 11, 13, 14];
                for s in pool {
                    if rng.chance(0.4) {
                        main[at(s)] = rng.range(0.6, 0.75);
                    }
                }
            }
        }
    }
    match lane {
        "kick" | "hat" | "perc" => {
            sparse = Some(
                main.iter()
                    .enumerate()
                    .map(|(i, &x)| {
                        if lane == "kick" {
                            x * 0.94
                        } else if i % 4 == 2 && x > 0.0 {
                            0.72
                        } else if lane == "perc" && i % 8 == 6 {
                            0.6
                        } else {
                            0.0
                        }
                    })
                    .collect(),
            )
        }
        "clap" | "snare" => {
            let mut a = main.clone();
            for bar in 0..bars as usize {
                a[bar * 16 + 14] = 0.45;
                a[bar * 16 + 15] = 0.6;
            }
            alt = Some(a);
        }
        _ => {}
    }
    let mut variants = vec![("main".to_string(), main)];
    if let Some(s) = sparse {
        variants.push(("sparse".into(), s));
    }
    if let Some(a) = alt {
        variants.push(("alt".into(), a));
    }
    DrumView {
        lane: lane.into(),
        pattern_bars: bars,
        variants,
        trigs: Vec::new(),
        length: None,
        speed: 1.0,
        swing: None,
        groove: None,
        groove_amount: 1.0,
    }
}

fn energy(kind: &str, rng: &mut Rng) -> f32 {
    let base = match kind {
        "intro" | "outro" => 0.4,
        "break" => 0.35,
        "verse" => 0.55,
        "build" => 0.6,
        "groove" => 0.65,
        "hook" => 0.85,
        "drop" => 0.95,
        _ => 0.6,
    };
    ((base + (rng.f32() - 0.5) * 0.06) * 100.0).round() / 100.0
}

fn section_layers(kind: &str, song: &SongView, genre: &Genre) -> Vec<String> {
    let drums: Vec<&str> = song.drums.iter().map(|d| d.lane.as_str()).collect();
    let parts: Vec<&str> = song.parts.iter().map(|p| p.name.as_str()).collect();
    let loops: Vec<&str> = song.tracks.iter().filter(|t| t.kind == "loop").map(|t| t.name.as_str()).collect();
    let is = |l: &str, set: &[&str]| set.contains(&l);
    let club = matches!(genre.feel, Feel::FourFloor | Feel::TwoStep);
    let pick = |l: &&str| -> bool {
        let l = *l;
        match kind {
            "intro" => {
                (club && l == "kick")
                    || is(l, &["hat", "shaker", "perc", "ride"])
                    || (genre.feel == Feel::BoomBap && is(l, &["keys", "pad"]))
                    || is(l, &loops)
            }
            "verse" | "groove" => !is(l, &["ohat", "lead", "arp", "pad"]) || (kind == "groove" && l == "ohat"),
            "build" => is(l, &["clap", "snare", "hat", "chords", "keys", "lead", "pad"]),
            "break" => is(l, &["pad", "chords", "keys", "lead", "hat"]),
            "outro" => is(l, &["kick", "hat", "shaker", "perc", "bass"]),
            _ => true,
        }
    };
    drums.iter().chain(parts.iter()).chain(loops.iter()).copied().filter(|l| pick(l)).map(|s| s.to_string()).collect()
}

/// what a section of a genre built on samples plays, as trip-hop's profile writes its forms; in a sample's place,
/// what plays instead of it: the drum lanes for the break, keys and strings for the phrase
fn sampled_layers(kind: &str, song: &SongView) -> Vec<String> {
    let has = |name: &str| song.tracks.iter().any(|t| t.name == name);
    let plan: &[&str] = match kind {
        "intro" => &["sample", "texture", "perc"],
        "verse" | "groove" => &["break", "perc", "bass", "sample", "texture"],
        "hook" | "drop" => &["break", "ohat", "perc", "bass", "sample", "strings", "lead", "texture"],
        "break" | "build" => &["sample", "texture", "lead"],
        "outro" => &["sample", "strings", "texture"],
        _ => &["break", "bass", "sample", "texture"],
    };
    let mut out: Vec<String> = Vec::new();
    for &layer in plan {
        let instead: &[&str] = match layer {
            "break" if !has("break") => &["kick", "snare", "hat"],
            "sample" if !has("sample") => &["keys", "strings"],
            _ => std::slice::from_ref(&layer),
        };
        for l in instead {
            if has(l) && !out.iter().any(|o| o == l) {
                out.push(l.to_string());
            }
        }
    }
    out
}

/// whether every mode a genre plays in is minor
fn dark(genre: &Genre) -> bool {
    genre.modes.iter().all(|m| Key::parse(&format!("C {m}")).is_some_and(|k| k.is_minor()))
}

/// a whole new song: title, tempo, key, form, drums, parts, tracks
/// the demo's mix around the tracks, as claudius' genres build theirs: a glue compressor on the drums, the music
/// high-passed, the master's EQ, compressor and saturation, and its reverb and delay
fn demo_mix(genre: &str) -> claudius_api::MixView {
    use serde_json::json;
    claudius_api::MixView {
        buses: vec![
            (
                "drums".into(),
                vec![
                    json!({"fx": "compressor", "threshold_db": -12, "ratio": 2.5, "attack_ms": 10, "release_ms": 100, "knee_db": 6, "makeup_db": 1, "mix": 1}),
                ],
            ),
            ("bass".into(), Vec::new()),
            (
                "music".into(),
                vec![json!({"fx": "eq", "bands": [{"kind": "highpass", "freq": 110, "gain_db": 0, "q": 0.7}]})],
            ),
        ],
        master: vec![
            json!({"fx": "eq", "bands": [{"kind": "highpass", "freq": 28, "gain_db": 0, "q": 0.7}]}),
            json!({"fx": "compressor", "threshold_db": -10, "ratio": 2, "attack_ms": 20, "release_ms": 140, "knee_db": 6, "makeup_db": 0, "mix": 1}),
            json!({"fx": "saturate", "kind": "tanh", "drive_db": 2, "mix": 0.3}),
        ],
        lufs: super::synth::master_level(genre),
        ceiling_db: -1.0,
        reverb: json!({"fx": "reverb", "decay": 2.2, "damping": 0.5, "predelay_ms": 20, "width": 1.1, "mix": 1, "lowcut": 300}),
        delay: json!({"fx": "delay", "beats": 0.75, "feedback": 0.35, "damping": 0.5, "pingpong": true, "mix": 1}),
    }
}

pub fn generate(genre_name: &str, seed: u64, bpm: Option<f64>, key: Option<&str>, loops: bool) -> SongView {
    generate_with(genre_name, seed, bpm, key, loops, &[])
}

/// a whole new song built on the samples `cuts`: the phrase's tempo (unless one is asked for), its key and its chords
/// become the song's. A genre built on samples plays what plays instead of those it has not got
pub fn generate_with(
    genre_name: &str,
    seed: u64,
    bpm: Option<f64>,
    key: Option<&str>,
    loops: bool,
    cuts: &[Cut],
) -> SongView {
    let genre = genre(genre_name);
    let mut rng = Rng::new(seed ^ 0x9e37_79b9_7f4a_7c15);
    // a tempo is drawn only when none is asked for, so that a seed and a tempo make the song they always made
    let phrase = cuts.iter().find(|c| c.role == Role::Phrase);
    let bpm = bpm.unwrap_or_else(|| {
        let drawn = (genre.bpm.0 + (genre.bpm.1 - genre.bpm.0) * rng.f64()).round();
        phrase.map_or(drawn, |p| (p.bpm * 10.0).round() / 10.0)
    });
    let swing = ((genre.swing.0 + (genre.swing.1 - genre.swing.0) * rng.f32()) * 100.0).round() / 100.0;
    let mode = *rng.pick(genre.modes);
    let drawn_key = key.and_then(Key::parse).unwrap_or(Key {
        tonic: rng.below(12) as i32,
        mode: music::MODES.iter().position(|(m, _)| *m == mode).unwrap_or(1),
    });
    // a phrase played faster than it was cut sounds higher: its key and its notes move by as many semitones
    let shift = phrase.map_or(0, |p| p.shift(bpm));
    let key = match phrase.and_then(|p| p.key) {
        Some(heard) => {
            let k = Key { tonic: (heard.tonic + shift).rem_euclid(12), mode: heard.mode };
            // a genre with no major mode plays a major phrase in its relative minor: the same notes
            if dark(genre) && !k.is_minor() { Key { tonic: (k.tonic + 9) % 12, mode: 1 } } else { k }
        }
        None => drawn_key,
    };
    let title = format!("{} {}", rng.pick(ADJECTIVES), rng.pick(NOUNS));

    let drums: Vec<DrumView> = genre.drums.iter().map(|l| drum_lane(l, genre.feel, &mut rng)).collect();
    let parts: Vec<PartView> = genre
        .parts
        .iter()
        .map(|(name, role, styles, presets, register)| PartView {
            name: name.to_string(),
            role: role.to_string(),
            style: rng.pick(styles).to_string(),
            styles: styles.iter().map(|s| s.to_string()).collect(),
            preset: presets[0].to_string(),
            presets: presets.iter().map(|s| s.to_string()).collect(),
            register: *register,
            density: ((0.3 + rng.f32() * 0.35) * 100.0).round() / 100.0,
            cycles: Vec::new(),
        })
        .collect();
    let mut tracks: Vec<TrackView> = drums
        .iter()
        .map(|d| {
            let lane = d.lane.as_str();
            let sound = match lane {
                "clap" if loops => "clap cut from 'Deep Blue Void Claps' (Apple Loops)".to_string(),
                "perc" if loops => "perc cut from 'Sparkle and Shine Topper 01' (Apple Loops)".to_string(),
                "hat" => "synth closed_hat".into(),
                "ohat" => "synth open_hat".into(),
                other => format!("synth {other}"),
            };
            let (pan, reverb, delay) = match lane {
                "hat" => (0.15, 0.0, 0.0),
                "ohat" => (-0.1, 0.0, 0.0),
                "shaker" => (-0.3, 0.0, 0.0),
                "perc" => (0.35, 0.0, 0.15),
                "ride" => (0.2, 0.05, 0.0),
                "clap" => (0.0, 0.18, 0.0),
                "snare" => (0.0, if genre.feel == Feel::Straight { 0.35 } else { 0.15 }, 0.0),
                _ => (0.0, 0.0, 0.0),
            };
            TrackView {
                name: lane.into(),
                kind: "drum".into(),
                role: lane.into(),
                sound,
                level_db: 0.0,
                pan,
                reverb,
                delay,
                duck: 0.0,
                muted: false,
                solo: false,
                loudness: None,
                fx: Vec::new(),
                bus: "drums".into(),
            }
        })
        .collect();
    for p in &parts {
        let (reverb, delay, duck) = match p.role.as_str() {
            "bass" => (0.0, 0.0, 0.55),
            "chords" | "keys" => (0.2, 0.08, 0.45),
            "pad" => (0.35, 0.0, 0.5),
            "lead" => (0.25, 0.2, 0.2),
            _ => (0.2, 0.25, 0.3),
        };
        tracks.push(TrackView {
            name: p.name.clone(),
            kind: "part".into(),
            role: p.role.clone(),
            sound: format!("synth patch '{}'", p.preset),
            level_db: 0.0,
            pan: if p.role == "arp" { -0.2 } else { 0.0 },
            reverb,
            delay,
            duck,
            muted: false,
            solo: false,
            loudness: None,
            fx: Vec::new(),
            bus: if p.role == "bass" { "bass".into() } else { "music".into() },
        });
    }
    if loops || genre.name == "house" {
        tracks.push(TrackView {
            name: "top".into(),
            kind: "loop".into(),
            role: "top".into(),
            sound: format!("loop 'Sparkle and Shine Topper 01' ({bpm:.0} BPM, Apple Loops)"),
            level_db: -2.0,
            pan: 0.1,
            reverb: 0.1,
            delay: 0.0,
            duck: 0.3,
            muted: false,
            solo: false,
            loudness: None,
            fx: Vec::new(),
            bus: "drums".into(),
        });
    }
    for c in cuts {
        // as trip-hop's profile mixes its samples: the break on the drum bus, the phrase up 2 dB into reverb and delay
        let (bus, level_db, reverb, delay) = match c.role {
            Role::Break => ("drums", 0.0, 0.0, 0.0),
            Role::Phrase => ("music", 2.0, 0.3, 0.15),
            Role::Texture => ("music", 0.0, 0.0, 0.0),
        };
        tracks.push(TrackView {
            name: c.role.track().into(),
            kind: "loop".into(),
            role: c.role.level().into(),
            sound: c.label(),
            level_db,
            pan: 0.0,
            reverb,
            delay,
            duck: 0.0,
            muted: false,
            solo: false,
            loudness: None,
            fx: Vec::new(),
            bus: bus.into(),
        });
    }
    tracks.push(TrackView {
        name: "fx".into(),
        kind: "fx".into(),
        role: "fx".into(),
        sound: "synth riser, impact, reverse crash".into(),
        level_db: -1.0,
        pan: 0.0,
        reverb: 0.3,
        delay: 0.0,
        duck: 0.0,
        muted: false,
        solo: false,
        loudness: None,
        fx: Vec::new(),
        bus: "fx".into(),
    });

    let mut progressions = progressions(key.mode_name(), &mut rng);
    if let (Some(p), Some(a)) = (phrase, progressions.first_mut()) {
        a.1 = super::sample::chords(&p.bars_moved(shift), &key);
    }
    let mut lineage =
        vec![format!("generated: {} seed {seed}{}", genre.name, if loops { " (around Apple Loops)" } else { "" })];
    if !cuts.is_empty() {
        let what: Vec<String> =
            cuts.iter().map(|c| format!("{} '{}' by {}", c.role.noun(), c.found.title, c.found.creator)).collect();
        lineage.push(format!("sampled: {}", what.join(", ")));
    }
    let mut song = SongView {
        title,
        genre: genre.name.into(),
        seed,
        bpm,
        key: key.name(),
        swing,
        progressions,
        drums,
        parts,
        tracks,
        mix: demo_mix(genre.name),
        lineage,
        ..Default::default()
    };
    let samples = sampling(genre.name).is_some();
    song.layers = song
        .drums
        .iter()
        .map(|d| d.lane.clone())
        .chain(song.parts.iter().map(|p| p.name.clone()))
        .chain(song.tracks.iter().filter(|t| t.kind == "loop").map(|t| t.name.clone()))
        .collect();
    for (name, kind, bars) in genre.form {
        let has_sparse = |s: &SongView| s.drums.iter().any(|d| d.variants.iter().any(|(v, _)| v == "sparse"));
        let variant = if matches!(*kind, "intro" | "outro") && has_sparse(&song) {
            "sparse"
        } else if *name == "drop2" || *name == "hook3" {
            "alt"
        } else {
            "main"
        };
        let layers = if samples { sampled_layers(kind, &song) } else { section_layers(kind, &song, genre) };
        // what plays the phrase plays its chords
        let prog = if matches!(*kind, "break" | "build") && !layers.iter().any(|l| l == "sample") { "B" } else { "A" };
        let section = SectionView {
            name: name.to_string(),
            kind: kind.to_string(),
            start_bar: 0,
            bars: *bars,
            energy: energy(kind, &mut rng),
            layers,
            prog: prog.into(),
            variant: variant.into(),
            filter_in: *kind == "intro",
            build: *kind == "build" || (*kind == "break" && rng.chance(0.4)),
            gap: *kind == "build",
            impact: matches!(*kind, "drop" | "hook"),
            fills: matches!(*kind, "drop" | "groove" | "hook" | "verse"),
        };
        song.sections.push(section);
    }
    crate::edits::relayout(&mut song);
    song.script = script(&song);
    song
}

fn mini(steps: &[f32], sound: &str) -> String {
    // the first bar in mini-notation: "bd ~ ~ ~ bd ~ ~ ~ ..."
    let bar: Vec<String> = steps
        .iter()
        .take(16)
        .map(|&v| {
            if v <= 0.0 {
                "~".to_string()
            } else if v < 0.45 {
                format!("{sound}:ghost")
            } else {
                sound.to_string()
            }
        })
        .collect();
    let mut groups = Vec::new();
    for g in bar.chunks(4) {
        if g.iter().all(|s| s == "~") {
            groups.push("~".to_string());
        } else {
            groups.push(format!("[{}]", g.join(" ")));
        }
    }
    groups.join(" ")
}

/// the song as a (Strudel-flavoured) claudius script
pub fn script(song: &SongView) -> String {
    let mut out = String::new();
    out += &format!("// {}: {} in {} at {:.0} BPM (seed {})\n", song.title, song.genre, song.key, song.bpm, song.seed);
    out += &format!(
        "song(\"{}\").genre(\"{}\").bpm({}).key(\"{}\").swing({:.2})\n\n",
        song.title, song.genre, song.bpm, song.key, song.swing
    );
    for (name, chords) in &song.progressions {
        out += &format!("chords(\"{name}\", \"<{}>\")\n", chords.join(" "));
    }
    out += "\n// drums: one bar of each lane's main pattern\n";
    let short = |lane: &str| match lane {
        "kick" => "bd",
        "snare" => "sd",
        "clap" => "cp",
        "hat" => "hh",
        "ohat" => "oh",
        "ride" => "rd",
        "shaker" => "sh",
        _ => "perc",
    };
    for d in &song.drums {
        let steps = d.variants.first().map(|(_, s)| s.as_slice()).unwrap_or(&[]);
        let t = song.tracks.iter().find(|t| t.name == d.lane);
        let mut line = format!("track(\"{}\", s(\"{}\")", d.lane, mini(steps, short(&d.lane)));
        if let Some(t) = t {
            line += &format!(".gain({:.2})", (10f32.powf(t.level_db / 20.0) * 0.8).min(1.0));
            if t.pan.abs() > 0.01 {
                line += &format!(".pan({:.2})", t.pan);
            }
            if t.reverb > 0.0 {
                line += &format!(".room({:.2})", t.reverb);
            }
        }
        out += &line;
        out += ")\n";
    }
    out += "\n// parts\n";
    for p in &song.parts {
        out += &format!(
            "part(\"{}\", synth(\"{}\").style(\"{}\").range(\"{}\", \"{}\").density({:.2}))\n",
            p.name,
            p.preset,
            p.style,
            music::note_name(p.register.0),
            music::note_name(p.register.1),
            p.density
        );
    }
    out += "\n// form\nform([\n";
    for s in &song.sections {
        let mut flags = String::new();
        for (on, name) in
            [(s.filter_in, "filterIn"), (s.build, "build"), (s.gap, "gap"), (s.impact, "impact"), (s.fills, "fills")]
        {
            if on {
                flags += &format!(".{name}()");
            }
        }
        out += &format!(
            "  section(\"{}\", \"{}\", {}).energy({:.2}).drums(\"{}\").layers(\"{}\"){flags},\n",
            s.name,
            s.kind,
            s.bars,
            s.energy,
            s.variant,
            s.layers.join(" ")
        );
    }
    out += "])\n";
    out
}

/// the critic's reading of a song (deterministic in the song, a little noise from `rng`)
pub fn judge(song: &SongView, rng: &mut Rng) -> VerdictView {
    let key = Key::parse(&song.key).unwrap_or(Key { tonic: 9, mode: 1 });
    let genre = genre(&song.genre);
    // arc: do louder sections carry more layers, is the drop above the break
    let n = song.sections.len().max(1) as f64;
    let (me, ml) =
        song.sections.iter().fold((0.0, 0.0), |(e, l), s| (e + s.energy as f64 / n, l + s.layers.len() as f64 / n));
    let (mut cov, mut ve, mut vl) = (0.0, 0.0, 0.0);
    for s in &song.sections {
        let (de, dl) = (s.energy as f64 - me, s.layers.len() as f64 - ml);
        cov += de * dl;
        ve += de * de;
        vl += dl * dl;
    }
    let corr = if ve > 0.0 && vl > 0.0 { cov / (ve.sqrt() * vl.sqrt()) } else { 0.0 };
    let max_layers = |kinds: &[&str]| {
        song.sections.iter().filter(|s| kinds.contains(&s.kind.as_str())).map(|s| s.layers.len()).max().unwrap_or(0)
            as f64
    };
    let drop_minus_break = (max_layers(&["drop", "hook"]) - max_layers(&["break"])) * 0.9 + rng.f64() * 0.6;
    let arc = (0.45 + 0.45 * corr.max(0.0) + if drop_minus_break >= 4.0 { 0.1 } else { 0.0 }).clamp(0.0, 1.0);
    // mix: levels far from their targets and muted essentials cost
    let spread: f64 =
        song.tracks.iter().map(|t| (t.level_db as f64).abs()).sum::<f64>() / song.tracks.len().max(1) as f64;
    let kick_muted = song.tracks.iter().any(|t| t.role == "kick" && t.muted);
    let hat_level = song.tracks.iter().find(|t| t.name == "hat").map_or(0.0, |t| t.level_db as f64);
    let mix = (0.99 - spread * 0.035 - if kick_muted { 0.25 } else { 0.0 } - rng.f64() * 0.03).clamp(0.0, 1.0);
    // theory: every numeral should name a chord
    let bad: Vec<&String> =
        song.progressions.iter().flat_map(|(_, c)| c).filter(|c| music::chord(c, &key).is_none()).collect();
    let theory = (1.0 - 0.15 * bad.len() as f64).clamp(0.0, 1.0);
    let in_range = (genre.bpm.0 - 2.0..=genre.bpm.1 + 2.0).contains(&song.bpm);
    let genre_score = (0.62 + if in_range { 0.14 } else { -0.12 } + rng.f64() * 0.12).clamp(0.0, 1.0);
    let production = (0.55 + rng.f64() * 0.18 - spread * 0.01).clamp(0.0, 1.0);
    let scores = vec![
        ("mix".to_string(), mix),
        ("arc".into(), arc),
        ("theory".into(), theory),
        ("genre".into(), genre_score),
        ("production".into(), production),
    ];
    let weights = [0.25, 0.25, 0.15, 0.2, 0.15];
    let total = scores.iter().zip(weights).map(|((_, s), w)| s * w).sum::<f64>() * 100.0;

    let presence = -24.2 + rng.f64() * 2.0 + hat_level * 0.4;
    let mut issues = Vec::new();
    if presence < -23.0 {
        issues.push((
            0.12 + (-23.0 - presence) * 0.1,
            format!("dull 4-8 kHz: hats, claps and snare lack bite (presence {presence:.1} dB, healthy from -23)"),
            "band:presence-".to_string(),
        ));
    }
    if drop_minus_break < 4.0 {
        issues.push((0.35, format!("the drop is only {drop_minus_break:.1} LU above the break (aim for 4 or more): thin the break or add layers to the drop"), "form:drop+".into()));
    }
    if kick_muted {
        issues.push((0.6, "the kick is muted: the groove has no floor".into(), "mix:kick".into()));
    }
    if hat_level > 3.0 {
        issues.push((0.25, format!("hats sit {hat_level:.1} dB over their target: harsh top end"), "mix:hat-".into()));
    }
    for c in &bad {
        issues.push((0.3, format!("'{c}' is not a chord claudius can read in {}", song.key), "harmony".into()));
    }
    if !in_range {
        issues.push((
            0.3,
            format!("{:.0} BPM is unusual for {} ({:.0}-{:.0})", song.bpm, genre.name, genre.bpm.0, genre.bpm.1),
            "tempo".into(),
        ));
    }
    if spread > 3.0 {
        issues.push((
            0.2,
            format!("levels drift {spread:.1} dB from their role targets on average"),
            "mix:levels".into(),
        ));
    }
    issues.sort_by(|a, b| b.0.total_cmp(&a.0));

    let alt = *rng.pick(genre.alts);
    let flaws = [
        "muddy low end",
        "harsh high frequencies",
        "thin sound",
        "clipping distortion",
        "amateur home recording",
        "lo-fi low quality audio",
    ];
    let ears = EarsView {
        genre_query: genre.query.into(),
        genre_z: 1.2 + genre_score * 1.2 + rng.f64() * 0.3,
        best_alt: alt.into(),
        best_alt_z: 0.6 + rng.f64() * 0.8,
        production_z: production * 1.4 - 0.3,
        worst_flaw: rng.pick(&flaws).to_string(),
        worst_flaw_z: -0.4 + rng.f64() * 0.7,
        sounds_like: vec![genre.query.into(), "electronic dance music".into(), alt.into()],
        p_real: Some(0.2 + rng.f64() * 0.3),
    };
    let lufs = -10.0 + rng.f64() * 0.4;
    let measures = vec![
        ("lufs".to_string(), lufs),
        ("true_peak_db".into(), -1.05 - rng.f64() * 0.2),
        ("lra_lu".into(), 3.2 + rng.f64() * 1.5),
        ("band_sub_db".into(), -5.8 + rng.f64()),
        ("band_low_db".into(), -3.4 + rng.f64()),
        ("band_lowmid_db".into(), -6.8 + rng.f64()),
        ("band_mid_db".into(), -12.3 + rng.f64()),
        ("band_presence_db".into(), presence),
        ("band_air_db".into(), -18.6 + rng.f64()),
        ("stereo_correlation".into(), 0.93 + rng.f64() * 0.05),
        ("low_end_correlation".into(), 0.995),
        ("drop_minus_break_lu".into(), drop_minus_break),
        ("arc_correlation".into(), corr),
        ("lead_in_key".into(), 1.0),
        ("bass_roots_on_changes".into(), theory),
        ("length_s".into(), song.seconds),
    ];
    let fix = issues.first().map(|(_, w, _)| format!(" Fix first: {w}")).unwrap_or_default();
    let summary = format!("{total:.0}/100 (arc {arc:.2}, mix {mix:.2}, theory {theory:.2}).{fix}");
    VerdictView { total, summary, scores, issues, ears: Some(ears), measures }
}

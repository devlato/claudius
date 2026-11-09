//! Names and numbers as a person reads them: the names a script writes (`sub_bass`, `kick_follow`, `tr909:bd`), pan,
//! the verdict's measures, the drums' patterns, effects and their settings. The studio shows these beside the values
//! and in their tooltips, so that what it shows reads without the docs at hand.

/// A name as a script writes it, as a person reads it: `sub_bass` "sub bass", `fm_bass` "FM bass", `epiano` "electric
/// piano", `uk_drill` "UK drill", `bd` "kick"; a sound of a pack or a kit with its pack after it, `tr909:bd` "kick
/// (TR-909)".
pub fn name_in_words(name: &str) -> String {
    if let Some((pack, sound)) = name.split_once(':') {
        return format!("{} ({})", name_in_words(sound), name_in_words(pack));
    }
    let drum = match name {
        "bd" => "kick",
        "sd" | "sn" => "snare",
        "cp" => "clap",
        "hh" | "ch" => "closed hat",
        "oh" | "ohat" => "open hat",
        "rd" => "ride",
        "cr" => "crash",
        "lt" => "low tom",
        "mt" => "mid tom",
        "ht" => "high tom",
        "rs" => "rim",
        "sh" => "shaker",
        "cb" => "cowbell",
        _ => "",
    };
    if !drum.is_empty() {
        return drum.to_string();
    }
    let word = |w: &str| -> String {
        match w {
            "fm" => "FM",
            "pwm" => "PWM",
            "eq" => "EQ",
            "uk" => "UK",
            "ny" => "NY",
            "idm" => "IDM",
            "rnb" => "R&B",
            "dnb" => "drum and bass",
            "g" => "G",
            "gfunk" => "G-funk",
            "lofi" => "lo-fi",
            "moog" => "Moog",
            "solina" => "Solina",
            "dist" => "distorted",
            "epiano" => "electric piano",
            "updown" => "up and down",
            "root8" => "root eighths",
            "tr909" => "TR-909",
            "tr808" => "TR-808",
            "tr707" => "TR-707",
            other => other,
        }
        .to_string()
    };
    name.split('_').filter(|w| !w.is_empty()).map(word).collect::<Vec<_>>().join(" ")
}

/// What the engine says a track's sound is (`TrackView::sound`), as a person reads it: a preset or an instrument by
/// its name in words, a synthesized drum as such, each layer of a rack so; a loop or a recording as it is.
/// "synth patch 'deep_bass'" is "deep bass", "sampled bd (tr909:bd)" "kick (TR-909)", "synth closed_hat"
/// "synthesized closed hat".
pub fn sound_in_words(described: &str) -> String {
    if let Some(layers) = described.strip_prefix("a rack of ") {
        return format!("layers: {}", layers.split(" + ").map(sound_in_words).collect::<Vec<_>>().join(" + "));
    }
    // what follows a layer's sound: the band it plays in (" under 90 Hz")
    let then = |name: &str, after: &str| format!("{}{after}", name_in_words(name));
    if let Some((preset, after)) = described.strip_prefix("synth patch '").and_then(|r| r.split_once('\'')) {
        return then(preset, after);
    }
    if let Some(rest) = described.strip_prefix("sampled ") {
        return match rest.split_once(" (").and_then(|(_, r)| r.split_once(')')) {
            Some((name, after)) => then(name, after),
            None => {
                let name = rest.split(' ').next().unwrap_or(rest);
                then(name, &rest[name.len()..])
            }
        };
    }
    if let Some(rest) = described.strip_prefix("synth ") {
        let voice = rest.split(' ').next().unwrap_or(rest);
        return format!("synthesized {}", then(voice, &rest[voice.len()..]));
    }
    described.to_string()
}

/// Where a pan of -1 (left) to 1 (right) puts a track, as a person says it: "centre", "30% left", "all the way right".
pub fn pan_in_words(pan: f32) -> String {
    let side = if pan < 0.0 { "left" } else { "right" };
    let far = (pan.abs() * 100.0).round();
    if far < 1.0 {
        "centre".into()
    } else if far >= 100.0 {
        format!("all the way {side}")
    } else {
        format!("{far:.0}% {side}")
    }
}

/// A band of the verdict's tonal balance (`sub`, `low`, `lowmid`, `mid`, `presence`, `air`) as a person reads it, and
/// the frequencies it spans.
pub fn band_in_words(band: &str) -> (&str, &'static str) {
    match band {
        "sub" => ("sub", "20-60 Hz"),
        "low" => ("lows", "60-250 Hz"),
        "lowmid" | "low-mid" => ("low mids", "250 Hz-1 kHz"),
        "mid" => ("mids", "1-4 kHz"),
        "presence" => ("presence", "4-8 kHz"),
        "air" => ("air", "8-16 kHz"),
        other => (other, ""),
    }
}

/// One of the verdict's measures (`VerdictView::measures`) as a person reads it: what it measures, its value in its
/// unit, and what it says ("loudness", "-9.8 LUFS", "how loud the whole song is ...").
pub fn measure_in_words(key: &str, value: f64) -> (String, String, String) {
    let pct = |v: f64| format!("{:.0}%", v * 100.0);
    let said = |name: &str, value: String, about: &str| (name.to_string(), value, about.to_string());
    match key {
        "lufs" => {
            return said(
                "loudness",
                format!("{value:.1} LUFS"),
                "how loud the whole song is, as streaming services measure it: about -14 for Spotify and Apple \
                 Music, -9 to -6 for loud club music",
            );
        }
        "true_peak_db" => {
            return said(
                "true peak",
                format!("{value:.1} dBTP"),
                "the highest the song's waveform reaches, between its samples too: above -1 it may distort once it \
                 is made an MP3 or a stream",
            );
        }
        "lra_lu" => {
            return said(
                "loudness range",
                format!("{value:.1} LU"),
                "how far the loudness moves between the song's quiet and loud passages: club music keeps it small",
            );
        }
        "stereo_correlation" => {
            return said(
                "stereo correlation",
                format!("{value:.2}"),
                "how alike the left and right channels are: 1 is mono, 0 unrelated, below 0 out of phase; a mix \
                 nearer 1 than 0.96 sounds narrow, one under 0.15 phasey",
            );
        }
        "low_end_correlation" => {
            return said(
                "low end in mono",
                format!("{value:.2}"),
                "the stereo correlation below 150 Hz, where the kick and the bass belong in the middle: 0.9 or more",
            );
        }
        "drop_minus_break_lu" => {
            return said(
                "drop over the break",
                format!("{value:+.1} LU"),
                "how much louder the loudest drop is than the quietest break: a drop should hit harder",
            );
        }
        "arc_correlation" => {
            return said(
                "loudness following the intensity",
                format!("{value:.2}"),
                "how closely each section's loudness follows the intensity it is written at: 1 follows it exactly",
            );
        }
        "bass_roots_on_changes" => {
            return said(
                "bass on the root at chord changes",
                pct(value),
                "how often the bass plays the chord's root when the chord changes",
            );
        }
        "lead_in_key" => return said("lead in the key", pct(value), "the lead's notes that are in the song's key"),
        "lead_chord_tones_on_strong_beats" => {
            return said(
                "lead on chord notes on strong beats",
                pct(value),
                "the lead's notes on strong beats (one and three, or held a beat or more) that are notes of the chord \
                 under them",
            );
        }
        "lead_range_semitones" => {
            return said(
                "lead's range",
                format!("{value:.0} semitones"),
                "from the lead's lowest note to its highest: a sung melody spans about an octave and a half",
            );
        }
        "lead_stepwise" => {
            return said(
                "lead moving by steps",
                pct(value),
                "the lead's moves of a tone or less: a melody that is easy to sing moves mostly by steps",
            );
        }
        "length_s" => {
            let whole = value.max(0.0).round() as u64;
            return said(
                "length",
                format!("{}:{:02}", whole / 60, whole % 60),
                "how long the song runs: most tracks run two to five minutes",
            );
        }
        "stock_share" => {
            return said(
                "stock music",
                pct(value),
                "how far the listening model hears it as stock library music rather than its genre: the verdict \
                 loses up to 40% of itself by it",
            );
        }
        _ => {}
    }
    // measures of one band, track or section: by its name, then what of it is measured
    let of = |name: &str, what: &str, value: String, about: &str| (format!("{name}: {what}"), value, about.to_string());
    if let Some(band) = key.strip_prefix("band_") {
        if let Some(b) = band.strip_suffix("_off_db") {
            let (name, _) = band_in_words(b);
            return of(
                name,
                "off the healthy range",
                format!("{value:+.1} dB"),
                "how far above (+) or below (-) the range a healthy master keeps it in",
            );
        }
        if let Some(b) = band.strip_suffix("_db") {
            let (name, hz) = band_in_words(b);
            return (
                name.to_string(),
                format!("{value:.1} dB"),
                format!("the share of the song's energy in the {name}, {hz}"),
            );
        }
    }
    if let Some(track) = key.strip_prefix("level_").and_then(|k| k.strip_suffix("_off_db")) {
        return of(
            track,
            "level off its role's",
            format!("{value:+.1} dB"),
            "how much louder (+) or quieter (-) it plays than tracks of its role are mixed at: 3 dB either way is a \
             choice, more a fault",
        );
    }
    if let Some(section) = key.strip_prefix("section_").and_then(|k| k.strip_suffix("_lufs")) {
        let name = section.split_once('_').map_or(section, |(_, n)| n);
        return of(
            name,
            "loudness",
            format!("{value:.1} LUFS"),
            "the section's typical loudness: its median short-term loudness",
        );
    }
    if let Some(rest) = key.strip_prefix("time_") {
        if let Some(track) = rest.strip_suffix("_own_bpm") {
            return of(
                track,
                "its own tempo",
                format!("{value:.1} BPM"),
                "the tempo on whose sixteenths its onsets gather most tightly",
            );
        }
        if let Some(track) = rest.strip_suffix("_drift_ms") {
            return of(
                track,
                "drift off the beat",
                format!("{value:.0} ms"),
                "how far it wanders off the song's beat over four bars: a band playing live wanders about 30 ms, \
                 more than 40 sounds out of time",
            );
        }
        if let Some(track) = rest.strip_suffix("_on_grid") {
            return of(
                track,
                "on the song's grid",
                pct(value),
                "how tightly its onsets gather on the song's sixteenths: 100% all on one place of them",
            );
        }
        if let Some(track) = rest.strip_suffix("_own") {
            return of(
                track,
                "on a grid of its own",
                pct(value),
                "how tightly its onsets gather on the sixteenths of the tempo they keep best",
            );
        }
    }
    if let Some(track) = key.strip_prefix("voice_leading_") {
        return of(
            track,
            "voice leading",
            format!("{value:.1} semitones"),
            "how far its chords' voices move at each change, on average: smooth chords move 1 to 3",
        );
    }
    (name_in_words(key), format!("{value:.2}"), String::new())
}

/// What notes sung or played become in one of the `NOTE_STYLES`, in a few words.
pub fn note_style_in_words(style: &str) -> &'static str {
    match style {
        "line" => "the line as it is",
        "harmony" => "the line with a third above it",
        "octaves" => "the line doubled an octave down",
        "chords" => "a triad under each note",
        "bass" => "the line down in a bass's register",
        "arp" => "a rising arpeggio on each note",
        "pad" => "a chord held through each bar the line sounds in",
        _ => "",
    }
}

/// A drum pattern variant (`SectionView::variant`) as a person reads it, and what it plays.
pub fn variant_in_words(variant: &str) -> (String, &'static str) {
    match variant {
        "main" => ("main".into(), "the beat as most of the song plays it"),
        "sparse" => ("sparse".into(), "fewer hits, for intros, breaks and outros"),
        "alt" => ("alternate".into(), "another take on the beat, for a lift"),
        "half" => ("half time".into(), "the backbeat on three alone: the beat feels half as fast"),
        other => (name_in_words(other), "a beat the genre plays in some of its sections"),
    }
}

/// An effect, by the name a chain gives it (`{"fx": "saturate"}`), as a person reads it, and what it does.
pub fn effect_in_words(fx: &str) -> (String, &'static str) {
    let (name, about) = match fx {
        "gain" => ("gain", "turns it up or down"),
        "eq" => ("EQ", "boosts or cuts bands of its frequencies"),
        "filter" => ("filter", "lets some of its frequencies through and takes the rest away"),
        "compressor" => ("compressor", "turns its loud moments down, evening out its level"),
        "limiter" => ("limiter", "keeps its peaks under a ceiling"),
        "saturate" => ("saturation", "drives it into distortion, from warm to gritty"),
        "bitcrush" => ("bitcrusher", "fewer bits and a lower sample rate: a lo-fi, digital grit"),
        "reverb" => ("reverb", "a room or a hall around it"),
        "delay" => ("delay", "echoes of it, in time with the song"),
        "chorus" => ("chorus", "slightly delayed, wavering copies of it, for width and shimmer"),
        "flanger" => ("flanger", "a short delay that sweeps, a jet's whoosh"),
        "phaser" => ("phaser", "notches sweeping through it, a swirl"),
        "width" => ("stereo width", "narrows or widens its stereo image, and can keep its lows in mono"),
        "tremolo" => ("tremolo", "its level pulsing in time with the song"),
        "auto_pan" => ("auto pan", "it moving between left and right in time with the song"),
        "gate" => ("gate", "chops it into steps of the beat, each at a level of its own"),
        "transient" => {
            ("transient shaper", "sharpens or softens the start of each hit, and lengthens or shortens its tail")
        }
        "tape" => ("tape", "the wow, flutter, saturation and dulled top of a tape machine"),
        "vinyl" => ("vinyl", "the crackle and hiss of a record"),
        "tape_stop" => ("tape stop", "it slowing to a halt, as a tape machine stopped"),
        "reverse" => ("reverse", "a stretch of it played backwards"),
        "stutter" => ("stutter", "a slice of it repeated, a glitch"),
        "parallel" => ("parallel chains", "chains side by side, mixed with the sound as it was"),
        "tune" => ("tuning", "pulls a voice's notes onto the chords, the key or the semitones"),
        _ => return (name_in_words(fx), ""),
    };
    (name.to_string(), about)
}

/// One of an effect's settings, by its key, as a person reads it: what it is, its unit ("" for none), and what it
/// does where that is not plain from its name.
pub fn setting_in_words(key: &str) -> (String, &'static str, &'static str) {
    let (name, unit, about) = match key {
        "db" => ("gain", "dB", ""),
        "mix" => ("mix", "", "how much of the effect is heard: 0 none of it, 1 all of it"),
        "threshold_db" => ("threshold", "dB", "the level above which it turns the sound down"),
        "ratio" => ("ratio", ":1", "how hard it turns down what goes over the threshold: at 4, 4 dB over comes out 1"),
        "attack_ms" => ("attack", "ms", "how fast it starts to work"),
        "release_ms" => ("release", "ms", "how fast it lets go"),
        "knee_db" => ("knee", "dB", "how gently it starts to work around the threshold"),
        "makeup_db" => ("make-up gain", "dB", "the level it gives back after turning down"),
        "ceiling_db" => ("ceiling", "dB", "the highest its peaks may reach"),
        "mode" => ("type", "", ""),
        "cutoff" => ("cutoff", "Hz", "where the filter starts to take frequencies away"),
        "res" => ("resonance", "", "how much it rings at the cutoff"),
        "drive" => ("drive", "", "how hard the ladder filter is driven"),
        "kind" => ("character", "", ""),
        "drive_db" => ("drive", "dB", "how hard it is driven: more is dirtier"),
        "bits" => ("bit depth", "bits", "fewer is grittier"),
        "downsample" => ("sample-rate cut", "x", "1 keeps the rate, more divides it"),
        "decay" => ("decay", "s", "how long the reverb's tail rings"),
        "damping" => ("damping", "", "how much darker the tail grows as it dies away"),
        "predelay_ms" => ("pre-delay", "ms", "the gap before the reverb starts"),
        "lowcut" => ("low cut", "Hz", "the lows taken out of the reverb, so that it does not muddy the mix"),
        "lowcut_hz" => ("low cut", "Hz", ""),
        "beats" => ("time", "beats", ""),
        "feedback" => ("feedback", "", "how much of each repeat comes round again"),
        "pingpong" => ("ping-pong", "", "the echoes bouncing between left and right"),
        "rate_hz" => ("rate", "Hz", "how fast it sweeps"),
        "depth_ms" => ("depth", "ms", "how far it sweeps"),
        "delay_ms" => ("delay", "ms", ""),
        "stages" => ("stages", "", "more notches, a deeper swirl"),
        "depth" => ("depth", "", "how far it moves: 0 not at all, 1 all the way"),
        "width" => ("width", "", "0 mono, 1 as it is, more is wider"),
        "mono_below_hz" => ("mono below", "Hz", "the lows kept in the middle"),
        "steps" => ("steps", "", "a level for each step"),
        "step_beats" => ("step length", "beats", ""),
        "smooth_ms" => ("smoothing", "ms", "how softly it moves from step to step"),
        "attack_db" => ("attack", "dB", "the start of each hit, louder or softer"),
        "sustain_db" => ("sustain", "dB", "the tail of each hit, louder or softer"),
        "wow" => ("wow", "", "a slow wavering of the pitch"),
        "flutter" => ("flutter", "", "a fast wavering of the pitch"),
        "lowpass_hz" => ("top cut", "Hz", "the highs the tape dulls"),
        "crackle" => ("crackle", "", ""),
        "hiss_db" => ("hiss", "dB", ""),
        "start" => ("start", "beat", ""),
        "len" => ("length", "beats", ""),
        "slice" => ("slice", "beats", ""),
        "dry" => ("dry", "", "the sound as it was, mixed in"),
        "amount" => ("amount", "", ""),
        "strength" => ("strength", "", "how far notes are pulled onto the scale: 1 all the way"),
        "speed_ms" => ("speed", "ms", "how fast a note is pulled: fast is robotic"),
        "humanize" => ("humanize", "cents", "how much of the voice's own wavering it keeps"),
        "vibrato" => ("vibrato", "", "how much of the voice's vibrato it keeps"),
        "scale" => ("tuned to", "", ""),
        "voice" => ("voice", "", ""),
        _ => {
            for (suffix, unit) in [("_db", "dB"), ("_ms", "ms"), ("_hz", "Hz")] {
                if let Some(name) = key.strip_suffix(suffix) {
                    return (name_in_words(name), unit, "");
                }
            }
            return (name_in_words(key), "", "");
        }
    };
    (name.to_string(), unit, about)
}

/// One of the choices an effect's setting takes (a filter's `mode`, a saturation's `kind`, an EQ band's `kind`, the
/// notes `tune` pulls onto) as a person reads it: `lp` "low-pass", `tanh` "soft clip", `chords` "the chords' notes".
pub fn option_in_words(option: &str) -> String {
    match option {
        "lp" | "lowpass" => "low-pass",
        "hp" | "highpass" => "high-pass",
        "bp" | "bandpass" => "band-pass",
        "allpass" => "all-pass",
        "lowshelf" => "low shelf",
        "highshelf" => "high shelf",
        "ladder" => "ladder (a resonant low-pass)",
        "tanh" => "soft clip",
        "hard" => "hard clip",
        "fold" => "wavefold",
        "chords" => "the chords' notes",
        "key" => "the key's notes",
        "chromatic" => "every semitone",
        other => return name_in_words(other),
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_reads_as_people_write_it() {
        assert_eq!(name_in_words("sub_bass"), "sub bass");
        assert_eq!(name_in_words("fm_bass"), "FM bass");
        assert_eq!(name_in_words("epiano"), "electric piano");
        assert_eq!(name_in_words("kick_follow"), "kick follow");
        assert_eq!(name_in_words("rnb_keys"), "R&B keys");
        assert_eq!(name_in_words("uk_drill"), "UK drill");
        assert_eq!(name_in_words("808"), "808");
        assert_eq!(name_in_words("bd"), "kick");
        assert_eq!(name_in_words("tr909:bd"), "kick (TR-909)");
        assert_eq!(name_in_words("salamander:piano"), "piano (salamander)");
    }

    #[test]
    fn a_sound_the_engine_describes_reads_in_words() {
        assert_eq!(sound_in_words("synth patch 'deep_bass'"), "deep bass");
        assert_eq!(sound_in_words("sampled bd (tr909:bd)"), "kick (TR-909)");
        assert_eq!(sound_in_words("sampled salamander:piano"), "piano (salamander)");
        assert_eq!(sound_in_words("synth closed_hat"), "synthesized closed hat");
        assert_eq!(
            sound_in_words("a rack of synth patch 'sub_bass' under 90 Hz + sampled bd (tr909:bd) above 1.2 kHz"),
            "layers: sub bass under 90 Hz + kick (TR-909) above 1.2 kHz"
        );
        assert_eq!(sound_in_words("loop 'Deep Blue Void' (122 BPM)"), "loop 'Deep Blue Void' (122 BPM)");
    }

    #[test]
    fn a_pan_says_which_side_and_how_far() {
        assert_eq!(pan_in_words(0.0), "centre");
        assert_eq!(pan_in_words(0.004), "centre");
        assert_eq!(pan_in_words(-0.3), "30% left");
        assert_eq!(pan_in_words(0.25), "25% right");
        assert_eq!(pan_in_words(1.0), "all the way right");
        assert_eq!(pan_in_words(-1.0), "all the way left");
    }

    #[test]
    fn every_measure_the_critic_takes_has_a_name_and_a_unit() {
        let words = |k: &str, v: f64| {
            let (name, value, about) = measure_in_words(k, v);
            format!("{name}: {value}{}", if about.is_empty() { "" } else { " (said)" })
        };
        assert_eq!(words("lufs", -9.84), "loudness: -9.8 LUFS (said)");
        assert_eq!(words("true_peak_db", -1.04), "true peak: -1.0 dBTP (said)");
        assert_eq!(words("length_s", 245.0), "length: 4:05 (said)");
        assert_eq!(words("lead_in_key", 0.93), "lead in the key: 93% (said)");
        assert_eq!(words("band_presence_db", -26.04), "presence: -26.0 dB (said)");
        assert_eq!(words("band_lowmid_off_db", 2.5), "low mids: off the healthy range: +2.5 dB (said)");
        assert_eq!(words("level_reverse_crash_off_db", -4.2), "reverse_crash: level off its role's: -4.2 dB (said)");
        assert_eq!(words("section_03_drop_2_lufs", -8.2), "drop_2: loudness: -8.2 LUFS (said)");
        assert_eq!(words("time_sample_own_bpm", 140.0), "sample: its own tempo: 140.0 BPM (said)");
        assert_eq!(words("time_sample_own", 0.4), "sample: on a grid of its own: 40% (said)");
        assert_eq!(words("time_sample_on_grid", 0.02), "sample: on the song's grid: 2% (said)");
        assert_eq!(words("time_sample_drift_ms", 60.0), "sample: drift off the beat: 60 ms (said)");
        assert_eq!(words("voice_leading_chords", 2.4), "chords: voice leading: 2.4 semitones (said)");
        assert_eq!(words("something_new", 0.5), "something new: 0.50");
    }

    #[test]
    fn an_effects_settings_read_with_their_units() {
        assert_eq!(
            setting_in_words("threshold_db"),
            ("threshold".into(), "dB", "the level above which it turns the sound down")
        );
        assert_eq!(setting_in_words("cutoff").1, "Hz");
        assert_eq!(setting_in_words("decay").1, "s");
        assert_eq!(setting_in_words("hold_ms"), ("hold".into(), "ms", ""));
        assert_eq!(setting_in_words("shine"), ("shine".into(), "", ""));
        assert_eq!(option_in_words("lp"), "low-pass");
        assert_eq!(option_in_words("tube"), "tube");
        assert_eq!(effect_in_words("auto_pan").0, "auto pan");
        assert_eq!(effect_in_words("unheard_of"), ("unheard of".into(), ""));
    }
}

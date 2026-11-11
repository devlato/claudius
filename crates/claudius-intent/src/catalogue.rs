//! Everything the studio does that its command palette offers: each action with its name, the other words it is found
//! by (and what it is for, in a person's words, which its meaning is read from with its name), where it lives in the
//! studio, the key that does it too, and the id the studio acts on.

/// An action of the studio.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    /// what the studio acts on: an action's own id, `tab:<tab>` for a tab
    pub id: &'static str,
    pub name: &'static str,
    pub words: &'static str,
    /// where it lives in the studio, for a person to find it there next time
    pub place: &'static str,
    /// the key that does it too (`Cmd` is Ctrl off a Mac)
    pub keys: &'static str,
}

const fn e(
    id: &'static str,
    name: &'static str,
    words: &'static str,
    place: &'static str,
    keys: &'static str,
) -> Entry {
    Entry { id, name, words, place, keys }
}

/// every action, in the order the palette lists them before anything is typed
pub const ENTRIES: &[Entry] = &[
    e("play", "Play or pause", "start listen hear the song, pause playback", "transport", "Space"),
    e("to_start", "Back to the start", "rewind to the beginning, home, go to the top", "transport", "Home"),
    e("stop", "Stop", "halt playback and rewind", "transport", ""),
    e("loop", "Loop", "repeat a section over and over, play a part on repeat, cycle the loop region", "transport", "L"),
    e("undo", "Undo", "go back, revert, take back the last change", "toolbar", "Cmd+Z"),
    e("redo", "Redo", "again, forward, bring back the change undone", "toolbar", "Shift+Cmd+Z"),
    e(
        "new_song",
        "New song...",
        "make or create a song, generate, compose, write a track in a genre, start over",
        "toolbar",
        "",
    ),
    e(
        "example",
        "Open an example song...",
        "examples, a library of demo songs to learn from, a study of a style",
        "New song, From an example",
        "",
    ),
    e("open", "Open a song...", "load a file, a blueprint, a script, a project", "toolbar", ""),
    e("save", "Save or download", "download, export, keep the song as a file, wav mp3 flac audio", "toolbar", ""),
    e(
        "render",
        "Render",
        "listen to the latest version, judge it, how good is it, rate it, the verdict and its score out of 100, what \
         the critic thinks, bounce, mix and master",
        "status bar",
        "",
    ),
    e(
        "record",
        "Record a take...",
        "microphone mic, sing, a voice, track vocals, lay down a part, play a guitar or an instrument, a MIDI \
         keyboard, record over the song",
        "transport, the red button",
        "",
    ),
    e(
        "add_recording",
        "Add a recording or sample one...",
        "import a file, bring in a recording, an mp3 or a wav, sample vocals, search free libraries, freesound \
         jamendo, a track to chop, a passage, a stab, remake a song",
        "transport, the + button",
        "",
    ),
    e(
        "phone",
        "Play from a phone...",
        "mobile, pads, keys, a turntable to scratch, a controller",
        "transport, the phone button",
        "",
    ),
    e("paint", "Paint tool", "draw cells in the arrangement, switch parts on and off", "arrangement", "B"),
    e("tab:drums", "Drums", "the beat, steps, the sequencer, kick snare hat, drum pattern", "tab", ""),
    e("tab:instruments", "Instruments", "parts, sounds, synths, bass lead pad, layers", "tab", ""),
    e("tab:notes", "Notes", "the piano roll, a melody, MIDI notes", "tab", ""),
    e("tab:chords", "Chords", "harmony, the progression, change the key, the scale, the mode", "tab", ""),
    e(
        "tab:mixer",
        "Mixer",
        "levels, volume, louder or quieter, turn it up or down, mute, solo, pan left or right, faders, effects, fx, \
         the mix, reverb, echo, delay, distortion, eq, compression",
        "tab",
        "",
    ),
    e("tab:sounds", "Sounds", "samples, swap a sound, kits, browse the library", "tab", ""),
    e("tab:script", "Script", "code, rhai, edit the song as text", "tab", ""),
    e(
        "flow_large",
        "Signal flow, large",
        "the graph, a whiteboard, routing, buses, chains, big",
        "arrangement, its Signal flow chip",
        "G",
    ),
    e("tab:flow", "Signal flow", "the graph, a whiteboard, routing, buses, the effects chain", "tab", ""),
    e("tab:log", "Log", "history, messages, what happened, what claudius did, the changes made", "tab", ""),
    e(
        "variations",
        "Try variations",
        "mutate, alternatives, other versions, ideas, suggestions, change it a little, surprise me",
        "status bar, Try",
        "",
    ),
    e(
        "improve",
        "Improve the song",
        "evolve, generations, make it better, optimise for the critic",
        "status bar, Improve",
        "",
    ),
    e(
        "fix_all",
        "Fix what the critic found",
        "quick fix, the issues, problems, repair the verdict, fix the mix",
        "status bar, Fix them all",
        "",
    ),
    e(
        "cancel",
        "Stop what claudius is doing",
        "cancel, abort, halt the running job, stop rendering, stop the evolution",
        "status bar",
        "",
    ),
    e(
        "stems",
        "Render the tracks",
        "stems, one wav a track, export the parts separately, for mixing in another studio or daw",
        "status bar",
        "",
    ),
    e(
        "packs",
        "Download the sample packs",
        "install instruments, real sounds, sampled instruments, kits",
        "status bar",
        "",
    ),
    e(
        "publish",
        "Release the song...",
        "publish, distribute to stores, spotify apple music, artwork, check it for release, isrc",
        "toolbar",
        "",
    ),
    e(
        "assistants",
        "Connect an assistant...",
        "claude, chatgpt, cursor, codex, an ai assistant or agent, mcp, let an assistant make music",
        "toolbar",
        "",
    ),
    e(
        "reference",
        "Script reference",
        "the language, functions, patterns, how to write a script, syntax help",
        "Script tab",
        "",
    ),
    e(
        "tour",
        "Take the tour",
        "help, a guide, learn the studio, get started, show me around, how does it work",
        "",
        "",
    ),
    e("docs", "Read the docs", "help, the manual, a guide, a tutorial, documentation, how to", "on the web", ""),
    e("look:dark", "Dark look", "dark mode, a dark theme, night, darker colours", "? menu, Look", ""),
    e(
        "look:light",
        "Light look",
        "light mode, a light theme, day, bright colours, a white background",
        "? menu, Look",
        "",
    ),
    e(
        "look:system",
        "Look as the system is",
        "an automatic theme, follow the system, dark mode at night, match the computer's appearance",
        "? menu, Look",
        "",
    ),
];

/// the entry of that id
pub fn entry(id: &str) -> Option<&'static Entry> {
    ENTRIES.iter().find(|e| e.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_action_has_an_id_and_a_name_of_its_own_and_words_to_be_found_by() {
        let ids: std::collections::HashSet<&str> = ENTRIES.iter().map(|e| e.id).collect();
        let names: std::collections::HashSet<&str> = ENTRIES.iter().map(|e| e.name).collect();
        assert_eq!((ids.len(), names.len()), (ENTRIES.len(), ENTRIES.len()));
        assert!(ENTRIES.iter().all(|e| !e.words.is_empty()));
        assert_eq!(entry("tab:mixer").map(|e| e.name), Some("Mixer"));
    }
}

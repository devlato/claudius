// "Skyline Fracture": melodic dubstep of the 2010s and 2020s, the emotional, huge-chorded side of bass music (Seven
// Lions, Illenium, Said the Sky). An original song in that style, built to show layered sounds: its chords and its
// bass are each a rack of several.
//
// How it is made:
// - Half time at 140 BPM: the kick on 1, the snare on 3, so the drop feels slow and heavy however fast its hats go.
// - Bittersweet chords, VI VII i v (Db Eb Fm Cm), on a rack of three: a warm pad under 300 Hz keeping the low end
//   clean and in the middle, a supersaw above it spread wide across the stereo, and a bright pluck over 2.5 kHz,
//   quieter, for the shimmer on top. Layered this way, the chords are huge but leave room for the bass.
// - The bass is a rack too: a sine sub under 100 Hz that ducks under the kick, and a wobble from 100 Hz to 2 kHz,
//   distorted, whose own filter an LFO in time with the song opens and closes every eighth note: the dubstep
//   wobble, kept out of the sub's way, its fizz above 2.5 kHz filtered off.
// - A singing lead over the drop, on the chords' own notes where it lands on the beat, and a breakdown of the pad and
//   the lead alone before the drop comes back.
blank("dubstep");
title("Skyline Fracture");
tempo(140); key("F minor"); swing(0.5);
prog("A", "VI VII i v");
bars_per_chord(1);

// drums in steps, as the Drums tab shows them: X accent, x hit, o ghost, . rest, sixteen a bar
drum("kick", "X.........x.....");
drum("snare", "........X.......");
drum("hat", "x.x.x.x.x.x.xxxx");
mix("snare", #{ reverb: 0.3 });
mix("hat", #{ pan: 0.2, level: -2 });

track("chords", chord("prog").voicing(50, 74).s("supersaw").legato(1).duck(0.6).role("chords"));
rack("chords", [
  #{ sound: "warm_pad", band: [0, 300] },
  #{ sound: "supersaw", band: [300, 0], fx: [#{ fx: "width", width: 1.8, mono_below_hz: 300 }, #{ fx: "chorus", rate_hz: 0.4, depth_ms: 3, delay_ms: 12, mix: 0.4 }] },
  #{ sound: "pluck", band: [2500, 0], level: -8, transpose: 12, pan: 0.2 },
]);
track("bass", chord("prog").root().struct("x ~ ~ ~ ~ ~ ~ ~ x ~ ~ ~ ~ ~ x ~").s("wobble").legato(0.9).role("bass"));
rack("bass", [
  #{ sound: "sub_bass", band: [0, 100], duck: 0.7 },
  #{ sound: "wobble", band: [100, 2000], level: -1, fx: [
    #{ fx: "saturate", kind: "hard", drive_db: 10, mix: 0.8 },
    #{ fx: "filter", mode: "lp", cutoff: 2500, res: 0.2 },
  ] },
]);
// the lead on the chords' own notes where it lands on the beat: F over Db, Eb and G over Eb, F and Ab over Fm, C over Cm
track("lead", n("<[7@3 5] [6@2 8@2] [7@3 9] [11@4]>").octave(4).s("saw_lead").legato(1).room(0.4).delay(0.3).role("lead").lpf(4500));
track("pad", chord("prog").voicing(48, 72).s("pad").room(0.5).role("pad").level(-4));

form([
  sec("intro", 8, "chords pad").filter_in(),
  sec("build", 8, "chords hat pad").build().gap(),
  sec("drop", 16, "kick snare hat bass chords lead").impact(),
  sec("break", 16, "pad lead"),
  sec("build", 8, "kick hat chords").build().gap(),
  sec("drop", 32, "kick snare hat bass chords lead").impact(),
  sec("outro", 8, "chords pad"),
]);

// "Satellite Hymn": 2000s art rock, the brooding, cinematic guitar music at the top of the decade's charts
// (Radiohead, Muse, Coldplay). An original song in that style: none of its notes come from a record.
//
// How it is made:
// - A band's rock beat at 92 BPM: a kick that pushes into the bar, a snare a little late on 2 and 4 in a big room,
//   ghosted sixteenths on the hats, and a crash on every chorus.
// - i v VI III (Bm F#m G D): minor and melancholy, the minor v keeping it from ever resolving brightly.
// - A piano alone at the start and in the break, and under the verses: the chords struck and left to ring. A clean,
//   tidy guitar arpeggio here would tip it into stock "inspirational" music; the band only plays when it is loud.
// - The chorus is a wall: two distorted guitars playing power chords (the root and the fifth) in eighths, panned
//   hard left and right and saturated hard, over a bass pushing the roots in threes and twos.
// - A high melody on strings over the chorus, long notes leaving space, as a falsetto would.
blank("art_rock");
title("Satellite Hymn");
tempo(92); key("B minor"); swing(0.5);
prog("A", "i v VI III");
bars_per_chord(1);

// drums in steps, as the Drums tab shows them: X accent, x hit, o ghost, . rest, sixteen a bar
drum("kick", "X.....o.X....o..");
drum("snare", "....X.......X..o");
drum("hat", "XoxoXoxoXoxoXoxo");
lane("snare", #{ late: 7 });
mix("snare", #{ reverb: 0.35 });
mix("hat", #{ pan: 0.2, level: 2 });
bus("drums", [#{ fx: "saturate", kind: "tape", drive_db: 5, mix: 0.6 }]);

track("bass", chord("prog").root().struct("x ~ ~ x ~ ~ x ~").s("saw_bass").legato(0.8));
fx("bass", [#{ fx: "saturate", kind: "tube", drive_db: 6, mix: 0.6 }]);
track("piano", chord("prog").voicing(48, 72).struct("x ~ ~ ~ x ~ ~ ~").s("keys").legato(0.9).room(0.35).pan(-0.1).role("keys"));
// power chords: the root and the fifth, B F#, F# C#, G D, D A
let power = "<[0,4]*8 [4,8]*8 [5,9]*8 [2,6]*8>";
for side in [["dist", -0.45], ["dist2", 0.45]] {
  track(side[0], n(power).octave(2).s("supersaw").legato(0.9).room(0.15).pan(side[1]).role("chords").level(1.5));
  fx(side[0], [
    #{ fx: "eq", bands: [#{ kind: "highpass", freq: 85, q: 0.7 }, #{ kind: "peak", freq: 1700, gain_db: 3, q: 0.8 }] },
    #{ fx: "saturate", kind: "hard", drive_db: 24, mix: 1 },
  ]);
}
track("melody", n("<[4@3 5] [4@2 1@2] [2@3 4] [6@4]>").octave(4).s("strings").legato(1).room(0.35).delay(0.25).role("lead"));
fx("melody", [#{ fx: "saturate", kind: "tape", drive_db: 6, mix: 0.5 }]);

form([
  sec("intro", 8, "piano").filter_in(),
  sec("verse", 16, "kick snare hat bass piano"),
  sec("hook", 16, "kick snare hat bass dist dist2 melody").impact(),
  sec("verse", 16, "kick snare hat bass piano"),
  sec("hook", 16, "kick snare hat bass dist dist2 melody").impact(),
  sec("break", 8, "piano"),
  sec("hook", 16, "kick snare hat bass dist dist2 melody").impact(),
  sec("outro", 8, "piano"),
]);

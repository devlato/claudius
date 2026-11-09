// "In the End Study": a production study of Linkin Park's In the End (2000), the rap-rock single from Hybrid Theory.
// The record's tempo, key, chords, groove and sound, with an original melody, piano part and bass line: no note of
// the record's.
//
// How it is made:
// - 105 BPM, a rock beat with a hip hop swing: the kick and snare tight, the hats steady in eighths, a crash at the
//   top of every four bars of the chorus.
// - E-flat minor: the verse turns on i VI VII (Ebm Cb Db), and the chorus adds the relative major (Ebm Cb Db Gb), the
//   lift that makes it soar.
// - A piano opens the song alone, as a player would: chords held in the left hand and a plain, melancholic line in the
//   right, in quarters and halves; it runs under the rap of each verse and ends the song.
// - The chorus is a wall of distorted guitars, here a supersaw driven through a tube in power chords, roots and fifths,
//   chorused wide, with a picked bass doubling the roots in eighths and strings swelling behind.
// - Intro, verse, chorus, verse, chorus, a bridge where it all drops away to the piano, the last chorus twice as long,
//   and the piano alone to end.
blank("art_rock");
title("In the End Study");
tempo(105); key("Eb minor"); swing(0.52);
prog("A", "i VI VII VII");
prog("C", "i VI VII III");
bars_per_chord(1);

// drums in steps, as the Drums tab shows them: X accent, x hit, o ghost, . rest; sixteen a bar
drum("kick", "X.....x.X.x.....");
drum("snare", "....X.......X...");
drum("hat", "x.x.x.x.x.x.x.x.");
drum("crash", "main", "X...............|................|................|................", 16);
mix("crash", #{ level: -3 });
mix("hat", #{ level: 3 });
mix("snare", #{ level: 1 });

// the piano: chords held in the left hand, a line in the right
track("piano", chord("prog").voicing(46, 62).struct("x ~ ~ ~ ~ ~ x ~").legato(1.2).s("keys").room(0.45)
                 .role("keys"));
track("melody", n("<[7@2 6 4] [4@3 2] [3@2 4 5] [5@4]>").octave(4).s("keys").legato(0.9).room(0.45).role("keys"));
// the guitars: power chords, root and fifth, driven through a tube
track("guitars", chord("prog").root().transpose(12).struct("x*8").superimpose(|x| x.transpose(7))
                   .s("supersaw").legato(0.9).role("chords"));
fx("guitars", [
  #{ fx: "saturate", kind: "tube", drive_db: 16 },
  #{ fx: "filter", mode: "lp", cutoff: 3000, res: 0.15 },
  #{ fx: "chorus", rate_hz: 0.5, depth_ms: 3, delay_ms: 12, mix: 0.4 },
  #{ fx: "width", width: 1.6, mono_below_hz: 150 },
]);
track("bass", chord("prog").root().struct("x*8").s("pluck_bass").legato(0.7).role("bass"));
fx("bass", [#{ fx: "saturate", kind: "tube", drive_db: 6 }]);
track("strings", chord("prog").voicing(55, 76).s("strings").room(0.45).role("pad").level(-4));

form([
  sec("intro", 4, "piano melody"),
  sec("verse", 16, "kick snare hat bass piano melody"),
  sec("hook", 8, "kick snare hat crash bass guitars strings").prog("C").impact(),
  sec("verse", 16, "kick snare hat bass piano melody"),
  sec("hook", 8, "kick snare hat crash bass guitars strings").prog("C").impact(),
  sec("break", 8, "piano melody"),
  sec("hook", 16, "kick snare hat crash bass guitars strings").prog("C").impact(),
  sec("outro", 4, "piano melody"),
]);

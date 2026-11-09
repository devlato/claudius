// "Yonkers Study": a production study of Tyler, the Creator's Yonkers (2011), the stark, menacing beat from Goblin.
// The record's tempo, key, groove and sound, with an original melody, bass line and drums: no note of the record's.
//
// How it is made:
// - 79 BPM, straight and unhurried, the drums dry and hard: a booming kick, a snare cracked on 2 and 4 with a clap
//   under it, closed hats dragging a little behind the beat, and an open hat to end the bar.
// - E-flat minor with its raised seventh, round a loop that slips down a semitone and back (Ebm Dm Bbm Fm): the D
//   minor chord leans on the leading tone, which is what makes the loop lurch.
// - A synth bass held under each chord, the lowest and loudest thing in the mix: a clean sub under it and the same
//   notes driven through a tube over it, a rack of two.
// - An eerie lead that circles each chord's root a semitone away, and a cold bell an octave up that answers it: the
//   menace is in how little there is.
// - Verses all the way and no chorus, as on the record: the beat loops under a rap, and drops to the bass alone for a
//   phrase before each new verse.
blank("hip_hop");
title("Yonkers Study");
tempo(79); key("Eb harmonic minor"); swing(0.5);
prog("A", "Ebm Dm Bbm Fm");
bars_per_chord(1);

// drums in steps, as the Drums tab shows them: X accent, x hit, o ghost, . rest; sixteen a bar
drum("kick", "X.........X..x..");
drum("snare", "....X.......X...");
drum("clap", "....x.......x...");
drum("hat", "x.x.x.x.x.x.x.xo");
drum("ohat", "..............x.");
lane("hat", #{ late: 14 });
mix("snare", #{ reverb: 0.12 });
mix("clap", #{ level: -4 });
mix("hat", #{ pan: 0.2, level: -3 });
mix("ohat", #{ pan: -0.2, level: -4 });

// the bass: a whole note under each chord
track("bass", chord("prog").root().struct("x").s("moog_bass").legato(1).role("bass"));
rack("bass", [
  #{ sound: "sub_bass", band: [0, 100], duck: 0.5 },
  #{ sound: "moog_bass", band: [100, 0], level: -2, fx: [
    #{ fx: "saturate", kind: "tube", drive_db: 18 },
    #{ fx: "filter", mode: "lp", cutoff: 1400, res: 0.2 },
  ] },
]);
// the lead circles each root a semitone away; the bell answers on each chord's third
track("lead", note("<[eb4@3 d4 eb4@2 ~ bb3] [d4@3 eb4 d4@2 ~ f4] [bb3@3 cb4 bb3@2 ~ f3] [f4@3 gb4 f4@2 eb4 c4]>")
                .s("soft_lead").legato(0.9).room(0.3).delay(0.15).role("lead"));
track("bell", note("<[~ ~ ~ ~ gb5 ~ ~ ~] [~ ~ ~ ~ f5 ~ ~ ~] [~ ~ ~ ~ db5 ~ ~ ~] [~ ~ ~ ~ ab5 ~ ab5 ~]>")
                .s("bell").legato(0.5).room(0.5).level(-3).pan(0.3).role("lead"));

form([
  sec("intro", 4, "lead bell"),
  sec("verse", 16, "kick snare clap hat ohat bass lead bell"),
  sec("break", 4, "bass"),
  sec("verse", 16, "kick snare clap hat ohat bass lead"),
  sec("break", 4, "bass"),
  sec("verse", 16, "kick snare clap hat ohat bass lead bell"),
  sec("outro", 4, "bass lead bell"),
]);

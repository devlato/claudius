// "Moonlight Sonata": the first movement of Beethoven's Moonlight Sonata (Piano Sonata No. 14, Op. 27 No. 2, 1801) as
// dark, minimal hip hop. The music is in the public domain; this is an original arrangement of it, in the style of
// the dark hip hop of the 2010s.
//
// How it is made:
// - Beethoven's first four bars, note for note: the triplets rocking G sharp, C sharp and E over C sharp minor, the
//   bass stepping down to B, then A and D over F sharp, and the G sharp dominant to come home. A piano plays them, as
//   the right hand does, over the bass in octaves.
// - 79 BPM, the beat sparse and heavy: a kick on one and the "and" of three, a snare and a clap on two and four, the
//   hats laid back and an open hat at the end of the bar.
// - The bass is the sonata's own, in octaves, on a sub and a moog bass through a tube, filtered dark.
// - A soft pad holds the chords; the beat drops out twice, to the piano and the bass alone.
blank("hip_hop");
title("Moonlight Sonata");
tempo(79); key("C# harmonic minor"); swing(0.5);
prog("A", "C#m C#m A G#");
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

// Beethoven's triplets, twelve a bar: C sharp minor, over B, then A and D over F sharp, then the dominant G sharp
let triplets = "<[gs3 cs4 e4 gs3 cs4 e4 gs3 cs4 e4 gs3 cs4 e4] [gs3 cs4 e4 gs3 cs4 e4 gs3 cs4 e4 gs3 cs4 e4] [a3 cs4 e4 a3 cs4 e4 a3 d4 fs4 a3 d4 fs4] [gs3 c4 fs4 gs3 cs4 e4 gs3 cs4 ds4 fs3 c4 ds4]>";
track("piano", note(triplets).s("keys").legato(0.9).room(0.4).role("keys"));
fx("piano", [#{ fx: "width", width: 1.5, mono_below_hz: 200 }]);
// the sonata's bass, in octaves: C sharp, B, A and F sharp, G sharp
track("bass", note("<[cs2] [b1] [a1@2 fs1@2] [gs1]>").s("moog_bass").legato(1).role("bass"));
rack("bass", [
  #{ sound: "sub_bass", band: [0, 100], duck: 0.5 },
  #{ sound: "moog_bass", band: [100, 0], level: -2, fx: [
    #{ fx: "saturate", kind: "tube", drive_db: 18 },
    #{ fx: "filter", mode: "lp", cutoff: 1400, res: 0.2 },
  ] },
]);
track("pad", chord("prog").voicing(52, 68).s("warm_pad").room(0.5).level(-6).role("pad"));

form([
  sec("intro", 4, "piano"),
  sec("verse", 16, "kick snare clap hat ohat bass piano pad"),
  sec("break", 4, "piano bass"),
  sec("verse", 16, "kick snare clap hat ohat bass piano"),
  sec("break", 4, "piano bass"),
  sec("verse", 16, "kick snare clap hat ohat bass piano pad"),
  sec("outro", 4, "piano bass pad"),
]);

// "Bad Guy Study": a production study of Billie Eilish's bad guy (2019), the Finneas production that turned pop inside
// out with almost nothing in it. The record's tempo, key, chords, groove and sound, with an original melody, bass line
// and hook: no note of the record's.
//
// How it is made:
// - 135 BPM and nearly empty: a kick on every beat, round and layered, finger snaps on 2 and 4, and for most of the
//   song nothing else but the bass.
// - G minor with its raised seventh, i iv V7 (Gm Cm D7): the F-sharp in the D7 is what gives the line its sneer.
// - A fat, buzzing synth bass carries the song, a riff in eighths bouncing between the root and its octave, with a
//   deep 808 under it for the weight.
// - The hook is a cartoonish synth riff after each chorus, bright and staccato, over the beat and the bass.
// - The bridge breaks it: the same chords in half time, the kick distorted, a snare on 3, rolling hats and the 808
//   sliding, the way the record drops to 60 BPM for its end.
blank("hip_hop");
title("Bad Guy Study");
tempo(135); key("G harmonic minor"); swing(0.5);
prog("A", "Gm Gm Cm D7");
bars_per_chord(1);

// drums in steps, as the Drums tab shows them: X accent, x hit, o ghost, . rest; sixteen a bar
drum("kick", "X...X...X...X...");
drum("snap", "....X.......X...");
// the bridge, in half time: a kick on 1 and the and of 3, the snare on 3, hats rolling into each bar's end
drum("kick", "half", "X.........X.....");
drum("snare", "half", "........X.......");
drum("hat", "half", "x.x.x.x.x.x.xxxxx.x.x.x.x.x.x.x.x.x.x.x.x.x.xxxxxxxxx.x.x.x.x.x.", 32);
mix("snap", #{ reverb: 0.35, level: 2 });
mix("hat", #{ pan: 0.2 });

// the bass riff, root and octave in eighths, and the 808 an octave under its chords
track("bass", note("<[g1 ~ g2 ~ g1 g1 ~ bb1] [g1 ~ g2 ~ f2 ~ d2 ~] [c2 ~ c3 ~ c2 c2 ~ eb2] [d2 ~ d3 ~ fs2 ~ a1 ~]>")
                .s("dist_bass").legato(0.6).duck(0.3).role("bass"));
track("sub", chord("prog").root().transpose(-12).struct("x").s("808").legato(1).role("bass").level(-3));
track("hook", n("<[0 ~ 2 3 ~ 4 ~ 3] [2 ~ 0 ~ -1 ~ 0 ~] [3 ~ 5 7 ~ 5 ~ 3] [4 ~ 6 ~ 8 ~ 6 ~]>").octave(5)
                .s("pluck").legato(0.3).delay(0.15).role("lead"));
fx("hook", [
  #{ fx: "chorus", rate_hz: 0.8, depth_ms: 3, delay_ms: 10, mix: 0.5 },
  #{ fx: "width", width: 1.7, mono_below_hz: 200 },
]);
// the bridge's 808, sliding into each change
track("slide", note("<[g1@6 a1 bb1] [g1@8] [c2@6 bb1 a1] [d2@4 fs1@4]>").s("808").legato(1).role("bass"));

form([
  sec("intro", 4, "bass snap").energy(0.3),
  sec("verse", 16, "kick snap bass sub").energy(0.6),
  sec("build", 4, "kick snap bass").energy(0.55).build(),
  sec("hook", 8, "kick snap bass sub").energy(0.8).impact(),
  sec("hook", 8, "kick bass hook snap").name("post").energy(0.85),
  sec("verse", 16, "kick snap bass sub").energy(0.6),
  sec("build", 4, "kick snap bass").energy(0.55).build(),
  sec("hook", 8, "kick snap bass sub").energy(0.8).impact(),
  sec("hook", 8, "kick bass hook snap").name("post").energy(0.85),
  sec("groove", 16, "kick snare hat slide").name("bridge").variant("half").energy(0.9).impact(),
  sec("outro", 4, "slide").energy(0.3),
]);
// the bridge's kick, distorted
fx("kick", [#{ fx: "saturate", kind: "hard", drive_db: sections(0, #{ bridge: 16 }) }]);

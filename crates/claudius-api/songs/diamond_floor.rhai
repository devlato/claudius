// "Diamond Floor": 1990s piano house, the soulful club records that crossed into the charts in the early part of
// the decade (Robin S, Black Box, CeCe Peniston). An original song in that style.
//
// How it is made:
// - 124 BPM four on the floor, with an open hat on every offbeat and a shaker: the drum machines of the time.
// - Piano chords, the decade's signature: minor sevenths and ninths hammered on the offbeats, played on a bright
//   house piano.
// - An organ bass, the deep, round bass of the M1 organ preset, syncopated between the kicks.
// - A diva's chopped vocal as the hook, short soulful phrases in the choruses, where a singer would belt.
blank("house");
title("Diamond Floor");
tempo(124); key("A minor"); swing(0.52);
prog("A", "i7 iv9 VII7 IIImaj7");
bars_per_chord(1);

// drums in steps, as the Drums tab shows them: X accent, x hit, o ghost, . rest, sixteen a bar
drum("kick", "X...X...X...X...");
drum("clap", "....X.......X...");
drum("ohat", "..x...x...x...x.");
drum("shaker", "o.x.o.x.o.x.o.x.");
mix("clap", #{ reverb: 0.2 });
mix("ohat", #{ pan: -0.15 });
mix("shaker", #{ pan: 0.3 });

track("bass", chord("prog").root().struct("x ~ ~ x ~ ~ x ~").s("organ_bass").legato(0.6).duck(0.4));
track("piano", chord("prog").voicing(55, 76).struct("~ x ~ x ~ x ~ x").s("house_piano").legato(0.4).room(0.25).role("chords"));
track("pad", chord("prog").voicing(52, 70).s("strings").role("pad").room(0.35).level(-3));
fx("pad", [#{ fx: "width", width: 1.5 }]);
track("diva", n("<[~ ~ 4 4 6 ~ 4 ~] [7@2 6 4] [~ ~ 3 3 5 ~ 3 ~] [4@4]>")
                .octave(4).s("vox_lead").legato(0.8).delay(0.2).room(0.3).role("vocal"));

form([
  sec("intro", 8, "kick ohat shaker piano").filter_in(),
  sec("verse", 16, "kick clap ohat shaker bass piano"),
  sec("hook", 16, "kick clap ohat shaker bass piano pad diva").impact(),
  sec("break", 8, "piano pad"),
  sec("build", 8, "kick clap shaker bass piano pad").build(),
  sec("hook", 16, "kick clap ohat shaker bass piano pad diva").impact(),
  sec("hook", 16, "kick clap ohat shaker bass piano pad diva"),
  sec("outro", 8, "kick ohat shaker piano"),
]);

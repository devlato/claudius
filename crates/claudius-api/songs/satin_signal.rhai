// "Satin Signal": 2020s disco-pop, the disco revival of 2020 (Dua Lipa's Future Nostalgia era, Doja Cat, The
// Weeknd). An original song in that style.
//
// How it is made:
// - Four on the floor at 118 BPM with a clap on 2 and 4 and an open hat on every offbeat: the disco "tss" that
//   pulls the beat forward.
// - Minor sevenths that never resolve, i7 iv7 VII7 IIImaj7 (Dm7 Gm7 C7 Fmaj7): it loops for the whole song.
// - The bass in octaves on eighths, the engine of every disco record, here a synth one.
// - A muted, plucked chop on the sixteenth offbeats where the rhythm guitar would play, an electric piano holding
//   the chords, and strings stabbing the offbeats in the choruses only, so they lift.
// - A brassy synth hook instead of a singer, short phrases with rests between them, as a topline is written.
blank("nu_disco");
title("Satin Signal");
tempo(118); key("D minor"); swing(0.52);
prog("A", "i7 iv7 VII7 IIImaj7");
prog("B", "VImaj7 VImaj7 V7 V7");
bars_per_chord(1);

// drums in steps, as the Drums tab shows them: X accent, x hit, o ghost, . rest, sixteen a bar
drum("kick", "X...X...X...X...");
drum("clap", "....X.......X...");
drum("ohat", "..x...x...x...x.");
drum("hat", "oxoxoxoxoxoxoxox");
// congas, which put the band back in the beat: a disco record had a percussionist, a house track has a sampler
drum("perc", "..x..xx...x.xx..");
mix("clap", #{ reverb: 0.15 });
mix("ohat", #{ pan: -0.15, level: 1 });
mix("hat", #{ pan: 0.25, level: 2 });
mix("perc", #{ pan: 0.35 });

track("bass", note("<[d2 d3]*4 [g1 g2]*4 [c2 c3]*4 [f1 f2]*4>").s("pluck_bass").legato(0.55));
track("guitar", chord("prog").voicing(55, 70).struct("[~ x]*4").s("pluck").legato(0.25).pan(-0.35).role("chords"));
track("keys", chord("prog").voicing(52, 69).struct("x ~ ~ ~ x ~ ~ ~").s("epiano").legato(0.9).pan(0.25).role("keys"));
track("strings", chord("prog").voicing(62, 79).struct("~ x ~ x").s("strings").legato(0.3).room(0.25).pan(0.2).role("pad"));
track("lead", n("<[4 ~ 4 6 4 ~ 2 ~] [5 ~ 4 3 2 ~ 0 ~] [3 ~ 1 3 5 ~ 4 3] [4 ~ ~ 2 1 ~ ~ ~]>")
                .octave(4).s("square_lead").legato(0.85).room(0.25).delay(0.2));

form([
  sec("intro", 8, "kick hat perc guitar").filter_in(),
  sec("verse", 16, "kick clap hat perc bass guitar keys"),
  sec("build", 8, "kick clap hat perc bass guitar keys strings").build(),
  sec("hook", 16, "kick clap ohat hat perc bass guitar keys strings lead").impact(),
  sec("break", 8, "perc guitar keys strings").prog("B"),
  sec("build", 8, "kick clap hat perc bass guitar keys strings").build(),
  sec("hook", 16, "kick clap ohat hat perc bass guitar keys strings lead").impact(),
  sec("outro", 8, "kick hat perc bass guitar"),
]);

// "Reese Machine": neurofunk drum and bass, the technical, mechanical edge of drum and bass in the 2010s and 2020s
// (Noisia, Mefjus, Phace). An original song in that style, built to show layered sounds: its bass and its snare are
// each a rack of several.
//
// How it is made:
// - 174 BPM on the genre's two-step beat: a kick on 1 and the "and" of 3, the snare on 2 and 4, hats rolling.
// - The snare is three hits at once, as producers stack one: the snare itself for the body, a clap a little to the
//   right for the crack, and a burst of noise over 5 kHz with a short room for the air.
// - The bass is the whole track, a reese split into three bands, each treated on its own as neurofunk producers do:
// - the sub under 90 Hz, clean and ducked under the kick;
// - the reese's body from 90 Hz to 900 Hz through a phaser, the slow detuned movement the genre is built on;
// - its top over 900 Hz crushed and gated in sixteenths: the metallic, mechanical texture, without touching the sub.
// - Minor and dark, i iv VI v (Am Dm F Em), and a breakdown with only the pad before the drop returns.
blank("dnb");
title("Reese Machine");
tempo(174); key("A minor"); swing(0.5);
prog("A", "i iv VI v");
bars_per_chord(1);

// drums in steps, as the Drums tab shows them: X accent, x hit, o ghost, . rest, sixteen a bar
drum("kick", "X.........X.....");
drum("snare", "....X.......X...");
drum("hat", "x.x.x.x.x.x.x.xx");
mix("hat", #{ pan: 0.2, level: -2 });
rack("snare", [
  #{ sound: "sd" },
  #{ sound: "cp", level: -4, pan: 0.2 },
  #{ sound: "hh", band: [5000, 0], level: -10, fx: [#{ fx: "reverb", decay: 0.8, mix: 0.4 }] },
]);

track("bass", chord("prog").root().struct("x ~ ~ x ~ ~ x ~ ~ ~ x ~ x ~ ~ ~").s("reese").legato(0.9).role("bass"));
rack("bass", [
  #{ sound: "sub_bass", band: [0, 90], duck: 0.6 },
  #{ sound: "reese", band: [90, 900], fx: [#{ fx: "phaser", rate_hz: 0.25, stages: 6, depth: 0.8, feedback: 0.5, mix: 0.6 }] },
  #{ sound: "reese", band: [900, 0], level: -9, fx: [
    #{ fx: "bitcrush", bits: 6, downsample: 2, mix: 0.5 },
    #{ fx: "gate", steps: [1, 0, 1, 0, 1, 1, 0, 1], step_beats: 0.25, depth: 0.8, smooth_ms: 3 },
  ] },
]);
track("pad", chord("prog").voicing(52, 72).s("pad").room(0.5).role("pad").level(-3));
fx("pad", [#{ fx: "width", width: 1.6, mono_below_hz: 200 }]);
track("stab", chord("prog").voicing(57, 76).struct("~ ~ ~ ~ ~ ~ x ~ ~ ~ ~ ~ ~ ~ ~ ~").s("supersaw").legato(0.2).delay(0.3).room(0.3).role("lead"));

form([
  sec("intro", 16, "pad hat").filter_in(),
  sec("build", 8, "hat pad bass").build().gap(),
  sec("drop", 32, "kick snare hat bass stab").impact(),
  sec("break", 16, "pad"),
  sec("build", 8, "hat bass pad").build().gap(),
  sec("drop", 32, "kick snare hat bass stab").impact(),
  sec("outro", 16, "kick snare hat pad"),
]);

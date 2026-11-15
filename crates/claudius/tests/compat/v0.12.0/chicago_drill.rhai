// Glass Garden — chicago_drill (seed 7)
// song("chicago_drill", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(131); key("D harmonic_minor"); swing(0.5051635);
humanize(0.1);
prog("A", "i VI V V");
prog("B", "iv V i VI");
bars_per_chord(1);
master(#{ lufs: -8.5, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "alt", "X.........X.....|X......X........|X.........X.....|X......X........");
drum("kick", "main", "X......X..X.....|X..X............|X......X........|X.........X.....");
drum("snare", "main", "........X.......");
drum("clap", "main", "........X.......");
drum("hat", "alt", "x.....x.....x.....x.....x.....x.....x.....x.....|x.....x.....x.....x.....x.....x.....o.x.x.x.x.X.|x.....x.....x.....x.....x.....x.....x.....x.....|x.....x.....x.....x.....x.....x.....X.Xx.xx.xx.x", 48);
drum("hat", "main", "x.....x.....o.xx.xx.xx.Xx.....x.....x..x..x.....|o.xx.xx.xx.Xx.....x.....x.....x.....X.Xx.xx.xx.x|x.....x.....ooxxxxxxxxXXx.....x.....x..x..x.....|......o.xx.xx.....x.....x.....x.....ooxxxxxxxxXX", 48);
drum("ohat", "main", "................");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "drill", preset: "808", register: [24, 40], density: 0.5726087, seed: 2642474102756894640 });
part("keys", #{ style: "ostinato", preset: "keys", register: [55, 79], density: 0.4710492, seed: -6710496186308540014 });
part("brass", #{ style: "sustain", preset: "brass_stab", register: [48, 70], density: 0.54910505, seed: -3681901744659677821 });
part("lead", #{ style: "drill_melody", preset: "house_piano", register: [67, 88], density: 0.3056614, seed: 805921219908132358 });
part("strings", #{ style: "legato", preset: "vowel_pad", register: [48, 72], density: 0.5554895, seed: 6734273059743298459 });

form([
  sec("intro", 8, "keys brass").energy(0.5403019).variant("main"),
  sec("hook", 8, "*").energy(0.883739).impact(),
  sec("verse", 8, "kick snare hat bass keys brass").energy(0.6907866).variant("alt").gap(),
  sec("hook", 8, "*").name("hook2").energy(0.9211304).impact(),
  sec("verse", 8, "kick snare hat bass keys brass strings").name("verse2").energy(0.69666034).variant("alt").fills(true),
  sec("hook", 8, "*").name("hook3").energy(0.98874223).gap().impact(),
  sec("hook", 8, "*").name("hook4").energy(0.98664886),
  sec("outro", 4, "keys brass lead").energy(0.4076398).variant("main"),
]);

mix("snare", #{ reverb: 0.12 });
mix("clap", #{ level: -3, reverb: 0.15 });
mix("hat", #{ pan: 0.12 });
mix("ohat", #{ pan: -0.15 });
mix("keys", #{ reverb: 0.5127861, delay: 0.122124426 });
mix("brass", #{ level: -2, reverb: 0.24518426 });
mix("lead", #{ level: -4, reverb: 0.42610887, delay: 0.22258927 });
mix("strings", #{ level: -3, reverb: 0.52315444 });
mix("fx", #{ level: -2, reverb: 0.2 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [
  #{ fx: "saturate", drive_db: 7, kind: "tanh", mix: 1 },
  #{ fx: "filter", cutoff: 5000, drive: 1, mix: 1, mode: "lp", res: 0.1 },
  #{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 },
]);
fx("keys", [#{ fx: "chorus", delay_ms: 11, depth_ms: 2.5, mix: 0.4, rate_hz: 0.45 }]);
fx("brass", [#{ fx: "phaser", depth: 0.69981074, feedback: 0.37219805, mix: sections(0.2858641, #{ drop: 0.42805505, hook: 0.42805505 }), rate_hz: 0.14278676, stages: 6 }]);
fx("lead", [#{ fx: "flanger", delay_ms: 3.3502333, depth_ms: 2.713053, feedback: 0.49436772, mix: 0.4030139, rate_hz: 0.25133887 }]);
fx("strings", []);
bus("bass", [#{ fx: "saturate", drive_db: 4, kind: "tanh", mix: 0.5 }]);
bus("drums", [#{ fx: "compressor", attack_ms: 8, knee_db: 6, makeup_db: 1, mix: 1, ratio: 2.2, release_ms: 90, threshold_db: -10 }]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 140, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 24, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 20, knee_db: 6, makeup_db: 0, mix: 1, ratio: 2, release_ms: 140, threshold_db: -9 },
  #{ fx: "saturate", drive_db: 2, kind: "tanh", mix: 0.3 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.4176444, decay: 2.1580954, lowcut: 281.2672, mix: 1, predelay_ms: 20.852709, width: 1.400633 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.5, feedback: 0.25210696, mix: 1, pingpong: true } });

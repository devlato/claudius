// Glass Garden — uk_drill (seed 7)
// song("uk_drill", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(138); key("D minor"); swing(0.5051635);
humanize(0.12);
prog("A", "i iv V7 i");
prog("B", "i VI iv V");
bars_per_chord(2);
master(#{ lufs: -9, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X.........X.....|......X.........|X.........X.....|......X....X....");
drum("snare", "main", "........X.......|........X....o..|........X.......|........X....o..");
drum("hat", "main", "x........x........x.................XXXxxxxxxxxx|x........x..o.xx.xx.xx.XX.X.x.x.x.x.XXXxxxxxxxxx|x........x........x.................ooxxxxxxxxXX|x........x........x.....o.xx.xx.xx.Xx.....x.....", 48);
drum("ohat", "main", "................|............x...|................|............x...");
drum("perc", "main", "......x......x..");
drum_sound("perc", "rim");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "kick_follow", preset: "808", register: [28, 43], density: 0.5726087, seed: 2642474102756894640 });
part("keys", #{ style: "half", preset: "keys", register: [52, 76], density: 0.4710492, seed: -6710496186308540014 });
part("lead", #{ style: "motif", preset: "vox_lead", register: [64, 88], density: 0.399105, seed: -3681901744659677821 });
part("pad", #{ style: "sustain", preset: "vowel_pad", register: [48, 72], density: 0.47421524, seed: 805921219908132358 });

form([
  sec("intro", 8, "keys pad").energy(0.4903019).variant("main"),
  sec("hook", 8, "*").energy(0.86373895).impact(),
  sec("verse", 8, "kick snare hat perc bass keys").energy(0.6907866).gap(),
  sec("hook", 8, "*").name("hook2").energy(0.8711304).impact(),
  sec("verse", 8, "kick snare hat perc bass keys pad").name("verse2").energy(0.7266603).fills(true).gap(),
  sec("hook", 8, "*").name("hook3").energy(0.9387422).gap().impact(),
  sec("hook", 8, "*").name("hook4").energy(0.98664886),
  sec("outro", 4, "keys pad lead").energy(0.3576398).variant("main"),
]);

mix("snare", #{ reverb: 0.1 });
mix("hat", #{ pan: 0.15 });
mix("ohat", #{ pan: -0.15 });
mix("perc", #{ pan: -0.25, reverb: 0.15 });
mix("keys", #{ reverb: 0.5127861, delay: 0.122124426 });
mix("lead", #{ reverb: 0.24518426, delay: 0.38817757 });
mix("pad", #{ reverb: 0.4869816 });
mix("fx", #{ level: -2, reverb: 0.2 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [#{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 }]);
fx("keys", [
  #{ fx: "width", mono_below_hz: 200, width: 1.4 },
  #{ fx: "chorus", delay_ms: 11, depth_ms: 2.5, mix: 0.4, rate_hz: 0.45 },
]);
fx("lead", [#{ fx: "saturate", drive_db: 5.1466026, kind: "tube", mix: 0.32567206 }]);
fx("pad", [#{ fx: "width", mono_below_hz: 200, width: 1.5 }]);
bus("bass", [#{ fx: "saturate", drive_db: 4, kind: "tanh", mix: 0.5 }]);
bus("drums", [#{ fx: "compressor", attack_ms: 6, knee_db: 6, makeup_db: 1, mix: 1, ratio: 2.5, release_ms: 80, threshold_db: -10 }]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 150, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 24, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 25, knee_db: 6, makeup_db: 0, mix: 1, ratio: 1.8, release_ms: 150, threshold_db: -9 },
  #{ fx: "saturate", drive_db: 2, kind: "tanh", mix: 0.3 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.4176444, decay: 2.373905, lowcut: 281.2672, mix: 1, predelay_ms: 20.852709, width: 1.5 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.5, feedback: 0.25210696, mix: 1, pingpong: true } });

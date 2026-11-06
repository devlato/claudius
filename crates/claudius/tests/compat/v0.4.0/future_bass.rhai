// Glass Garden — future_bass (seed 7)
// song("future_bass", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(141); key("D major"); swing(0.5051635);
humanize(0.1);
prog("A", "I bVII IV I");
prog("B", "IV I V vi");
bars_per_chord(1);
master(#{ lufs: -9, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X.........X.....|X.........X.....|X.........X.....|X...............");
drum("snare", "main", "........X.......");
drum("hat", "main", "x.....x..x..x.....x.....x..x..x.....x.....x..x..|x.....x..x..x.....x.....x..x..x.....X.Xx.xx.xx.x|x.....x..x..X.X.x.x.x.x.x..x..x.....x.....x..x..|x.....x..x..x.....x.....x..x..x.....x.....x..x..", 48);
drum("ohat", "main", "..........x.....");
drum("perc", "main", "..x.....x.....x.");
drum("perc", "sparse", "..x.............");
drum_sound("perc", "snap");

// parts written by the theory engine: change the style, preset, register, density or seed
part("chords", #{ style: "pump", preset: "supersaw", register: [52, 79], density: 0.42864978, seed: -37300132614189792 });
part("bass", #{ style: "sustain", preset: "sub_bass", register: [33, 48], density: 0.55524325, seed: 6553130387488681005 });
part("lead", #{ style: "motif", preset: "pluck", register: [67, 91], density: 0.5065276, seed: -4694233468511451402 });
part("pad", #{ style: "sustain", preset: "solina", register: [50, 76], density: 0.5752324, seed: 8524255199291047346 });

form([
  sec("intro", 8, "pad lead").energy(0.4903019).variant("main"),
  sec("verse", 8, "kick snare hat bass pad lead").energy(0.513739).fills(true),
  sec("build", 4, "snare hat chords lead").energy(0.6407866).gap(),
  sec("drop", 8, "kick snare hat ohat perc bass chords lead").energy(0.97113043).impact(),
  sec("break", 4, "pad lead arp").energy(0.37666035),
  sec("build", 4, "snare chords arp").name("build2").energy(0.6387422).prog("B").gap(),
  sec("drop", 12, "kick snare hat ohat perc bass chords lead arp").name("drop2").energy(0.98664886).impact(),
  sec("drop", 12, "kick snare hat ohat perc bass chords lead arp").name("drop3").energy(1),
  sec("outro", 8, "pad chords").energy(0.30374148).variant("main"),
]);

mix("snare", #{ reverb: 0.25 });
mix("hat", #{ pan: 0.15 });
mix("ohat", #{ pan: -0.15 });
mix("perc", #{ pan: 0.4, reverb: 0.3 });
mix("chords", #{ reverb: 0.22838272, delay: 0.07441711, duck: 0.85 });
mix("bass", #{ duck: 0.6 });
mix("lead", #{ reverb: 0.24518426, delay: 0.31054208, duck: 0.3 });
mix("pad", #{ reverb: 0.4869816, duck: 0.5 });
mix("fx", #{ reverb: 0.3 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("chords", [#{ fx: "auto_pan", beats: 2, depth: 0.51514673 }]);
fx("lead", [#{ fx: "tape", drive_db: 1.1752481, flutter: 0.19840398, lowpass_hz: 10785.415, wow: 0.12320626 }]);
fx("pad", [#{ fx: "chorus", delay_ms: 8.684588, depth_ms: 2.5733013, mix: 0.37727153, rate_hz: 0.35640424 }]);
bus("drums", [#{ fx: "compressor", attack_ms: 10, knee_db: 6, makeup_db: 1, mix: 1, ratio: 2.2, release_ms: 100, threshold_db: -11 }]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 130, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 26, gain_db: 0, kind: "highpass", q: 0.7 }, #{ freq: 10000, gain_db: 1.5, kind: "highshelf", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 25, knee_db: 6, makeup_db: 0, mix: 1, ratio: 1.8, release_ms: 150, threshold_db: -9 },
  #{ fx: "saturate", drive_db: 2, kind: "tanh", mix: 0.3 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.31764442, decay: 2.5897145, lowcut: 281.2672, mix: 1, predelay_ms: 25.852709, width: 1.4589927 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.45, feedback: 0.29412478, mix: 1, pingpong: true } });

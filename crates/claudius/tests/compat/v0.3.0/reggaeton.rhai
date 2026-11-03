// Glass Garden — reggaeton (seed 7)
// song("reggaeton", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(89); key("D minor"); swing(0.5051635);
humanize(0.12);
prog("A", "i iv V7 i");
prog("B", "i VI iv V");
bars_per_chord(1);
master(#{ lufs: -9, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X...X...X...X...");
drum("snare", "main", "...X..X....X..X.");
drum("hat", "main", "x...x...x...x...x...x.......Xxxx|x...x...x...x...x...x...x...x...|x...x...x...x...x...x...x...x...|x...x...x...x...oxxxxxxXx...x...", 32);
drum("rim", "main", "..x.......x....x");
drum("shaker", "main", "o....o....oooooo");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "kick_follow", preset: "deep_bass", register: [28, 45], density: 0.42864978, seed: -37300132614189792 });
part("chords", #{ style: "sustain", preset: "keys", register: [55, 76], density: 0.5272447, seed: 5396775171407160694 });
part("lead", #{ style: "motif", preset: "vox_lead", register: [62, 86], density: 0.52791286, seed: -7892568474930094457 });

form([
  sec("intro", 8, "chords pad").energy(0.4903019).variant("main"),
  sec("verse", 8, "kick snare hat rim bass chords").energy(0.66373897),
  sec("hook", 8, "kick snare hat rim shaker bass chords lead pad").energy(0.8907866).impact(),
  sec("verse", 8, "kick snare hat rim bass chords pad").name("verse2").energy(0.72113043).fills(true).gap(),
  sec("hook", 8, "kick snare hat rim shaker bass chords lead pad").name("hook2").energy(0.9266603).impact(),
  sec("break", 4, "chords lead hat").energy(0.3887422).build(true),
  sec("hook", 4, "kick snare hat rim shaker bass chords lead pad").name("hook3").energy(0.98664886).gap().impact(),
  sec("hook", 4, "kick snare hat rim shaker bass chords lead pad").name("hook4").energy(1),
  sec("outro", 8, "chords pad hat").energy(0.30374148).variant("main"),
]);

mix("snare", #{ reverb: 0.1 });
mix("hat", #{ pan: 0.15 });
mix("rim", #{ pan: -0.3, reverb: 0.1 });
mix("shaker", #{ pan: 0.3 });
mix("bass", #{ duck: 0.3 });
mix("chords", #{ reverb: 0.3662758, delay: 0.122124426, duck: 0.3 });
mix("lead", #{ reverb: 0.24518426, delay: 0.38817757, duck: 0.2 });
mix("fx", #{ level: -3, reverb: 0.25 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [#{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 }]);
fx("chords", [
  #{ fx: "width", mono_below_hz: 200, width: 1.3 },
  #{ fx: "chorus", delay_ms: 11, depth_ms: 2.5, mix: 0.4, rate_hz: 0.45 },
]);
fx("lead", [#{ fx: "saturate", drive_db: 5.1466026, kind: "tube", mix: 0.32567206 }]);
bus("bass", [#{ fx: "saturate", drive_db: 3, kind: "tanh", mix: 0.4 }]);
bus("drums", [#{ fx: "compressor", attack_ms: 6, knee_db: 6, makeup_db: 1.5, mix: 1, ratio: 2.5, release_ms: 80, threshold_db: -10 }]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 140, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 26, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 20, knee_db: 6, makeup_db: 0, mix: 1, ratio: 2, release_ms: 140, threshold_db: -9 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.4176444, decay: 1.9422858, lowcut: 281.2672, mix: 1, predelay_ms: 20.852709, width: 1.400633 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.5, feedback: 0.25210696, mix: 1, pingpong: true } });

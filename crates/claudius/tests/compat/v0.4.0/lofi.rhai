// Glass Garden — lofi (seed 7)
// song("lofi", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(71); key("D major"); swing(0.5720481);
humanize(0.6);
prog("A", "vi7 ii7 V7 Imaj7");
prog("B", "Imaj7 vi7 ii7 V7");
bars_per_chord(1);
master(#{ lufs: -12.5, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X......X..X.....|X......X..X.....|X......X..X.....|X.........X.....");
drum("snare", "main", "....X.......X...");
drum("hat", "main", "x.x.x...x.x.x.xx");
drum("shaker", "main", "..oooooo.oo...oo");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "walking", preset: "organ_bass", register: [33, 50], density: 0.5995956, seed: 3796763344308755361 });
part("keys", #{ style: "sustain", preset: "keys", register: [52, 74], density: 0.45851198, seed: 6674420348178494801 });
part("lead", #{ style: "motif", preset: "fm_kalimba", register: [64, 84], density: 0.3823303, seed: -7892568474930094457 });

form([
  sec("intro", 8, "keys").energy(0.44030192).variant("main"),
  sec("verse", 8, "kick snare hat bass keys").name("a").energy(0.563739),
  sec("hook", 8, "*").name("b").energy(0.7407866).gap(),
  sec("hook", 8, "*").name("b2").energy(0.97113043),
  sec("verse", 8, "kick snare hat bass keys pad").name("a2").energy(0.6266603).fills(true),
  sec("outro", 4, "keys pad lead").energy(0.2887422).variant("main"),
]);

mix("snare", #{ reverb: 0.1 });
mix("hat", #{ pan: 0.2 });
mix("shaker", #{ pan: -0.3 });
mix("keys", #{ reverb: 0.3662758 });
mix("lead", #{ reverb: 0.2860483, delay: 0.31054208 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [#{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 }]);
fx("keys", [
  #{ fx: "tape", drive_db: 2, flutter: 0.6, lowpass_hz: 7000, wow: 0.8 },
  #{ fx: "chorus", delay_ms: 11, depth_ms: 2.5, mix: 0.4, rate_hz: 0.45 },
]);
fx("lead", [#{ fx: "saturate", drive_db: 5.1466026, kind: "tube", mix: 0.32567206 }]);
bus("drums", [
  #{ fx: "bitcrush", bits: 11, downsample: 2, mix: 0.35 },
  #{ fx: "filter", cutoff: 8500, drive: 1, mix: 1, mode: "lp", res: 0.05 },
  #{ fx: "compressor", attack_ms: 15, knee_db: 6, makeup_db: 2, mix: 1, ratio: 2.5, release_ms: 120, threshold_db: -14 },
]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 90, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 30, gain_db: 0, kind: "highpass", q: 0.7 }, #{ freq: 8000, gain_db: -2.5, kind: "highshelf", q: 0.7 }] },
  #{ fx: "tape", drive_db: 3, flutter: 0.4, lowpass_hz: 11000, wow: 0.5 },
  #{ fx: "vinyl", crackle: 0.35, hiss_db: -42 },
  #{ fx: "compressor", attack_ms: 30, knee_db: 6, makeup_db: 0, mix: 1, ratio: 1.6, release_ms: 200, threshold_db: -14 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.6176444, decay: 1.7264763, lowcut: 234.38934, mix: 1, predelay_ms: 15.852709, width: 1.1671941 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.7, feedback: 0.25210696, mix: 1, pingpong: true } });

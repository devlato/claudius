// Glass Garden — pop_punk (seed 7)
// song("pop_punk", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(152); key("D major"); swing(0.50172114);
humanize(0.35);
prog("A", "I bVII IV I");
prog("B", "IV I V vi");
bars_per_chord(1);
master(#{ lufs: -9, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "half", "X...............");
drum("kick", "main", "X.....X.X.X.....");
drum("kick", "skate", "X...X...X...X...");
drum("kick", "verse", "X.......X.......");
lane("kick", #{ late: -1 });
drum("snare", "half", "........X.......");
drum("snare", "main", "....X.......X...");
drum("snare", "skate", "..X...X...X...X.");
drum("snare", "verse", "....X.......X...");
lane("snare", #{ late: 2 });
drum("hat", "half", "x...x...x...x...");
drum("hat", "main", "x.x.x.x.x.x.x.x.");
lane("hat", #{ late: 2 });
drum("ride", "main", "x.x.x.x.x.x.x.x.");
lane("ride", #{ late: -2 });
drum("crash", "main", "X...............|................|X...............|................");
drum("tom", "main", "........XXXXXXXX");

// parts written by the theory engine: change the style, preset, register, density or seed
part("mute_l", #{ style: "palm", preset: "supersaw", register: [40, 59], density: 0.42864978, seed: -37300132614189792 });
part("gtr_l", #{ style: "pump", preset: "supersaw", register: [40, 64], density: 0.56008077, seed: -1629742732023437997 });
part("bass", #{ style: "drive", preset: "saw_bass", register: [28, 47], density: 0.5554895, seed: 6734273059743298459 });
part("hook", #{ style: "rock_lead", preset: "square_lead", register: [62, 79], density: 0.51895356, seed: 5366676654615143931 });
part("mute_r", #{ style: "palm", preset: "supersaw", register: [40, 59], density: 0.5272447, seed: 5396775171407160694, doubles: "mute_l", late: 7 });
part("gtr_r", #{ style: "pump", preset: "supersaw", register: [40, 64], density: 0.47421524, seed: 805921219908132358, doubles: "gtr_l", late: 5 });

form([
  sec("intro", 8, "kick snare hat crash bass gtr_l gtr_r").energy(0.7903019).variant("main").fills(true).filter_in(),
  sec("verse", 8, "kick snare hat bass mute_l mute_r").energy(0.61373895).variant("verse"),
  sec("build", 4, "kick snare hat tom bass gtr_l gtr_r").name("pre").energy(0.7907866).prog("B").fills(true),
  sec("hook", 8, "kick snare ride crash bass gtr_l gtr_r hook").name("chorus").energy(0.97113043).impact(),
  sec("verse", 8, "kick snare hat bass mute_l mute_r").name("verse2").energy(0.6766603).variant("verse").fills(true),
  sec("hook", 8, "kick snare ride crash bass gtr_l gtr_r hook").name("chorus2").energy(0.98874223).impact(),
  sec("break", 8, "kick snare hat bass gtr_l").name("bridge").energy(0.53664887).variant("half").gap(),
  sec("hook", 4, "kick snare ride crash bass gtr_l gtr_r hook").name("chorus3").energy(1).variant("skate").gap().impact(),
  sec("hook", 4, "kick snare ride crash bass gtr_l gtr_r hook").name("chorus4").energy(0.9537415).variant("skate"),
  sec("outro", 8, "kick snare crash bass gtr_l gtr_r").energy(0.9116369).variant("main"),
]);

mix("snare", #{ reverb: 0.2 });
mix("hat", #{ pan: 0.25 });
mix("ride", #{ pan: -0.3 });
mix("crash", #{ pan: 0.35, reverb: 0.15 });
mix("tom", #{ pan: -0.2, reverb: 0.25 });
mix("mute_l", #{ pan: -0.8, reverb: 0.045676544 });
mix("mute_r", #{ pan: 0.8, reverb: 0.07325516 });
mix("gtr_l", #{ level: 1, pan: -0.85, reverb: 0.065382466 });
mix("gtr_r", #{ level: 1, pan: 0.85, reverb: 0.097396314 });
mix("hook", #{ level: -2, pan: 0.1, reverb: 0.27926657, delay: 0.12549901 });
mix("fx", #{ reverb: 0.2 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("mute_l", [
  #{ fx: "eq", bands: [#{ freq: 90, gain_db: 0, kind: "highpass", q: 0.7 }, #{ freq: 1800, gain_db: 2, kind: "peak", q: 0.8 }, #{ freq: 5500, gain_db: 0, kind: "lowpass", q: 0.7 }] },
  #{ fx: "auto_pan", beats: 2, depth: 0.51514673 },
]);
fx("mute_r", [#{ fx: "eq", bands: [#{ freq: 90, gain_db: 0, kind: "highpass", q: 0.7 }, #{ freq: 1600, gain_db: 2, kind: "peak", q: 0.8 }, #{ freq: 5200, gain_db: 0, kind: "lowpass", q: 0.7 }] }]);
fx("gtr_l", [
  #{ fx: "eq", bands: [#{ freq: 100, gain_db: 0, kind: "highpass", q: 0.7 }, #{ freq: 2000, gain_db: 2.5, kind: "peak", q: 0.8 }, #{ freq: 6000, gain_db: 0, kind: "lowpass", q: 0.7 }] },
  #{ fx: "chorus", delay_ms: 8.467328, depth_ms: 3.3550498, mix: 0.33927074, rate_hz: 0.24641252 },
]);
fx("gtr_r", [
  #{ fx: "eq", bands: [#{ freq: 100, gain_db: 0, kind: "highpass", q: 0.7 }, #{ freq: 1700, gain_db: 2.5, kind: "peak", q: 0.8 }, #{ freq: 5600, gain_db: 0, kind: "lowpass", q: 0.7 }] },
  #{ fx: "chorus", delay_ms: 8.684588, depth_ms: 2.5733013, mix: 0.37727153, rate_hz: 0.35640424 },
]);
fx("bass", [#{ fx: "saturate", drive_db: 6, kind: "tube", mix: 0.5 }]);
bus("bass", [
  #{ fx: "width", mono_below_hz: 150, width: 1 },
  #{ fx: "compressor", attack_ms: 8, knee_db: 6, makeup_db: 0, mix: 1, ratio: 3.5, release_ms: 80, threshold_db: -16 },
]);
bus("drums", [
  #{ fx: "width", mono_below_hz: 150, width: 1 },
  #{ fx: "parallel", chains: [#{ amount: 0.35, fx: [#{ fx: "compressor", attack_ms: 2, knee_db: 2, makeup_db: 0, mix: 1, ratio: 8, release_ms: 60, threshold_db: -28 }, #{ fx: "saturate", drive_db: 6, kind: "tape", mix: 1 }] }], dry: 1 },
  #{ fx: "compressor", attack_ms: 10, knee_db: 6, makeup_db: 1, mix: 1, ratio: 2.5, release_ms: 90, threshold_db: -14 },
]);
bus("music", [#{ fx: "width", mono_below_hz: 150, width: 1 }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 32, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 15, knee_db: 6, makeup_db: 0, mix: 1, ratio: 2.2, release_ms: 140, threshold_db: -10 },
  #{ fx: "saturate", drive_db: 1.5, kind: "tape", mix: 0.5 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.5176444, decay: 1.4027619, lowcut: 281.2672, mix: 1, predelay_ms: 15.852709, width: 1.1671941 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.6, feedback: 0.21008913, mix: 1, pingpong: true } });

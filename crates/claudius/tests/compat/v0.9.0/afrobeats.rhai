// Glass Garden — afrobeats (seed 7)
// song("afrobeats", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(101); key("D major"); swing(0.5303269);
humanize(0.3);
prog("A", "I bVII IV I");
prog("B", "IV I V vi");
bars_per_chord(1);
master(#{ lufs: -9.5, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X......XX.......|X......XX.......|X......XX.......|X......XX.....X.");
drum("clap", "main", "....X.......X...");
lane("clap", #{ late: 7 });
drum("rim", "main", "x..x..x...x.x...");
lane("rim", #{ late: 7 });
drum("hat", "main", "..x...x...x...x.");
lane("hat", #{ late: -3 });
drum("shaker", "main", "ooo..ooo.oooo.oo");
lane("shaker", #{ late: -7 });
drum_sound("conga", "perc");
drum("conga", "main", "..x..x....x.....");
lane("conga", #{ late: -1 });

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "afro", preset: "rubber_bass", register: [31, 48], density: 0.5726087, seed: 2642474102756894640 });
part("guitar", #{ style: "highlife", preset: "pluck", register: [55, 76], density: 0.4710492, seed: -6710496186308540014 });
part("lead", #{ style: "afro_flute", preset: "vox_lead", register: [62, 84], density: 0.44910502, seed: -3681901744659677821 });
part("pad", #{ style: "sustain", preset: "air_pad", register: [50, 74], density: 0.47421524, seed: 805921219908132358 });

form([
  sec("intro", 8, "shaker conga guitar").energy(0.5403019).variant("main"),
  sec("verse", 8, "kick clap rim shaker conga bass guitar keys").energy(0.66373897),
  sec("hook", 8, "kick clap rim hat shaker conga bass guitar keys lead pad").energy(0.8907866).impact(),
  sec("verse", 8, "kick clap rim shaker conga bass guitar keys pad").name("verse2").energy(0.72113043).fills(true).gap(),
  sec("hook", 8, "kick clap rim hat shaker conga bass guitar keys lead pad").name("hook2").energy(0.9266603).impact(),
  sec("break", 4, "shaker guitar lead").energy(0.3887422).build(true),
  sec("hook", 4, "kick clap rim hat shaker conga bass guitar keys lead pad").name("hook3").energy(0.98664886).gap().impact(),
  sec("hook", 4, "kick clap rim hat shaker conga bass guitar keys lead pad").name("hook4").energy(1),
  sec("outro", 8, "shaker conga guitar").energy(0.3537415).variant("main"),
]);

mix("clap", #{ reverb: 0.15 });
mix("rim", #{ pan: 0.3, reverb: 0.1 });
mix("hat", #{ pan: 0.15 });
mix("shaker", #{ pan: -0.3 });
mix("conga", #{ pan: -0.2, reverb: 0.1 });
mix("bass", #{ duck: 0.3 });
mix("guitar", #{ pan: 0.2, reverb: 0.29302064, delay: 0.18318664 });
mix("lead", #{ reverb: 0.24518426, delay: 0.31054208 });
mix("pad", #{ reverb: 0.4869816 });
mix("fx", #{ level: -3, reverb: 0.25 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [#{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 }]);
fx("guitar", [
  #{ fx: "width", mono_below_hz: 200, width: 1.4 },
  #{ fx: "chorus", delay_ms: 10.380553, depth_ms: 1.6460402, mix: 0.26222828, rate_hz: 0.71941394 },
]);
fx("lead", [#{ fx: "saturate", drive_db: 5.1466026, kind: "tube", mix: 0.32567206 }]);
fx("pad", [#{ fx: "width", mono_below_hz: 200, width: 1.4 }]);
bus("bass", [#{ fx: "saturate", drive_db: 2, kind: "tanh", mix: 0.3 }]);
bus("drums", [#{ fx: "compressor", attack_ms: 8, knee_db: 6, makeup_db: 1.5, mix: 1, ratio: 2.5, release_ms: 90, threshold_db: -11 }]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 130, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 28, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 20, knee_db: 6, makeup_db: 0, mix: 1, ratio: 2, release_ms: 150, threshold_db: -10 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.4176444, decay: 1.9422858, lowcut: 281.2672, mix: 1, predelay_ms: 20.852709, width: 1.400633 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.5, feedback: 0.25210696, mix: 1, pingpong: true } });

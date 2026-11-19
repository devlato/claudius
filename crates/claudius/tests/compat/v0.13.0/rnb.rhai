// Glass Garden — rnb (seed 7)
// song("rnb", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(65); key("D major"); swing(0.53376925);
humanize(0.3);
prog("A", "vi7 ii7 V7 Imaj7");
prog("B", "Imaj7 vi7 ii7 V7");
bars_per_chord(1);
master(#{ lufs: -10, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X.........X.....|..X.............|X...............|..X........X....");
lane("kick", #{ late: -1 });
drum("snare", "main", "....X.......X...");
lane("snare", #{ late: 12 });
drum("clap", "main", "....x.......x...");
lane("clap", #{ late: 16 });
drum("hat", "alt", "x...x...x...x...x...x...x...x...|x...x...x...x...x...x...x...x...|x...x.......Xxxxx...x...x...x...|x...x...x...x...x...x...x...x...", 32);
drum("hat", "main", "x...x.......Xxxxx...x...x...x...|x...x.......oxxxoxxxxxxXx...x...|x...x.......oxxxx...x...x...x...|x...x...x.x.x...oxxxxxxXoxxxxxxX", 32);
lane("hat", #{ late: 2 });
drum("ohat", "main", "..............x.");
drum("perc", "main", "....x.....x...x.");
drum("perc", "sparse", "..............x.");
drum_sound("perc", "snap");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "neo_soul", preset: "deep_bass", register: [26, 43], density: 0.42864978, seed: -37300132614189792, late: 9 });
part("keys", #{ style: "rnb_keys", preset: "house_piano", register: [50, 76], density: 0.5272447, seed: 5396775171407160694 });
part("pad", #{ style: "sustain", preset: "air_pad", register: [48, 74], density: 0.56008077, seed: -1629742732023437997 });
part("lead", #{ style: "rnb_runs", preset: "pluck", register: [60, 82], density: 0.3242152, seed: 805921219908132358 });

form([
  sec("intro", 8, "keys pad").energy(0.4903019).variant("main"),
  sec("verse", 8, "kick snare hat perc bass keys").energy(0.563739),
  sec("hook", 4, "*").energy(0.84078664),
  sec("verse", 4, "kick snare hat perc bass keys pad").name("verse2").energy(0.6211304).variant("alt").fills(true).gap(),
  sec("hook", 4, "*").name("hook2").energy(0.8766603),
  sec("break", 4, "keys pad lead").name("bridge").energy(0.4387422).gap(),
  sec("hook", 4, "*").name("hook3").energy(0.93664885).gap(),
  sec("hook", 4, "*").name("hook4").energy(1),
  sec("outro", 4, "keys pad").energy(0.2537415).variant("main"),
]);

mix("snare", #{ reverb: 0.2 });
mix("clap", #{ level: -6, reverb: 0.25 });
mix("hat", #{ pan: 0.18 });
mix("ohat", #{ pan: -0.15 });
mix("perc", #{ pan: -0.35, reverb: 0.3 });
mix("keys", #{ reverb: 0.43953097 });
mix("pad", #{ reverb: 0.36777636 });
mix("lead", #{ reverb: 0.3652362, delay: 0.22258927 });
mix("fx", #{ reverb: 0.35 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [
  #{ fx: "saturate", drive_db: 3, kind: "tanh", mix: 0.6 },
  #{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 },
]);
fx("keys", [
  #{ fx: "chorus", delay_ms: 10, depth_ms: 2, mix: 0.25, rate_hz: 0.5 },
  #{ fx: "width", mono_below_hz: 200, width: 1.3 },
]);
fx("pad", [#{ fx: "phaser", depth: 0.69981074, feedback: 0.37219805, mix: sections(0.2858641, #{ drop: 0.42805505, hook: 0.42805505 }), rate_hz: 0.14278676, stages: 6 }]);
fx("lead", [#{ fx: "flanger", delay_ms: 3.3502333, depth_ms: 2.713053, feedback: 0.49436772, mix: 0.4030139, rate_hz: 0.25133887 }]);
bus("bass", [#{ fx: "saturate", drive_db: 2, kind: "tanh", mix: 0.4 }]);
bus("drums", [#{ fx: "compressor", attack_ms: 10, knee_db: 6, makeup_db: 1, mix: 1, ratio: 2.5, release_ms: 100, threshold_db: -14 }]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 100, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 25, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 20, knee_db: 6, makeup_db: 0, mix: 1, ratio: 1.8, release_ms: 160, threshold_db: -11 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.4176444, decay: 1.9422858, lowcut: 234.38934, mix: 1, predelay_ms: 25.852709, width: 1.5 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.6, feedback: 0.29412478, mix: 1, pingpong: true } });

// Glass Garden — trip_hop (seed 7)
// song("trip_hop", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(79); key("D minor"); swing(0.51860577);
humanize(0.45);
prog("A", "i iv V7 i");
prog("B", "i VI iv V");
bars_per_chord(2);
master(#{ lufs: -11, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X.........X.....|X..X......X.....|X.........X.....|X..X......X.....");
drum("kick", "sparse", "x...............");
lane("kick", #{ late: -2 });
drum("snare", "main", "....X.o.....X...|....X.......X...|....X.o.....X...|....X.......X...");
lane("snare", #{ late: 19 });
drum("hat", "main", "....x...x.x.x.x.");
drum("hat", "sparse", "..o...o...o...o.");
lane("hat", #{ late: 2 });
drum("ohat", "main", "..............x.");
lane("ohat", #{ late: 1 });
drum("perc", "main", "...........x....");
lane("perc", #{ late: 4 });
drum_sound("perc", "rim");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "trip_hop", preset: "saw_bass", register: [28, 46], density: 0.42864978, seed: -37300132614189792, late: 15 });
part("keys", #{ style: "sustain", preset: "keys", register: [50, 74], density: 0.5272447, seed: 5396775171407160694 });
part("strings", #{ style: "tremolo", preset: "strings", register: [48, 79], density: 0.54910505, seed: -3681901744659677821 });
part("lead", #{ style: "ambient_line", preset: "mallet", register: [62, 79], density: 0.29242018, seed: 6845145886153731458 });

form([
  sec("intro", 8, "keys strings").energy(0.44030192),
  sec("verse", 8, "perc bass kick snare hat keys strings").energy(0.513739).fills(true),
  sec("hook", 8, "ohat perc bass strings lead kick snare hat keys").energy(0.7907866),
  sec("break", 4, "lead keys strings").energy(0.27113047).prog("A").gap(),
  sec("verse", 8, "perc bass keys kick snare hat strings").name("verse2").energy(0.57666034).gap(),
  sec("hook", 8, "ohat perc bass strings lead kick snare hat keys").name("hook2").energy(0.8887422).gap(),
  sec("hook", 8, "ohat perc bass strings lead kick snare hat keys").name("hook3").energy(0.98664886),
  sec("outro", 4, "strings keys").energy(0.3076398),
]);

mix("snare", #{ reverb: 0.15 });
mix("hat", #{ pan: 0.2 });
mix("ohat", #{ pan: -0.15 });
mix("perc", #{ pan: -0.35, reverb: 0.3, delay: 0.35 });
mix("bass", #{ duck: 0.15 });
mix("keys", #{ pan: -0.15, reverb: 0.5127861, delay: 0.18318664 });
mix("strings", #{ pan: 0.1, reverb: 0.4086404 });
mix("lead", #{ reverb: 0.42610887, delay: 0.24929999 });
mix("fx", #{ reverb: 0.45 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [
  #{ fx: "saturate", drive_db: 4, kind: "tube", mix: 0.4 },
  #{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 },
]);
fx("keys", [
  #{ fx: "phaser", depth: 0.6, feedback: 0.3, mix: 0.35, rate_hz: 0.18, stages: 4 },
  #{ fx: "tremolo", beats: 1, depth: 0.15 },
  #{ fx: "chorus", delay_ms: 11, depth_ms: 2.5, mix: 0.4, rate_hz: 0.45 },
]);
fx("strings", [
  #{ fx: "filter", cutoff: sections(2200, #{ hook: 6500 }), drive: 1, mix: 1, mode: "lp", res: 0.1 },
  #{ fx: "phaser", depth: 0.69981074, feedback: 0.37219805, mix: sections(0.2858641, #{ drop: 0.42805505, hook: 0.42805505 }), rate_hz: 0.14278676, stages: 6 },
]);
fx("lead", [
  #{ fx: "filter", cutoff: 320, drive: 1, mix: 1, mode: "hp", res: 0.05 },
  #{ fx: "flanger", delay_ms: 3.3502333, depth_ms: 2.713053, feedback: 0.49436772, mix: 0.4030139, rate_hz: 0.25133887 },
]);
bus("bass", [#{ fx: "saturate", drive_db: 3, kind: "tube", mix: 0.4 }]);
bus("drums", [
  #{ fx: "parallel", chains: [#{ amount: 0.35, fx: [#{ fx: "compressor", attack_ms: 1, knee_db: 3, makeup_db: 0, mix: 1, ratio: 8, release_ms: 60, threshold_db: -28 }, #{ fx: "saturate", drive_db: 6, kind: "tape", mix: 1 }] }, #{ amount: 0.25, fx: [#{ fx: "bitcrush", bits: 9, downsample: 3, mix: 1 }, #{ fx: "filter", cutoff: 6000, drive: 1, mix: 1, mode: "lp", res: 0.05 }] }], dry: 1 },
  #{ fx: "filter", cutoff: 13000, drive: 1, mix: 1, mode: "lp", res: 0.05 },
  #{ fx: "compressor", attack_ms: 15, knee_db: 6, makeup_db: 1, mix: 1, ratio: 2.5, release_ms: 120, threshold_db: -14 },
  #{ fx: "width", mono_below_hz: 160, width: 1 },
]);
bus("music", [
  #{ fx: "eq", bands: [#{ freq: 110, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "gain", db: sections(0, #{ "break": -3.5 }) },
]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 28, gain_db: 0, kind: "highpass", q: 0.7 }, #{ freq: 9000, gain_db: 0.5, kind: "highshelf", q: 0.7 }] },
  #{ fx: "tape", drive_db: 3, flutter: 0.25, lowpass_hz: 17000, wow: 0.35 },
  #{ fx: "vinyl", crackle: 0.12, hiss_db: -48 },
  #{ fx: "compressor", attack_ms: 30, knee_db: 6, makeup_db: 0, mix: 1, ratio: 1.5, release_ms: 200, threshold_db: -12 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.46764442, decay: 3.4529526, lowcut: 234.38934, mix: 1, predelay_ms: 35.852707, width: 1.400633 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.6, feedback: sections(0.32, #{ "break": [0.32, 0.58] }), mix: 1, pingpong: false } });

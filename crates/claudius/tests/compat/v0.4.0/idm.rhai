// Glass Garden — idm (seed 7)
// song("idm", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(98); key("E lydian"); swing(0.5103269);
humanize(0.15);
prog("A", "Imaj9 Vmaj9 II9 Vmaj9 Imaj9 Vmaj9 II9 V7");
prog("B", "II9 Vmaj9 Imaj9 Vmaj9");
bars_per_chord(2);
master(#{ lufs: -10, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X.....X..XX.....|X.X......X..X...|X.....X..XX.....|X.X......X..X...");
drum("kick", "sparse", "x...............");
drum("snare", "main", "......o.....oxxxo...o.o.....X...|........X...........o.......oxxx|......o.X.......o...o.o.....X...|........X...........o...X.......", 32);
drum("hat", "main", "x........x..x.....x..x..x.....x..x..x...........|x........x..x.....x..x..X.Xx.xx.xx.xx...........|x........x..x.....x..x..x.....x..x........X.xx.x|x........x..x.....x..x..x.....x..x..XXXxxxxxxxxx", 48);
drum_sound("click", "rim");
drum("click", "main", "x.x.x.x..x.x.x..");
drum("click", "sparse", "x.....x....x....");
drum("tom", "main", "........x.......x...........x.......x.......x...", 48);
drum("tom", "sparse", "....................x.......................x...", 48);

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "sustain", preset: "sub_bass", register: [28, 50], density: 0.5726087, seed: 2642474102756894640 });
part("bells", #{ style: "motif", preset: "bell", register: [64, 91], density: 0.568824, seed: -4128080410899203971 });
part("pad", #{ style: "sustain", preset: "strings", register: [48, 79], density: 0.59977853, seed: 4170999698929869575 });
part("arp", #{ style: "pinky", preset: "pluck", register: [60, 88], density: 0.5752324, rate: 0.125, seed: 6845145886153731458 });

form([
  sec("intro", 8, "bells pad click").energy(0.24030194).filter_in(),
  sec("groove", 8, "kick snare hat click bass bells pad").energy(0.563739).fills(false),
  sec("groove", 8, "*").name("groove2").energy(0.7407866),
  sec("break", 4, "pad bells click").energy(0.27113047).gap(),
  sec("drop", 8, "*").name("peak").energy(0.9266603).gap().impact(),
  sec("drop", 8, "*").name("peak2").energy(0.98874223),
  sec("outro", 4, "bells pad click").energy(0.28664884),
]);

mix("snare", #{ pan: 0.05, reverb: 0.12 });
mix("hat", #{ pan: 0.35 });
mix("click", #{ pan: -0.4, delay: 0.3 });
mix("tom", #{ level: -4, pan: 0.2, reverb: 0.2 });
mix("bass", #{ duck: 0.25 });
mix("bells", #{ pan: -0.15, reverb: 0.5127861, delay: 0.42743546 });
mix("pad", #{ reverb: 0.4086404 });
mix("arp", #{ pan: 0.3, reverb: 0.2434908, delay: 0.26710713 });
mix("fx", #{ reverb: 0.35 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [
  #{ fx: "saturate", drive_db: 4, kind: "tube", mix: 0.4 },
  #{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 },
]);
fx("bells", [
  #{ fx: "parallel", chains: [#{ amount: sections(0, #{ drop: 0.5, groove: [0, 0.3] }), fx: [#{ fx: "bitcrush", bits: 6, downsample: 6, mix: 1 }, #{ fx: "filter", cutoff: 2500, drive: 1, mix: 1, mode: "bp", res: 0.4 }] }], dry: 1 },
  #{ fx: "tape", drive_db: 1.8927076, flutter: 0.061683215, lowpass_hz: 9244.565, wow: 0.35970697 },
]);
fx("pad", [
  #{ fx: "parallel", chains: [#{ amount: sections(0, #{ drop: 0.85, groove: [0, 0.6] }), fx: [#{ fx: "gate", depth: 1, smooth_ms: 4, step_beats: 0.25, steps: [1, 0, 0.6, 0, 1, 0.3, 0, 0.8, 0, 1, 0, 0.5, 1, 0, 0.7, 0] }] }], dry: sections(1, #{ drop: 0.15, groove: [1, 0.4] }) },
  #{ fx: "phaser", depth: 0.69981074, feedback: 0.37219805, mix: sections(0.2858641, #{ drop: 0.42805505, hook: 0.42805505 }), rate_hz: 0.14278676, stages: 6 },
]);
fx("arp", [
  #{ fx: "bitcrush", bits: 10, downsample: 3, mix: 0.4 },
  #{ fx: "chorus", delay_ms: 14.267289, depth_ms: 3.6413164, mix: 0.50231516, rate_hz: 0.76374876 },
]);
bus("drums", [
  #{ fx: "parallel", chains: [#{ amount: 0.35, fx: [#{ fx: "bitcrush", bits: 7, downsample: 5, mix: 1 }, #{ fx: "filter", cutoff: 600, drive: 1, mix: 1, mode: "hp", res: 0 }] }, #{ amount: sections(0, #{ drop: 0.4 }), fx: [#{ fx: "compressor", attack_ms: 0.5, knee_db: 2, makeup_db: 0, mix: 1, ratio: 12, release_ms: 40, threshold_db: -32 }, #{ fx: "saturate", drive_db: 10, kind: "fold", mix: 1 }] }], dry: 1 },
  #{ fx: "compressor", attack_ms: 6, knee_db: 4, makeup_db: 1, mix: 1, ratio: 2.5, release_ms: 80, threshold_db: -14 },
]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 120, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 28, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 20, knee_db: 6, makeup_db: 0, mix: 1, ratio: 1.8, release_ms: 150, threshold_db: -12 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.2676444, decay: 2.8055239, lowcut: 281.2672, mix: 1, predelay_ms: 30.852709, width: 1.5 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.35, feedback: sections(0.35, #{ "break": [0.35, 0.7] }), mix: 1, pingpong: false } });

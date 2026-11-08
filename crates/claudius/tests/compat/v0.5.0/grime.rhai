// Glass Garden — grime (seed 7)
// song("grime", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(138); key("D minor"); swing(0.50688463);
humanize(0.1);
prog("A", "i iv V7 i");
prog("B", "i VI iv V");
bars_per_chord(2);
master(#{ lufs: -9, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X...............|..X.............|X...............|..X.............");
drum("snare", "main", "........X.......");
drum("hat", "alt", "x.x.x.x.x.x.x.xx");
drum("hat", "main", "x...x...x...x...");
drum("ohat", "main", "................|......x.........|................|......x.........");
drum("perc", "main", "......x.........");
drum_sound("perc", "rim");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "grime", preset: "sub_bass", register: [26, 45], density: 0.53341293, seed: -2526400537787473278 });
part("lead", #{ style: "riff", preset: "square_lead", register: [60, 84], density: 0.49405408, seed: 6553130387488681005 });
part("stabs", #{ style: "pizzicato", preset: "strings", register: [50, 74], density: 0.54910505, seed: -3681901744659677821 });
part("pad", #{ style: "sustain", preset: "vowel_pad", register: [48, 72], density: 0.47421524, seed: 805921219908132358 });

form([
  sec("intro", 8, "lead hat").energy(0.5403019).variant("main"),
  sec("verse", 8, "kick snare hat perc bass lead").energy(0.66373897),
  sec("hook", 8, "*").energy(0.8907866).fills(false).impact(),
  sec("verse", 8, "kick snare hat perc bass lead stabs").name("verse2").energy(0.72113043).variant("alt").fills(true),
  sec("break", 4, "lead pad").energy(0.37666035).gap(),
  sec("hook", 8, "*").name("hook2").energy(0.9387422).fills(false).gap().impact(),
  sec("hook", 8, "*").name("hook3").energy(0.98664886).fills(false),
  sec("outro", 4, "lead hat").energy(0.4076398).variant("main"),
]);

mix("snare", #{ reverb: 0.1 });
mix("hat", #{ pan: 0.15 });
mix("ohat", #{ pan: -0.15 });
mix("perc", #{ pan: -0.35, reverb: 0.15 });
mix("lead", #{ reverb: 0.21976548, delay: 0.18318664 });
mix("stabs", #{ reverb: 0.16345617 });
mix("pad", #{ reverb: 0.4869816 });
mix("fx", #{ reverb: 0.2 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [
  #{ fx: "saturate", drive_db: 4, kind: "tanh", mix: 0.6 },
  #{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 },
]);
fx("lead", [#{ fx: "tape", drive_db: 1.8927076, flutter: 0.061683215, lowpass_hz: 9244.565, wow: 0.35970697 }]);
fx("stabs", [#{ fx: "phaser", depth: 0.69981074, feedback: 0.37219805, mix: sections(0.2858641, #{ drop: 0.42805505, hook: 0.42805505 }), rate_hz: 0.14278676, stages: 6 }]);
fx("pad", [#{ fx: "width", mono_below_hz: 150, width: 1.4416066 }]);
bus("bass", [#{ fx: "saturate", drive_db: 3, kind: "tanh", mix: 0.5 }]);
bus("drums", [
  #{ fx: "saturate", drive_db: 3, kind: "tube", mix: 0.4 },
  #{ fx: "compressor", attack_ms: 6, knee_db: 6, makeup_db: 2, mix: 1, ratio: 3, release_ms: 80, threshold_db: -12 },
]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 140, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 28, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 15, knee_db: 6, makeup_db: 0, mix: 1, ratio: 2, release_ms: 120, threshold_db: -10 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.5176444, decay: 0.9711429, lowcut: 328.14508, mix: 1, predelay_ms: 15.852709, width: 1.400633 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.6, feedback: 0.21008913, mix: 1, pingpong: true } });

// Glass Garden — funk (seed 7)
// song("funk", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(97); key("D dorian"); swing(0.53376925);
humanize(0.4);
prog("A", "i7 IV9 i7 VIImaj7");
prog("B", "i7 VIImaj7 IV7 IV7");
bars_per_chord(2);
master(#{ lufs: -10, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X.X...X...X..X..");
lane("kick", #{ late: 2 });
drum("snare", "main", "..o.X....o..Xo.o");
lane("snare", #{ late: 1 });
drum("hat", "main", "x.xxx..xx.x.xxx.");
lane("hat", #{ late: -6 });
drum("ohat", "main", "......x.........");
lane("ohat", #{ late: -2 });
drum("perc", "main", "x.....x.....x...");
drum("perc", "sparse", "x...............");
drum_sound("perc", "cb");
drum("shaker", "main", "ooo..o.oo...ooo.");
lane("shaker", #{ late: -2 });

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "funk", register: [32, 53], density: 0.42864978, seed: -37300132614189792, late: 2 });
part("guitar", #{ style: "strum", preset: "pluck", register: [53, 77], density: 0.5272447, seed: 5396775171407160694 });
part("keys", #{ style: "gospel", preset: "rock_organ", register: [53, 77], density: 0.5144286, seed: 336892620131319353 });

form([
  sec("intro", 8, "kick snare hat guitar").energy(0.44030192).variant("main").filter_in(),
  sec("verse", 8, "kick snare hat perc bass keys").energy(0.56373894),
  sec("hook", 8, "kick snare hat ohat perc shaker bass guitar keys horns").energy(0.84078664),
  sec("break", 4, "bass guitar perc").energy(0.42113045),
  sec("verse", 8, "kick snare hat perc bass guitar").name("verse2").energy(0.6266603).fills(true).gap(),
  sec("hook", 8, "kick snare hat ohat perc shaker bass guitar keys horns").name("hook2").energy(0.8887422).gap(),
  sec("hook", 8, "kick snare hat ohat perc shaker bass guitar keys horns").name("hook3").energy(0.98664886),
  sec("outro", 4, "kick snare hat bass guitar").energy(0.45763978).variant("main"),
]);

mix("snare", #{ reverb: 0.1 });
mix("hat", #{ pan: 0.2 });
mix("ohat", #{ pan: -0.12 });
mix("perc", #{ level: -3, pan: 0.35 });
mix("shaker", #{ pan: -0.3 });
mix("guitar", #{ pan: 0.3, reverb: 0.17581238 });
mix("keys", #{ pan: -0.25, reverb: 0.12259213 });
mix("fx", #{ reverb: 0.2 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [
  #{ fx: "compressor", attack_ms: 8, knee_db: 4, makeup_db: 2, mix: 1, ratio: 4, release_ms: 80, threshold_db: -18 },
  #{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 },
]);
fx("guitar", [
  #{ fx: "filter", cutoff: 250, drive: 1, mix: 1, mode: "hp", res: 0.05 },
  #{ fx: "compressor", attack_ms: 3, knee_db: 4, makeup_db: 2, mix: 1, ratio: 4, release_ms: 60, threshold_db: -20 },
  #{ fx: "phaser", depth: 0.6, feedback: 0.3, mix: 0.25, rate_hz: 0.6, stages: 4 },
  #{ fx: "chorus", delay_ms: 10.380553, depth_ms: 1.6460402, mix: 0.26222828, rate_hz: 0.71941394 },
]);
fx("keys", [
  #{ fx: "chorus", delay_ms: 11, depth_ms: 2.5, mix: 0.4, rate_hz: 0.45 },
  #{ fx: "phaser", depth: 0.69981074, feedback: 0.37219805, mix: sections(0.2858641, #{ drop: 0.42805505, hook: 0.42805505 }), rate_hz: 0.14278676, stages: 6 },
]);
bus("bass", [#{ fx: "saturate", drive_db: 2, kind: "tube", mix: 0.4 }]);
bus("drums", [
  #{ fx: "saturate", drive_db: 2, kind: "tube", mix: 0.4 },
  #{ fx: "compressor", attack_ms: 5.585167, knee_db: 6, makeup_db: 2, mix: 1, ratio: 3.655613, release_ms: 88.36954, threshold_db: -11.541128 },
]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 100, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 28, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "tape", drive_db: 1, flutter: 0.05, lowpass_hz: 18000, wow: 0.05 },
  #{ fx: "compressor", attack_ms: 15, knee_db: 6, makeup_db: 0, mix: 1, ratio: 2, release_ms: 140, threshold_db: -11 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.46764442, decay: 0.9711429, lowcut: 281.2672, mix: 1, predelay_ms: 13.852709, width: 1.2839136 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.6, feedback: 0.16807131, mix: 1, pingpong: false } });

// racks: tracks played by several sounds at once, each its band, level, duck and effects
rack("bass", [
  #{ sound: "sub_bass", band: [0, 90], level: -3, fold: [28, 40] },
  #{ sound: "pluck_bass", band: [90, 0] },
]);

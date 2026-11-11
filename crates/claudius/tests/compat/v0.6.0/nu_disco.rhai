// Glass Garden — nu_disco (seed 7)
// song("nu_disco", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(113); key("D dorian"); swing(0.5103269);
humanize(0.3);
prog("A", "i7 IV9 i7 VIImaj7");
prog("B", "i7 VIImaj7 IV7 IV7");
bars_per_chord(1);
master(#{ lufs: -10, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X...X...X...X...");
drum("clap", "main", "....X.......X...");
lane("clap", #{ late: 2 });
drum("hat", "main", "x..xx.......x..o");
drum("hat", "sparse", "x...x...x...x...");
drum("ohat", "main", "..x...x...x...x.");
drum("shaker", "main", ".oo.oo....oooooo");
lane("shaker", #{ late: -1 });
drum("perc", "main", "x...x.....x.....");
drum("perc", "sparse", "....x...........");
drum_sound("perc", "cb");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "disco", preset: "saw_bass", register: [33, 52], density: 0.42864978, seed: -37300132614189792 });
part("guitar", #{ style: "scratch", preset: "pluck", register: [52, 76], density: 0.5272447, seed: 5396775171407160694 });
part("keys", #{ style: "gospel", preset: "rock_organ", register: [52, 74], density: 0.56008077, seed: -1629742732023437997 });
part("strings", #{ style: "disco_strings", preset: "strings", register: [55, 82], density: 0.49242017, seed: 6845145886153731458 });

form([
  sec("intro", 8, "kick hat shaker bass").energy(0.59030193),
  sec("verse", 8, "kick clap hat ohat shaker bass guitar keys").energy(0.61373895),
  sec("hook", 8, "kick clap hat ohat shaker perc bass guitar keys strings lead").energy(0.8907866).impact(),
  sec("break", 4, "strings keys perc").energy(0.37113047),
  sec("verse", 8, "kick clap hat ohat shaker bass guitar keys strings").name("verse2").energy(0.6766603).fills(true).gap(),
  sec("hook", 8, "kick clap hat ohat shaker perc bass guitar keys strings lead").name("hook2").energy(0.9387422).gap().impact(),
  sec("hook", 8, "kick clap hat ohat shaker perc bass guitar keys strings lead").name("hook3").energy(0.98664886),
  sec("outro", 4, "kick hat shaker bass guitar").energy(0.45763978),
]);

mix("clap", #{ reverb: 0.2 });
mix("hat", #{ pan: 0.2 });
mix("ohat", #{ pan: -0.1 });
mix("shaker", #{ pan: -0.3 });
mix("perc", #{ pan: 0.35, reverb: 0.1, delay: 0.1 });
mix("bass", #{ duck: 0.35 });
mix("guitar", #{ pan: 0.3, reverb: 0.21976548, duck: 0.3 });
mix("keys", #{ pan: -0.25, reverb: 0.16345617, duck: 0.35 });
mix("strings", #{ reverb: 0.4869816, duck: 0.4 });
mix("fx", #{ reverb: 0.3 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [
  #{ fx: "compressor", attack_ms: 10, knee_db: 4, makeup_db: 2, mix: 1, ratio: 4, release_ms: 80, threshold_db: -18 },
  #{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 },
]);
fx("guitar", [
  #{ fx: "filter", cutoff: 250, drive: 1, mix: 1, mode: "hp", res: 0.05 },
  #{ fx: "compressor", attack_ms: 3, knee_db: 4, makeup_db: 2, mix: 1, ratio: 4, release_ms: 60, threshold_db: -20 },
  #{ fx: "chorus", delay_ms: 9, depth_ms: 2, mix: 0.3, rate_hz: 0.6 },
]);
fx("keys", [
  #{ fx: "chorus", delay_ms: 11, depth_ms: 2.5, mix: 0.4, rate_hz: 0.45 },
  #{ fx: "phaser", depth: 0.69981074, feedback: 0.37219805, mix: sections(0.2858641, #{ drop: 0.42805505, hook: 0.42805505 }), rate_hz: 0.14278676, stages: 6 },
]);
fx("strings", [
  #{ fx: "phaser", depth: 0.7, feedback: 0.35, mix: sections(0.25, #{ "break": [0.25, 0.6] }), rate_hz: 0.25, stages: 6 },
  #{ fx: "width", mono_below_hz: 150, width: 1.4416066 },
]);
bus("drums", [
  #{ fx: "compressor", attack_ms: 12, knee_db: 6, makeup_db: 1, mix: 1, ratio: 2.2, release_ms: 120, threshold_db: -12 },
  #{ fx: "width", mono_below_hz: 120, width: 1 },
]);
bus("music", [
  #{ fx: "eq", bands: [#{ freq: 110, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 20, knee_db: 6, makeup_db: 0, mix: 1, ratio: 1.6, release_ms: 160, threshold_db: -16 },
]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 28, gain_db: 0, kind: "highpass", q: 0.7 }, #{ freq: 10000, gain_db: 1, kind: "highshelf", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 25, knee_db: 6, makeup_db: 0, mix: 1, ratio: 1.8, release_ms: 160, threshold_db: -12 },
  #{ fx: "saturate", drive_db: 2, kind: "tape", mix: 0.25 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.4176444, decay: 2.1580954, lowcut: 281.2672, mix: 1, predelay_ms: 25.852709, width: 1.400633 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.5, feedback: 0.25210696, mix: 1, pingpong: true } });

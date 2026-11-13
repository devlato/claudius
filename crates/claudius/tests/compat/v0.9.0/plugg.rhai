// Glass Garden — plugg (seed 7)
// song("plugg", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(141); key("D major"); swing(0.5034423);
humanize(0.05);
prog("A", "vi7 ii7 V7 Imaj7");
prog("B", "Imaj7 vi7 ii7 V7");
bars_per_chord(1);
master(#{ lufs: -9.5, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "alt", "X...............");
drum("kick", "main", "X.........X.....|X.....X.........|X.........X.....|X.....X.........");
drum("clap", "main", "........X.......");
drum("hat", "alt", "x.......x.......x.......x.......|x.......x.......x.......x.......|x.......XXxxxxxxx...........oxxx|x.......x.......x.......x.......", 32);
drum("hat", "main", "x...........x...........x...........X.X.x.x.x.x.|x...........X.Xx.xx.xx.xXXXxxxxxxxxxXXXxxxxxxxxx|x...........x...........x...........x.....x.....|x...........x...........x...........x.....x.....", 48);
drum("ohat", "main", "................");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "sustain", preset: "808", register: [24, 42], density: 0.5726087, seed: 2642474102756894640 });
part("keys", #{ style: "sustain", preset: "pad", register: [52, 76], density: 0.4710492, seed: -6710496186308540014 });
part("lead", #{ style: "trap_bells", preset: "soft_lead", register: [67, 88], density: 0.42006055, seed: -1629742732023437997 });

form([
  sec("intro", 8, "keys pad lead").energy(0.4903019).variant("main"),
  sec("hook", 8, "kick clap hat ohat bass keys pad lead").energy(0.813739),
  sec("verse", 8, "kick clap hat bass keys pad").energy(0.6407866).variant("alt").gap(),
  sec("hook", 8, "kick clap hat ohat bass keys pad lead").name("hook2").energy(0.8711304),
  sec("verse", 8, "kick clap hat bass keys lead").name("verse2").energy(0.6566603).variant("alt").fills(true),
  sec("hook", 8, "kick clap hat ohat bass keys pad lead").name("hook3").energy(0.90874225).gap(),
  sec("hook", 8, "kick clap hat ohat bass keys pad lead").name("hook4").energy(0.98664886),
  sec("outro", 4, "keys pad lead").energy(0.3576398).variant("main"),
]);

mix("clap", #{ reverb: 0.25 });
mix("hat", #{ pan: 0.12 });
mix("ohat", #{ pan: -0.15 });
mix("keys", #{ reverb: 0.5127861 });
mix("lead", #{ level: -3, reverb: 0.32691234, delay: 0.4658131 });
mix("fx", #{ level: -4, reverb: 0.3 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [
  #{ fx: "saturate", drive_db: 3, kind: "tanh", mix: 0.7 },
  #{ fx: "filter", cutoff: 4000, drive: 1, mix: 1, mode: "lp", res: 0.1 },
  #{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 },
]);
fx("keys", [#{ fx: "chorus", delay_ms: 12, depth_ms: 2.5, mix: 0.3, rate_hz: 0.4 }]);
fx("lead", [#{ fx: "saturate", drive_db: 5.1466026, kind: "tube", mix: 0.32567206 }]);
bus("bass", [#{ fx: "saturate", drive_db: 2, kind: "tanh", mix: 0.4 }]);
bus("drums", [#{ fx: "compressor", attack_ms: 10, knee_db: 6, makeup_db: 1, mix: 1, ratio: 2, release_ms: 100, threshold_db: -11 }]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 140, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 24, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 25, knee_db: 6, makeup_db: 0, mix: 1, ratio: 1.8, release_ms: 160, threshold_db: -10 },
  #{ fx: "saturate", drive_db: 1.5, kind: "tanh", mix: 0.25 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.3676444, decay: 2.5897145, lowcut: 262.51608, mix: 1, predelay_ms: 25.852709, width: 1.5 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.5, feedback: 0.29412478, mix: 1, pingpong: true } });

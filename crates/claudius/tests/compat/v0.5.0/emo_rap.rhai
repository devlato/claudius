// Glass Garden — emo_rap (seed 7)
// song("emo_rap", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(132); key("D minor"); swing(0.5051635);
humanize(0.12);
prog("A", "i VI VII v");
prog("B", "i VI III VII");
bars_per_chord(1);
master(#{ lufs: -9, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "alt", "X.........X.....|X...............|X.........X.....|X...............");
drum("kick", "main", "X......X..X.....|X.....X.........|X......X..X.....|X.....X.........");
drum("snare", "main", "........X.......");
drum("clap", "main", "........X.......");
drum("hat", "alt", "x.....x.....o.x.x.x.x.X.x.....x.....x.....x.....|x.....x...........X.xx.xx.....x.....o.x.x.x.x.X.|x.....x.....x.....x.....x.....x.....x.....x.....|......o.xx.xx.....x.....x.....x.....o.xx.xx.xx.X", 48);
drum("hat", "main", "x.....x.....x.....x.....x.....x.....x.....x..x..|x.....x...........X.xx.xx.....x.....x.....x..x..|x.....x.....x.....x.....x.....x.....o.x.x.x.x.X.|x.....x.....x.....x.....x.....x.....x.....x..x..", 48);
drum("ohat", "main", "..............x.");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "kick_follow", preset: "808", register: [24, 40], density: 0.5726087, seed: 2642474102756894640 });
part("guitar", #{ style: "arpeggio", preset: "harp_pluck", register: [52, 76], density: 0.5272447, seed: 5396775171407160694 });
part("pad", #{ style: "sustain", preset: "strings", register: [48, 72], density: 0.56008077, seed: -1629742732023437997 });
part("lead", #{ style: "rock_lead", preset: "soft_lead", register: [64, 84], density: 0.20655337, seed: 6104030045851553230 });

form([
  sec("intro", 8, "pad guitar").energy(0.4903019).variant("main"),
  sec("hook", 8, "kick snare clap hat ohat bass pad lead guitar").energy(0.813739).impact(),
  sec("verse", 8, "kick snare hat bass guitar").energy(0.6407866).variant("alt").gap(),
  sec("hook", 8, "kick snare clap hat ohat bass pad lead guitar").name("hook2").energy(0.8711304).impact(),
  sec("break", 4, "pad guitar").name("bridge").energy(0.37666035).prog("A"),
  sec("hook", 8, "kick snare clap hat ohat bass pad lead guitar").name("hook3").energy(0.9387422).gap().impact(),
  sec("hook", 8, "kick snare clap hat ohat bass pad lead guitar").name("hook4").energy(0.98664886),
  sec("outro", 4, "pad guitar").energy(0.3576398).variant("main"),
]);

mix("snare", #{ reverb: 0.2 });
mix("clap", #{ level: -4, reverb: 0.25 });
mix("hat", #{ pan: 0.12 });
mix("ohat", #{ pan: -0.15 });
mix("guitar", #{ reverb: 0.5127861, delay: 0.18318664 });
mix("pad", #{ level: -4, reverb: 0.4086404 });
mix("lead", #{ level: -5, reverb: 0.42610887, delay: 0.22258927 });
mix("fx", #{ level: -3, reverb: 0.3 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [
  #{ fx: "saturate", drive_db: 9, kind: "tanh", mix: 1 },
  #{ fx: "filter", cutoff: 4500, drive: 1, mix: 1, mode: "lp", res: 0.1 },
  #{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 },
]);
fx("guitar", [#{ fx: "chorus", delay_ms: 10, depth_ms: 2, mix: 0.3, rate_hz: 0.6 }]);
fx("pad", [#{ fx: "phaser", depth: 0.69981074, feedback: 0.37219805, mix: sections(0.2858641, #{ drop: 0.42805505, hook: 0.42805505 }), rate_hz: 0.14278676, stages: 6 }]);
fx("lead", [#{ fx: "flanger", delay_ms: 3.3502333, depth_ms: 2.713053, feedback: 0.49436772, mix: 0.4030139, rate_hz: 0.25133887 }]);
bus("bass", [#{ fx: "saturate", drive_db: 4, kind: "tanh", mix: 0.6 }]);
bus("drums", [#{ fx: "compressor", attack_ms: 8, knee_db: 6, makeup_db: 1, mix: 1, ratio: 2, release_ms: 90, threshold_db: -10 }]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 140, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 24, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 25, knee_db: 6, makeup_db: 0, mix: 1, ratio: 1.8, release_ms: 150, threshold_db: -9 },
  #{ fx: "saturate", drive_db: 2, kind: "tanh", mix: 0.3 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.4176444, decay: 2.373905, lowcut: 262.51608, mix: 1, predelay_ms: 23.852709, width: 1.5 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.55, feedback: 0.26891407, mix: 1, pingpong: true } });

// Glass Garden — alt_rnb (seed 7)
// song("alt_rnb", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(62); key("D minor"); swing(0.5103269);
humanize(0.2);
prog("A", "i7 iv7 v7 i7");
prog("B", "iv9 v7 i7 VImaj7");
bars_per_chord(1);
master(#{ lufs: -10.5, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "alt", "X...............|..X.............|X...............|..X.............");
drum("kick", "main", "X.........X.....|..X.............|X...............|..X........X....");
drum("snare", "main", "....X.......X...");
lane("snare", #{ late: 4 });
drum("clap", "main", "....x.......x...");
lane("clap", #{ late: 13 });
drum("hat", "alt", "o...o...o...o...");
drum("hat", "main", "x.....o.....X.X.x.x.x.x.XXXxxxxxxxxxx.....o.....|x.....o.....x.....o.....x.....o.....ooxxxxxxxxXX|x.....o.....x.....o.....o.xx.xx.xx.Xx.....o.....|x.....o.....x.....o.....o.xx.xx.xx.XX.Xx.xx.xx.x", 48);
lane("hat", #{ late: 1 });
drum("ohat", "main", "................");
lane("ohat", #{ late: -1 });

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "sustain", preset: "sub_bass", register: [24, 42], density: 0.53341293, seed: -2526400537787473278, late: 6 });
part("keys", #{ style: "sustain", preset: "epiano", register: [48, 74], density: 0.55524325, seed: 6553130387488681005 });
part("pad", #{ style: "sustain", preset: "swell_pad", register: [48, 74], density: 0.44522208, seed: -4694233468511451402 });
part("lead", #{ style: "ambient_line", preset: "vox_lead", register: [60, 82], density: 0.26931512, seed: 6845145886153731458 });

form([
  sec("intro", 8, "pad keys").energy(0.44030192).variant("main"),
  sec("verse", 4, "kick snare hat bass keys pad").energy(0.513739),
  sec("hook", 4, "*").energy(0.7907866),
  sec("verse", 4, "kick snare hat bass keys pad").name("verse2").energy(0.57113045).variant("alt").fills(true).gap(),
  sec("hook", 4, "*").name("hook2").energy(0.82666034),
  sec("break", 4, "pad keys lead").name("bridge").energy(0.3887422),
  sec("hook", 4, "*").name("hook3").energy(0.88664883).gap(),
  sec("hook", 4, "*").name("hook4").energy(1),
  sec("outro", 4, "pad keys").energy(0.2537415).variant("main"),
]);

mix("snare", #{ reverb: 0.4 });
mix("clap", #{ level: -5, reverb: 0.45 });
mix("hat", #{ pan: 0.15, reverb: 0.15 });
mix("ohat", #{ pan: -0.15, reverb: 0.3 });
mix("keys", #{ reverb: 0.6592964 });
mix("pad", #{ level: -2, reverb: 0.5720966 });
mix("lead", #{ level: -3, reverb: 0.6695997, delay: 0.26710713 });
mix("fx", #{ level: -5, reverb: 0.5 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [
  #{ fx: "saturate", drive_db: 3, kind: "tanh", mix: 0.6 },
  #{ fx: "filter", cutoff: 3500, drive: 1, mix: 1, mode: "lp", res: 0.1 },
  #{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 },
]);
fx("keys", [
  #{ fx: "tape", drive_db: 1.5, flutter: 0.1, lowpass_hz: 9000, wow: 0.2 },
  #{ fx: "chorus", delay_ms: 11, depth_ms: 2.5, mix: 0.4, rate_hz: 0.45 },
]);
fx("pad", [
  #{ fx: "chorus", delay_ms: 14, depth_ms: 3, mix: 0.3, rate_hz: 0.3 },
  #{ fx: "phaser", depth: 0.69981074, feedback: 0.37219805, mix: sections(0.2858641, #{ drop: 0.42805505, hook: 0.42805505 }), rate_hz: 0.14278676, stages: 6 },
]);
fx("lead", [#{ fx: "flanger", delay_ms: 3.3502333, depth_ms: 2.713053, feedback: 0.49436772, mix: 0.4030139, rate_hz: 0.25133887 }]);
bus("bass", [#{ fx: "saturate", drive_db: 2, kind: "tanh", mix: 0.4 }]);
bus("drums", [#{ fx: "compressor", attack_ms: 10, knee_db: 6, makeup_db: 1, mix: 1, ratio: 2.2, release_ms: 110, threshold_db: -13 }]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 110, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 24, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "tape", drive_db: 1, flutter: 0.05, lowpass_hz: 15000, wow: 0.08 },
  #{ fx: "compressor", attack_ms: 25, knee_db: 6, makeup_db: 0, mix: 1, ratio: 1.8, release_ms: 180, threshold_db: -11 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.4176444, decay: 3.4529526, lowcut: 234.38934, mix: 1, predelay_ms: 30.852709, width: 1.5 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.6, feedback: 0.3193355, mix: 1, pingpong: true } });

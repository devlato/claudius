// Glass Garden — synthwave (seed 7)
// song("synthwave", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(87); key("D minor"); swing(0.5);
humanize(0.1);
prog("A", "i III VII IV");
prog("B", "i VI VII i");
bars_per_chord(1);
master(#{ lufs: -10.5, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "alt", "X...X...X...X...");
drum("kick", "main", "X.......X.......");
drum("snare", "main", "....X.......X...");
drum("hat", "alt", "x.x.xxx.x.xxx.x.");
drum("hat", "main", "x.x.x.x.x.x.x.x.");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "root8", preset: "pluck_bass", register: [33, 48], density: 0.42864978, seed: -37300132614189792 });
part("arp", #{ style: "up", preset: "chip", register: [60, 84], density: 0.55524325, rate: 0.25, seed: -6710496186308540014 });
part("pad", #{ style: "sustain", preset: "glass_pad", register: [50, 76], density: 0.44522208, seed: -4694233468511451402 });
part("lead", #{ style: "motif", preset: "bell", register: [64, 86], density: 0.5252324, seed: 8524255199291047346 });
part("chords", #{ style: "half", preset: "supersaw", register: [55, 76], density: 0.49820793, seed: 4272912310042672631 });

form([
  sec("intro", 8, "pad arp").energy(0.4903019).variant("main"),
  sec("verse", 8, "kick snare hat bass arp pad").energy(0.563739),
  sec("hook", 8, "*").name("chorus").energy(0.84078664).impact(),
  sec("verse", 8, "kick snare hat bass arp pad").name("verse2").energy(0.57113045).fills(true).gap(),
  sec("hook", 8, "*").name("chorus2").energy(0.8766603).impact(),
  sec("break", 4, "pad lead").name("bridge").energy(0.4387422).build(true),
  sec("hook", 4, "*").name("chorus3").energy(0.98664886).gap().impact(),
  sec("hook", 4, "*").name("chorus4").energy(1),
  sec("outro", 4, "pad arp").energy(0.30374148).variant("main"),
]);

mix("snare", #{ reverb: 0.45 });
mix("hat", #{ pan: 0.2 });
mix("bass", #{ duck: 0.3 });
mix("arp", #{ pan: 0.15, reverb: 0.3662758, delay: 0.36637327, duck: 0.2 });
mix("pad", #{ reverb: 0.32691234, duck: 0.25 });
mix("lead", #{ reverb: 0.3652362, delay: 0.22258927 });
mix("chords", #{ reverb: 0.39236584, duck: 0.3 });
mix("fx", #{ level: -3, reverb: 0.35 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [#{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 }]);
fx("arp", [#{ fx: "auto_pan", beats: 4, depth: 0.32336643 }]);
fx("pad", [#{ fx: "chorus", delay_ms: 14, depth_ms: 3, mix: 0.5, rate_hz: 0.6 }]);
fx("lead", [#{ fx: "chorus", delay_ms: 12, depth_ms: 2.5, mix: 0.4, rate_hz: 0.8 }]);
fx("chords", []);
bus("drums", [#{ fx: "compressor", attack_ms: 12, knee_db: 6, makeup_db: 1, mix: 1, ratio: 2, release_ms: 120, threshold_db: -12 }]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 110, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 25, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 30, knee_db: 6, makeup_db: 0, mix: 1, ratio: 1.6, release_ms: 200, threshold_db: -11 },
  #{ fx: "saturate", drive_db: 2.5, kind: "tape", mix: 0.4 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.2676444, decay: 3.4529526, lowcut: 234.38934, mix: 1, predelay_ms: 35.852707, width: 1.5 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.45, feedback: 0.33614263, mix: 1, pingpong: true } });

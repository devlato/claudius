// Glass Garden — dance_pop (seed 7)
// song("dance_pop", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(117); key("D major"); swing(0.5034423);
humanize(0.1);
prog("A", "I bVII IV I");
prog("B", "IV I V vi");
bars_per_chord(1);
master(#{ lufs: -8.5, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X...X...X...X...");
drum("kick", "pop", "X.......X.X.....");
drum("clap", "main", "....X.......X...");
drum("hat", "main", "..xx..x..xxxxxxx");
drum("hat", "pop", "x.x.x.x.x.x.x.xx");
drum("hat", "sparse", "..x...x...x...x.");
drum("ohat", "main", "..x...x...x...x.");
drum("shaker", "main", "ooo.o.o.o.oo....");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "offbeat", preset: "sub_bass", register: [33, 52], density: 0.53341293, seed: -2526400537787473278 });
part("chords", #{ style: "syncopated", preset: "pluck", register: [55, 76], density: 0.55524325, seed: 6553130387488681005 });
part("pad", #{ style: "sustain", preset: "solina", register: [50, 76], density: 0.44522208, seed: -4694233468511451402 });
part("hook", #{ style: "motif", preset: "vox_lead", register: [60, 77], density: 0.5252324, seed: 8524255199291047346 });
part("lead", #{ style: "motif", preset: "fm_marimba", register: [67, 88], density: 0.346327, seed: -4105371792258199394 });

form([
  sec("intro", 8, "chords pad hat").energy(0.59030193),
  sec("verse", 8, "kick clap hat bass chords hook").energy(0.563739).variant("pop").fills(true),
  sec("build", 4, "clap hat bass chords pad hook").name("pre").energy(0.6907866).prog("B").gap(),
  sec("hook", 8, "*").name("chorus").energy(0.9211304).impact(),
  sec("verse", 8, "kick clap hat shaker bass chords hook").name("verse2").energy(0.6266603).variant("pop"),
  sec("build", 4, "clap hat bass chords pad hook").name("pre2").energy(0.6887422).prog("B").gap(),
  sec("hook", 4, "*").name("chorus2").energy(0.98664886).impact(),
  sec("break", 8, "pad hook chords").name("bridge").energy(0.4076398),
  sec("hook", 4, "*").name("chorus3").energy(0.9537415).gap().impact(),
  sec("hook", 4, "*").name("chorus4").energy(1),
  sec("outro", 4, "kick hat chords pad").energy(0.42864895),
]);

mix("clap", #{ reverb: 0.2 });
mix("hat", #{ pan: 0.2 });
mix("ohat", #{ pan: -0.15 });
mix("shaker", #{ pan: -0.35 });
mix("bass", #{ duck: 0.6 });
mix("chords", #{ reverb: 0.29302064, delay: 0.122124426, duck: 0.5 });
mix("pad", #{ reverb: 0.2860483, duck: 0.55 });
mix("hook", #{ reverb: 0.3652362, delay: 0.17807142, duck: 0.2 });
mix("lead", #{ pan: 0.15, reverb: 0.32697153, delay: 0.34599537, duck: 0.35 });
mix("fx", #{ reverb: 0.3 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [#{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 }]);
fx("chords", [
  #{ fx: "width", mono_below_hz: 200, width: 1.3 },
  #{ fx: "chorus", delay_ms: 10.380553, depth_ms: 1.6460402, mix: 0.26222828, rate_hz: 0.71941394 },
]);
fx("pad", [#{ fx: "phaser", depth: 0.69981074, feedback: 0.37219805, mix: sections(0.2858641, #{ drop: 0.42805505, hook: 0.42805505 }), rate_hz: 0.14278676, stages: 6 }]);
fx("hook", [
  #{ fx: "eq", bands: [#{ freq: 200, gain_db: 0, kind: "highpass", q: 0.7 }, #{ freq: 3000, gain_db: 2, kind: "peak", q: 1 }] },
  #{ fx: "compressor", attack_ms: 5, knee_db: 4, makeup_db: 2, mix: 1, ratio: 3, release_ms: 80, threshold_db: -18 },
  #{ fx: "flanger", delay_ms: 3.3502333, depth_ms: 2.713053, feedback: 0.49436772, mix: 0.4030139, rate_hz: 0.25133887 },
]);
bus("drums", [#{ fx: "compressor", attack_ms: 8, knee_db: 4, makeup_db: 1.5, mix: 1, ratio: 2.5, release_ms: 90, threshold_db: -12 }]);
bus("music", [
  #{ fx: "eq", bands: [#{ freq: 140, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 15, knee_db: 6, makeup_db: 0, mix: 1, ratio: 2, release_ms: 120, threshold_db: -16 },
]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 30, gain_db: 0, kind: "highpass", q: 0.7 }, #{ freq: 10000, gain_db: 0.5, kind: "highshelf", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 15, knee_db: 6, makeup_db: 0, mix: 1, ratio: 2, release_ms: 120, threshold_db: -10 },
  #{ fx: "saturate", drive_db: 1.5, kind: "tanh", mix: 0.25 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.3676444, decay: 1.9422858, lowcut: 328.14508, mix: 1, predelay_ms: 25.852709, width: 1.4589927 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.45, feedback: 0.25210696, mix: 1, pingpong: true } });

// Glass Garden — dub_techno (seed 7)
// song("dub_techno", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(117); key("D minor"); swing(0.50688463);
humanize(0.1);
prog("A", "i i i VI");
prog("B", "i i VI VI");
bars_per_chord(4);
master(#{ lufs: -10.5, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X...X...X...X...");
drum("hat", "main", "..x...x...x...x.");
drum("hat", "sparse", "..o...o...o...o.");
drum("ohat", "main", "..............o.");
drum("rim", "main", ".......o........");
drum("shaker", "main", "....ooo.oo....oo");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "dub", preset: "organ_bass", register: [28, 43], density: 0.5995956, seed: 3796763344308755361 });
part("chords", #{ style: "stabs", preset: "stab", register: [50, 70], density: 0.45851198, seed: 6674420348178494801 });
part("pad", #{ style: "sustain", preset: "swell_pad", register: [48, 74], density: 0.5823303, seed: -7892568474930094457 });

form([
  sec("intro", 4, "kick hat rim chords").energy(0.5403019),
  sec("groove", 16, "kick hat ohat rim bass chords pad").energy(0.61373895).fills(false),
  sec("break", 8, "chords pad rim").energy(0.3407866).prog("A"),
  sec("drop", 16, "*").name("deep").energy(0.82113045).gap(),
  sec("drop", 16, "*").name("deep2").energy(0.9766603),
  sec("outro", 8, "kick hat rim chords").energy(0.3887422),
]);

mix("hat", #{ pan: 0.15 });
mix("ohat", #{ pan: -0.15 });
mix("rim", #{ pan: -0.3, reverb: 0.35, delay: 0.45 });
mix("shaker", #{ pan: 0.35 });
mix("bass", #{ duck: 0.5 });
mix("chords", #{ reverb: 0.5860413, delay: 0.6, duck: 0.35 });
mix("pad", #{ reverb: 0.36777636, duck: 0.4 });
mix("fx", #{ reverb: 0.5 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [#{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 }]);
fx("chords", [
  #{ fx: "filter", cutoff: sections(1500, #{ "break": [3000, 900], drop: [1800, 4500], groove: [1500, 3000], intro: [500, 1500], outro: [1500, 450] }), drive: 1, mix: 1, mode: "lp", res: 0.35 },
  #{ fx: "chorus", delay_ms: 12, depth_ms: 3, mix: 0.6, rate_hz: 0.25 },
]);
fx("pad", [
  #{ fx: "filter", cutoff: 2200, drive: 1, mix: 1, mode: "lp", res: 0.1 },
  #{ fx: "phaser", depth: 0.69981074, feedback: 0.37219805, mix: sections(0.2858641, #{ drop: 0.42805505, hook: 0.42805505 }), rate_hz: 0.14278676, stages: 6 },
]);
bus("drums", [#{ fx: "compressor", attack_ms: 15, knee_db: 6, makeup_db: 1, mix: 1, ratio: 2, release_ms: 120, threshold_db: -14 }]);
bus("music", [
  #{ fx: "eq", bands: [#{ freq: 140, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "gain", db: sections(0, #{ "break": -3.5 }) },
]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 28, gain_db: 0, kind: "highpass", q: 0.7 }, #{ freq: 9000, gain_db: -1.5, kind: "highshelf", q: 0.7 }] },
  #{ fx: "tape", drive_db: 1, flutter: 0.1, lowpass_hz: 15000, wow: 0.15 },
  #{ fx: "compressor", attack_ms: 25, knee_db: 6, makeup_db: 0, mix: 1, ratio: 1.8, release_ms: 180, threshold_db: -12 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.5176444, decay: 3.7766669, lowcut: 281.2672, mix: 1, predelay_ms: 40.852707, width: 1.5 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.7, feedback: sections(0.45, #{ "break": [0.45, 0.7], drop: [0.5, 0.6] }), mix: 1, pingpong: false } });

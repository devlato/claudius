// Glass Garden — trance (seed 7)
// song("trance", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(134); key("D minor"); swing(0.50172114);
humanize(0.05);
prog("A", "i VI VII v");
prog("B", "i VI III VII");
bars_per_chord(2);
master(#{ lufs: -9, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X...X...X...X...");
drum("clap", "main", "....X.......X...");
drum("hat", "main", "o..ooo......o..o");
drum("ohat", "main", "..x...x...x...x.");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "rolling", preset: "fm_bass", register: [33, 50], density: 0.53341293, seed: -2526400537787473278 });
part("chords", #{ style: "pump", preset: "supersaw", register: [55, 79], density: 0.55524325, seed: 6553130387488681005 });
part("pad", #{ style: "sustain", preset: "strings", register: [48, 76], density: 0.44522208, seed: -4694233468511451402 });
part("lead", #{ style: "trance_lead", preset: "saw_lead", register: [64, 88], density: 0.5690405, seed: 8524255199291047346 });
part("arp", #{ style: "random", preset: "pluck", register: [60, 86], density: 0.49820793, rate: 0.25, seed: -4105371792258199394 });

form([
  sec("intro", 4, "kick hat ohat").energy(0.5403019).variant("main"),
  sec("groove", 8, "kick clap hat ohat bass arp").energy(0.66373897).fills(false),
  sec("break", 16, "pad chords lead").energy(0.39078662).build(true),
  sec("build", 4, "clap hat pad chords lead arp").energy(0.6211304).gap(),
  sec("drop", 16, "kick clap hat ohat bass arp pad chords lead").energy(0.9766603).impact(),
  sec("drop", 16, "kick clap hat ohat bass arp pad chords lead").name("drop2").energy(0.98874223),
  sec("outro", 8, "kick hat ohat arp").energy(0.38664883).variant("main"),
]);

mix("clap", #{ reverb: 0.2 });
mix("hat", #{ pan: 0.15 });
mix("ohat", #{ pan: -0.1 });
mix("bass", #{ duck: 0.6 });
mix("chords", #{ reverb: 0.43953097, delay: 0.18318664, duck: 0.5 });
mix("pad", #{ reverb: 0.4086404, duck: 0.4 });
mix("lead", #{ reverb: 0.42610887, delay: 0.22258927, duck: 0.3 });
mix("arp", #{ reverb: 0.39236584, delay: 0.41519445, duck: 0.4 });
mix("fx", #{ level: -2, reverb: 0.35 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [#{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 }]);
fx("chords", [
  #{ fx: "width", mono_below_hz: 200, width: 1.5 },
  #{ fx: "chorus", delay_ms: 10.380553, depth_ms: 1.6460402, mix: 0.26222828, rate_hz: 0.71941394 },
]);
fx("pad", [#{ fx: "phaser", depth: 0.69981074, feedback: 0.37219805, mix: sections(0.2858641, #{ drop: 0.42805505, hook: 0.42805505 }), rate_hz: 0.14278676, stages: 6 }]);
fx("lead", [#{ fx: "flanger", delay_ms: 3.3502333, depth_ms: 2.713053, feedback: 0.49436772, mix: 0.4030139, rate_hz: 0.25133887 }]);
bus("bass", [#{ fx: "saturate", drive_db: 2, kind: "tanh", mix: 0.3 }]);
bus("drums", [#{ fx: "compressor", attack_ms: 6, knee_db: 6, makeup_db: 1, mix: 1, ratio: 2.5, release_ms: 80, threshold_db: -10 }]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 160, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 26, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 20, knee_db: 6, makeup_db: 0, mix: 1, ratio: 2, release_ms: 140, threshold_db: -9 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.3676444, decay: 3.237143, lowcut: 328.14508, mix: 1, predelay_ms: 30.852709, width: 1.5 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.5, feedback: 0.33614263, mix: 1, pingpong: true } });

// Glass Garden — tech_house (seed 7)
// song("tech_house", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(124); key("D minor"); swing(0.5103269);
humanize(0.15);
prog("A", "i i i VI");
prog("B", "i i VI VI");
bars_per_chord(2);
master(#{ lufs: -8.5, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X...X...X...X...");
drum("clap", "main", "....X.......X...");
drum("hat", "main", "..x...x...x...x.");
drum("hat", "sparse", "..x...x...x...x.");
drum("ohat", "main", "................");
drum("shaker", "main", "...oo.ooo.....o.");
drum("perc", "main", "...x..........x.|...x..........x.|...x..........x.|...x.....x....x.");
drum_sound("perc", "rim");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "tech", preset: "organ_bass", register: [28, 45], density: 0.5726087, seed: 2642474102756894640 });
part("stabs", #{ style: "house_piano", preset: "stab", register: [52, 72], density: 0.5272447, seed: 5396775171407160694 });

form([
  sec("intro", 4, "kick hat shaker perc").energy(0.59030193),
  sec("groove", 8, "kick clap hat ohat shaker perc bass").energy(0.61373895).fills(false),
  sec("build", 4, "clap hat shaker perc bass stabs").energy(0.59078664).gap(),
  sec("drop", 16, "*").energy(0.9211304).impact(),
  sec("break", 8, "hat perc stabs pad").energy(0.37666035),
  sec("build", 4, "clap hat shaker perc stabs pad").name("build2").energy(0.6387422).gap(),
  sec("drop", 8, "kick clap hat ohat shaker perc bass stabs pad").name("drop2").energy(0.98664886).impact(),
  sec("drop", 8, "kick clap hat ohat shaker perc bass stabs pad").name("drop3").energy(1),
  sec("outro", 4, "kick hat shaker perc").energy(0.40374148),
]);

mix("clap", #{ reverb: 0.15 });
mix("hat", #{ pan: 0.15 });
mix("ohat", #{ pan: -0.1 });
mix("shaker", #{ pan: -0.3 });
mix("perc", #{ pan: 0.35, delay: 0.1 });
mix("bass", #{ duck: 0.5 });
mix("stabs", #{ reverb: 0.29302064, delay: 0.24424885, duck: 0.4 });
mix("fx", #{ level: -2, reverb: 0.3 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [#{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 }]);
fx("stabs", [#{ fx: "chorus", delay_ms: 10.380553, depth_ms: 1.6460402, mix: 0.26222828, rate_hz: 0.71941394 }]);
bus("bass", [#{ fx: "saturate", drive_db: 3, kind: "tanh", mix: 0.4 }]);
bus("drums", [#{ fx: "compressor", attack_ms: 5, knee_db: 6, makeup_db: 2, mix: 1, ratio: 3, release_ms: 60, threshold_db: -10 }]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 150, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 30, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 15, knee_db: 6, makeup_db: 0, mix: 1, ratio: 2, release_ms: 120, threshold_db: -9 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.5176444, decay: 1.2948573, lowcut: 328.14508, mix: 1, predelay_ms: 20.852709, width: 1.5 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.6, feedback: 0.25210696, mix: 1, pingpong: true } });

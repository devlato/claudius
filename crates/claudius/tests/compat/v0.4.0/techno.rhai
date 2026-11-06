// Glass Garden — techno (seed 7)
// song("techno", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(126); key("D minor"); swing(0.5034423);
humanize(0.1);
prog("A", "i i i VI");
prog("B", "i i VI VI");
bars_per_chord(4);
master(#{ lufs: -9, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X...X...X...X...");
drum("hat", "main", "..x...x...x...x.");
drum("hat", "sparse", "..x...x...x...x.");
drum("ohat", "main", "..............x.");
drum("clap", "main", "....X.......X...");
drum("ride", "main", "ooo.ooo.o.o.ooo.");
drum("perc", "main", "..x.....x...x...");
drum("perc", "sparse", "............x...");
drum_sound("perc", "tom");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "rolling", preset: "sub_bass", register: [30, 45], density: 0.53341293, seed: -2526400537787473278 });
part("chords", #{ style: "offbeat", preset: "stab", register: [52, 70], density: 0.4710492, seed: -6710496186308540014 });
part("pad", #{ style: "sustain", preset: "strings", register: [45, 70], density: 0.54910505, seed: -3681901744659677821 });
part("lead", #{ style: "motif", preset: "harp_pluck", register: [62, 82], density: 0.20566142, seed: 805921219908132358 });

form([
  sec("intro", 4, "kick hat perc top").energy(0.59030193),
  sec("groove", 16, "kick hat ohat clap perc ride bass top").energy(0.66373897).fills(false),
  sec("break", 8, "pad chords lead ride").energy(0.3407866).prog("A"),
  sec("build", 4, "clap hat ride chords pad").energy(0.57113045).gap(),
  sec("drop", 16, "kick hat ohat clap perc ride bass acid chords top").name("peak").energy(0.9766603).impact(),
  sec("drop", 16, "kick hat ohat clap perc ride bass acid chords top").name("peak2").energy(0.98874223),
  sec("outro", 8, "kick hat perc ride").energy(0.43664882),
]);

mix("hat", #{ pan: 0.1 });
mix("ohat", #{ pan: -0.15 });
mix("clap", #{ reverb: 0.25 });
mix("ride", #{ pan: 0.3 });
mix("perc", #{ pan: -0.35, reverb: 0.15, delay: 0.2 });
mix("bass", #{ duck: 0.65 });
mix("chords", #{ reverb: 0.5127861, delay: 0.36637327, duck: 0.45 });
mix("pad", #{ reverb: 0.36777636, duck: 0.5 });
mix("lead", #{ reverb: 0.4869816, delay: 0.31162497 });
mix("fx", #{ reverb: 0.3 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [#{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 }]);
fx("chords", [#{ fx: "chorus", delay_ms: 10.380553, depth_ms: 1.6460402, mix: 0.26222828, rate_hz: 0.71941394 }]);
fx("pad", [#{ fx: "phaser", depth: 0.69981074, feedback: 0.37219805, mix: sections(0.2858641, #{ drop: 0.42805505, hook: 0.42805505 }), rate_hz: 0.14278676, stages: 6 }]);
fx("lead", [#{ fx: "flanger", delay_ms: 3.3502333, depth_ms: 2.713053, feedback: 0.49436772, mix: 0.4030139, rate_hz: 0.25133887 }]);
bus("drums", [
  #{ fx: "compressor", attack_ms: 10, knee_db: 6, makeup_db: 1.5, mix: 1, ratio: 2.5, release_ms: 100, threshold_db: -12 },
  #{ fx: "saturate", drive_db: 2, kind: "tanh", mix: 0.3 },
]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 130, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 28, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 25, knee_db: 6, makeup_db: 0, mix: 1, ratio: 2, release_ms: 160, threshold_db: -10 },
  #{ fx: "saturate", drive_db: 3, kind: "tanh", mix: 0.4 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.3676444, decay: 3.237143, lowcut: 328.14508, mix: 1, predelay_ms: 35.852707, width: 1.400633 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.55, feedback: 0.37816042, mix: 1, pingpong: true } });

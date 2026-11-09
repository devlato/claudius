// Glass Garden — deep_house (seed 7)
// song("deep_house", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(118); key("D minor"); swing(0.53860575);
humanize(0.35);
prog("A", "i7 iv7 v7 i7");
prog("B", "iv9 v7 i7 VImaj7");
bars_per_chord(1);
master(#{ lufs: -11, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X...X...X...X...");
drum("kick", "sparse", "X...X...X...X...");
drum("clap", "main", "....X.......X...");
lane("clap", #{ late: 5 });
drum("hat", "main", "..xx..x..xxxxxxx");
drum("hat", "sparse", "..x...x...x...x.");
drum("shaker", "main", "o.o.ooo.ooo.o.oo");
lane("shaker", #{ late: -3 });
drum("perc", "main", ".x.x.x..x.x.x.x.");
drum("perc", "sparse", "...x....x.....x.");
lane("perc", #{ late: 4 });
drum_sound("perc", "rim");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "deep", preset: "pluck_bass", register: [33, 50], density: 0.42864978, seed: -37300132614189792 });
part("chords", #{ style: "house_piano", preset: "house_piano", register: [53, 74], density: 0.5272447, seed: 5396775171407160694 });
part("pad", #{ style: "sustain", preset: "strings", register: [50, 76], density: 0.54910505, seed: -3681901744659677821 });
part("lead", #{ style: "organ_riff", preset: "epiano", register: [62, 84], density: 0.3242152, seed: 805921219908132358 });

form([
  sec("intro", 4, "kick hat shaker perc top").energy(0.5403019),
  sec("groove", 8, "kick clap hat shaker perc bass chords top").energy(0.61373895).fills(false),
  sec("break", 8, "pad chords lead shaker").energy(0.3407866),
  sec("build", 4, "clap hat shaker chords lead pad").energy(0.52113044).prog("B"),
  sec("drop", 16, "kick clap hat shaker perc bass chords lead pad top").energy(0.8766603),
  sec("drop", 16, "kick clap hat shaker perc bass chords lead pad top").name("drop2").energy(0.98874223),
  sec("outro", 8, "kick hat shaker perc bass").energy(0.38664883),
]);

mix("clap", #{ reverb: 0.3 });
mix("hat", #{ pan: 0.18 });
mix("shaker", #{ pan: -0.3 });
mix("perc", #{ pan: 0.4, reverb: 0.1, delay: 0.2 });
mix("bass", #{ duck: 0.45 });
mix("chords", #{ reverb: 0.43953097, delay: 0.122124426, duck: 0.35 });
mix("pad", #{ reverb: 0.32691234, duck: 0.4 });
mix("lead", #{ reverb: 0.42610887, delay: 0.22258927, duck: 0.15 });
mix("fx", #{ level: -3, reverb: 0.3 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [#{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 }]);
fx("chords", [#{ fx: "chorus", delay_ms: 11, depth_ms: 2.5, mix: 0.4, rate_hz: 0.45 }]);
fx("pad", [#{ fx: "phaser", depth: 0.69981074, feedback: 0.37219805, mix: sections(0.2858641, #{ drop: 0.42805505, hook: 0.42805505 }), rate_hz: 0.14278676, stages: 6 }]);
fx("lead", [#{ fx: "flanger", delay_ms: 3.3502333, depth_ms: 2.713053, feedback: 0.49436772, mix: 0.4030139, rate_hz: 0.25133887 }]);
bus("drums", [#{ fx: "compressor", attack_ms: 15, knee_db: 6, makeup_db: 1, mix: 1, ratio: 1.8, release_ms: 140, threshold_db: -14 }]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 100, gain_db: 0, kind: "highpass", q: 0.7 }, #{ freq: 9000, gain_db: -1.5, kind: "highshelf", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 25, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 35, knee_db: 6, makeup_db: 0, mix: 1, ratio: 1.5, release_ms: 220, threshold_db: -11 },
  #{ fx: "saturate", drive_db: 2.5, kind: "tape", mix: 0.4 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.46764442, decay: 2.8055239, lowcut: 262.51608, mix: 1, predelay_ms: 30.852709, width: 1.400633 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.6, feedback: 0.25210696, mix: 1, pingpong: true } });

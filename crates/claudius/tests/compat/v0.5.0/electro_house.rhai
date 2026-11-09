// Glass Garden — electro_house (seed 7)
// song("electro_house", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(126); key("D minor"); swing(0.5034423);
humanize(0.05);
prog("A", "i VI VII v");
prog("B", "i VI III VII");
bars_per_chord(1);
master(#{ lufs: -8, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X...X...X...X...");
drum("clap", "main", "....X.......X...");
drum("hat", "main", "..x...x...x...x.");
drum("hat", "sparse", "..x...x...x...x.");
drum("ohat", "main", "..o...x...o...x.");
drum("perc", "main", "..x..x..x...x..x");
drum("perc", "sparse", "....x.......x...");
drum_sound("perc", "rim");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "syncopated", preset: "reese", register: [38, 57], density: 0.42864978, seed: -37300132614189792 });
part("chords", #{ style: "stabs", preset: "supersaw", register: [55, 74], density: 0.5272447, seed: 5396775171407160694 });
part("lead", #{ style: "riff", preset: "saw_lead", register: [64, 84], density: 0.60010093, seed: -1629742732023437997 });
part("pad", #{ style: "sustain", preset: "strings", register: [50, 76], density: 0.47421524, seed: 805921219908132358 });

form([
  sec("intro", 4, "kick hat perc bass").energy(0.59030193),
  sec("groove", 8, "kick clap hat ohat perc bass chords lead").energy(0.713739).fills(false),
  sec("build", 4, "clap hat chords lead").energy(0.6407866).prog("B").gap(),
  sec("drop", 8, "kick clap hat ohat perc bass chords lead").energy(0.97113043).impact(),
  sec("break", 8, "pad chords arp").energy(0.32666034),
  sec("build", 4, "clap hat pad lead arp").name("build2").energy(0.6887422).prog("B").gap(),
  sec("drop", 12, "kick clap hat ohat perc bass chords lead arp").name("drop2").energy(0.98664886).impact(),
  sec("drop", 12, "kick clap hat ohat perc bass chords lead arp").name("drop3").energy(1),
  sec("outro", 4, "kick hat perc bass").energy(0.40374148),
]);

mix("clap", #{ reverb: 0.18 });
mix("hat", #{ pan: 0.1 });
mix("ohat", #{ pan: -0.1 });
mix("perc", #{ pan: 0.35, delay: 0.15 });
mix("bass", #{ duck: 0.75 });
mix("chords", #{ reverb: 0.3662758, delay: 0.18318664, duck: 0.7 });
mix("lead", #{ reverb: 0.2043202, delay: 0.31054208, duck: 0.4 });
mix("pad", #{ reverb: 0.4869816, duck: 0.5 });
mix("fx", #{ reverb: 0.3 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [
  #{ fx: "parallel", chains: [#{ amount: sections(0.25, #{ drop: 0.45 }), fx: [#{ fx: "saturate", drive_db: 9, kind: "fold", mix: 1 }, #{ fx: "filter", cutoff: 600, drive: 1, mix: 1, mode: "hp", res: 0 }, #{ fx: "filter", cutoff: sections(2500, #{ build: [2500, 7000], drop: 7000 }), drive: 1, mix: 1, mode: "lp", res: 0.2 }] }], dry: 1 },
  #{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 },
]);
fx("chords", [#{ fx: "chorus", delay_ms: 10.380553, depth_ms: 1.6460402, mix: 0.26222828, rate_hz: 0.71941394 }]);
fx("lead", [#{ fx: "saturate", drive_db: 4, kind: "tanh", mix: 0.3 }]);
fx("pad", []);
bus("bass", [#{ fx: "saturate", drive_db: 2, kind: "tube", mix: 0.3 }]);
bus("drums", [
  #{ fx: "compressor", attack_ms: 8, knee_db: 4, makeup_db: 1.5, mix: 1, ratio: 3, release_ms: 90, threshold_db: -11 },
  #{ fx: "saturate", drive_db: 2, kind: "tanh", mix: 0.25 },
]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 150, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 30, gain_db: 0, kind: "highpass", q: 0.7 }, #{ freq: 10000, gain_db: 1, kind: "highshelf", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 20, knee_db: 6, makeup_db: 0, mix: 1, ratio: 1.8, release_ms: 140, threshold_db: -10 },
  #{ fx: "saturate", drive_db: 1.5, kind: "tanh", mix: 0.25 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.3676444, decay: 2.1580954, lowcut: 328.14508, mix: 1, predelay_ms: 25.852709, width: 1.400633 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.45, feedback: 0.29412478, mix: 1, pingpong: true } });

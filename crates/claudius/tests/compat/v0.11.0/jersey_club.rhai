// Glass Garden — jersey_club (seed 7)
// song("jersey_club", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(136); key("D minor"); swing(0.5034423);
humanize(0.04);
prog("A", "i iv V7 i");
prog("B", "i VI iv V");
bars_per_chord(1);
master(#{ lufs: -8, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "alt", "X...X...X..X....");
drum("kick", "main", "X...X...X..X..X.");
drum("snare", "main", "........X..X....");
drum("hat", "main", "..x.x.......x...");
drum_sound("squeak", "perc");
drum("squeak", "main", "..x...x......x..");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "kick_follow", preset: "sub_bass", register: [24, 40], density: 0.53341293, seed: -2526400537787473278 });
part("chords", #{ style: "syncopated", preset: "stab", register: [55, 76], density: 0.55524325, seed: 6553130387488681005 });
part("chop", #{ style: "riff", preset: "vox_lead", register: [62, 79], density: 0.44522208, seed: -4694233468511451402 });
part("pad", #{ style: "sustain", preset: "glass_pad", register: [48, 72], density: 0.49242017, seed: 6845145886153731458 });

form([
  sec("intro", 8, "chords chop hat").energy(0.59030193).variant("main"),
  sec("hook", 8, "kick snare hat squeak bass chords chop").energy(0.91373897).impact(),
  sec("verse", 8, "kick snare hat squeak bass chords").energy(0.7407866).variant("alt").gap(),
  sec("hook", 8, "*").name("hook2").energy(0.97113043).impact(),
  sec("break", 4, "chords chop pad").energy(0.42666033),
  sec("hook", 8, "*").name("hook3").energy(0.98874223).gap().impact(),
  sec("hook", 8, "*").name("hook4").energy(0.98664886),
  sec("outro", 4, "chords chop hat").energy(0.4076398).variant("main"),
]);

mix("snare", #{ reverb: 0.12 });
mix("hat", #{ pan: 0.15 });
mix("squeak", #{ pan: 0.25, delay: 0.1 });
mix("chords", #{ reverb: 0.3662758, delay: 0.18318664, duck: 0.3 });
mix("chop", #{ reverb: 0.16345617, delay: 0.31054208 });
mix("pad", #{ level: -4, reverb: 0.4869816 });
mix("fx", #{ level: -3, reverb: 0.2 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [
  #{ fx: "saturate", drive_db: 6, kind: "tanh", mix: 0.8 },
  #{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 },
]);
fx("chords", [#{ fx: "chorus", delay_ms: 10.380553, depth_ms: 1.6460402, mix: 0.26222828, rate_hz: 0.71941394 }]);
fx("chop", [#{ fx: "saturate", drive_db: 5.1466026, kind: "tube", mix: 0.32567206 }]);
fx("pad", []);
bus("bass", [#{ fx: "saturate", drive_db: 4, kind: "tanh", mix: 0.5 }]);
bus("drums", [
  #{ fx: "compressor", attack_ms: 5, knee_db: 4, makeup_db: 1.5, mix: 1, ratio: 3, release_ms: 70, threshold_db: -10 },
  #{ fx: "saturate", drive_db: 3, kind: "tanh", mix: 0.3 },
]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 150, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 25, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 10, knee_db: 6, makeup_db: 0, mix: 1, ratio: 2.2, release_ms: 100, threshold_db: -9 },
  #{ fx: "saturate", drive_db: 1.5, kind: "tanh", mix: 0.3 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.4176444, decay: 1.5106667, lowcut: 281.2672, mix: 1, predelay_ms: 15.852709, width: 1.400633 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.5, feedback: 0.25210696, mix: 1, pingpong: true } });

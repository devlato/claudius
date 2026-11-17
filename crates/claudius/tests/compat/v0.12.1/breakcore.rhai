// Glass Garden — breakcore (seed 7)
// song("breakcore", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(166); key("D minor"); swing(0.5034423);
humanize(0.15);
prog("A", "i VI VII v");
prog("B", "i VI III VII");
bars_per_chord(1);
master(#{ lufs: -8.5, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X.X.......XX....|X.X.......X..X..|X.X.......XX....|X.X.......X..X..");
drum("snare", "main", "......o.....X........o..o..X........X...........|...o..o.....X........X..o.xx.xx.xx.XX.....X..o..|............X.....o..X.....X..............X.xx.x|......o..o..X.....o.....XXXxxxxxxxxxX.....X..X..", 48);
drum("hat", "main", "xox.xox.x.x.x.x.");
drum("ride", "main", "o...o.o.o.o.o.o.");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "reese", preset: "dist_bass", register: [24, 40], density: 0.42864978, seed: -37300132614189792 });
part("pad", #{ style: "sustain", preset: "air_pad", register: [48, 76], density: 0.5272447, seed: 5396775171407160694 });
part("lead", #{ style: "lofi_melody", preset: "house_piano", register: [67, 86], density: 0.44910502, seed: -3681901744659677821 });
part("chords", #{ style: "sustain", preset: "keys", register: [52, 74], density: 0.47421524, seed: 805921219908132358 });

form([
  sec("intro", 4, "pad lead").energy(0.4903019).variant("main"),
  sec("drop", 16, "kick snare hat ride bass pad lead").energy(0.91373897).fills(false).impact(),
  sec("break", 8, "pad lead chords").energy(0.3407866).build(true).gap(),
  sec("drop", 16, "*").name("drop2").energy(0.97113043).impact(),
  sec("break", 4, "lead pad").name("break2").energy(0.37666035).prog("A"),
  sec("drop", 16, "*").name("drop3").energy(0.98874223).gap().impact(),
  sec("drop", 16, "*").name("drop4").energy(0.98664886),
  sec("outro", 4, "pad lead").energy(0.3576398).variant("main"),
]);

mix("snare", #{ reverb: 0.12 });
mix("hat", #{ pan: 0.2 });
mix("ride", #{ pan: -0.25, reverb: 0.15 });
mix("bass", #{ duck: 0.4 });
mix("pad", #{ reverb: 0.7325516, duck: 0.3 });
mix("lead", #{ reverb: 0.32691234, delay: 0.4658131, duck: 0.2 });
mix("chords", #{ reverb: 0.4869816, delay: 0.26710713, duck: 0.3 });
mix("fx", #{ level: -2, reverb: 0.3 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("kick", [#{ fx: "saturate", drive_db: 6, kind: "hard", mix: 0.4 }]);
fx("bass", [
  #{ fx: "saturate", drive_db: 6, kind: "tube", mix: 0.5 },
  #{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 },
]);
fx("pad", [#{ fx: "chorus", delay_ms: 10.380553, depth_ms: 1.6460402, mix: 0.26222828, rate_hz: 0.71941394 }]);
fx("lead", [#{ fx: "saturate", drive_db: 5.1466026, kind: "tube", mix: 0.32567206 }]);
fx("chords", [#{ fx: "chorus", delay_ms: 11, depth_ms: 2.5, mix: 0.4, rate_hz: 0.45 }]);
bus("bass", [#{ fx: "saturate", drive_db: 3, kind: "tube", mix: 0.4 }]);
bus("drums", [
  #{ fx: "saturate", drive_db: 4, kind: "hard", mix: 0.4 },
  #{ fx: "compressor", attack_ms: 5, knee_db: 6, makeup_db: 2, mix: 1, ratio: 3, release_ms: 70, threshold_db: -12 },
]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 150, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 26, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 15, knee_db: 6, makeup_db: 0, mix: 1, ratio: 2, release_ms: 120, threshold_db: -9 },
  #{ fx: "saturate", drive_db: 2, kind: "hard", mix: 0.3 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.4176444, decay: 2.8055239, lowcut: 328.14508, mix: 1, predelay_ms: 25.852709, width: 1.5 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.55, feedback: 0.33614263, mix: 1, pingpong: true } });

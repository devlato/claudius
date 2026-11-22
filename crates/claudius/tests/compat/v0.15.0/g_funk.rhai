// Glass Garden — g_funk (seed 7)
// song("g_funk", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(89); key("D dorian"); swing(0.5586058);
humanize(0.35);
prog("A", "i7 IV9 i7 VIImaj7");
prog("B", "i7 VIImaj7 IV7 IV7");
bars_per_chord(2);
master(#{ lufs: -10, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "alt", "X.....X...X.....|X.........X.....|X.....X...X.....|X.........X.....");
drum("kick", "main", "X......X..X.....");
lane("kick", #{ late: -1 });
drum("snare", "main", "....X.......X...");
lane("snare", #{ late: 7 });
drum("clap", "main", "....X.......X...");
lane("clap", #{ late: 13 });
drum("hat", "main", "x.xxx.x.xxx.x.x.");
lane("hat", #{ late: 1 });
drum("ohat", "main", "..............x.");
lane("ohat", #{ late: 3 });
drum("shaker", "main", "x.....x...o...x.");
lane("shaker", #{ late: 6 });

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "funk", preset: "deep_bass", register: [29, 49], density: 0.42864978, seed: -37300132614189792, late: 11 });
part("keys", #{ style: "gospel", preset: "epiano", register: [47, 71], density: 0.5272447, seed: 5396775171407160694 });
part("lead", #{ style: "rnb_runs", preset: "glide_lead", register: [68, 89], density: 0.32006055, seed: -1629742732023437997 });
part("talkbox", #{ style: "rnb_runs", preset: "vox_lead", register: [51, 68], density: 0.25655335, seed: 6104030045851553230 });

form([
  sec("intro", 8, "keys lead hat").energy(0.5403019).variant("main"),
  sec("verse", 8, "kick snare clap hat shaker bass keys").energy(0.61373895),
  sec("hook", 4, "kick snare clap hat ohat shaker bass keys lead strings talkbox").energy(0.84078664),
  sec("verse", 8, "kick snare clap hat shaker bass keys").name("verse2").energy(0.6211304).variant("alt").fills(true).gap(),
  sec("hook", 4, "kick snare clap hat ohat shaker bass keys lead strings talkbox").name("hook2").energy(0.8766603),
  sec("break", 4, "hat bass lead strings").name("bridge").energy(0.4387422).gap(),
  sec("hook", 8, "kick snare clap hat ohat shaker bass keys lead strings talkbox").name("hook3").energy(0.93664885).gap(),
  sec("hook", 8, "kick snare clap hat ohat shaker bass keys lead strings talkbox").name("hook4").energy(1),
  sec("outro", 8, "hat bass keys lead").energy(0.3537415).variant("main"),
]);

mix("snare", #{ reverb: 0.15 });
mix("clap", #{ level: -3, reverb: 0.22 });
mix("hat", #{ pan: 0.15 });
mix("ohat", #{ pan: -0.15 });
mix("shaker", #{ level: -3, pan: -0.35 });
mix("keys", #{ reverb: 0.3223227 });
mix("lead", #{ level: -4, reverb: 0.24518426, delay: 0.31054208 });
mix("talkbox", #{ level: -5, reverb: 0.2434908, delay: 0.08903571 });
mix("fx", #{ level: -4, reverb: 0.25 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [
  #{ fx: "saturate", drive_db: 3, kind: "tube", mix: 0.5 },
  #{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 },
]);
fx("keys", [#{ fx: "chorus", delay_ms: 10, depth_ms: 2, mix: 0.25, rate_hz: 0.5 }]);
fx("lead", [#{ fx: "saturate", drive_db: 5.1466026, kind: "tube", mix: 0.32567206 }]);
bus("bass", [#{ fx: "saturate", drive_db: 3, kind: "tanh", mix: 0.4 }]);
bus("drums", [
  #{ fx: "saturate", drive_db: 3, kind: "tube", mix: 0.4 },
  #{ fx: "compressor", attack_ms: 11.170334, knee_db: 6, makeup_db: 2, mix: 1, ratio: 3.655613, release_ms: 121.50812, threshold_db: -11.541128 },
]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 110, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 26, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "tape", drive_db: 1.5, flutter: 0.06, lowpass_hz: 18000, wow: 0.06 },
  #{ fx: "compressor", attack_ms: 20, knee_db: 6, makeup_db: 0, mix: 1, ratio: 2, release_ms: 150, threshold_db: -11 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.46764442, decay: 1.7264763, lowcut: 262.51608, mix: 1, predelay_ms: 20.852709, width: 1.5 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.6, feedback: 0.25210696, mix: 1, pingpong: true } });

// Glass Garden — new_jack_swing (seed 7)
// song("new_jack_swing", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(101); key("D minor"); swing(0.5903269);
humanize(0.08);
prog("A", "i III VII IV");
prog("B", "i VI VII i");
bars_per_chord(1);
master(#{ lufs: -10, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "alt", "X.......X..X....|X.......X.......|X.......X..X....|X.......X.......");
drum("kick", "main", "X......XX.......");
drum("snare", "main", "....X.......X...");
drum("clap", "main", "....X.......X...");
drum("hat", "main", "x.xxx.xxx.x.xxxx");
drum("ohat", "main", "..............x.");
drum("perc", "main", "......o.........");
drum_sound("perc", "rim");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "new_jack", preset: "moog_bass", register: [28, 48], density: 0.5995956, seed: 3796763344308755361 });
part("keys", #{ style: "gospel", preset: "tine_piano", register: [52, 76], density: 0.45851198, seed: 6674420348178494801 });
part("stabs", #{ style: "stabs", preset: "tine_piano", register: [55, 76], density: 0.5823303, seed: -7892568474930094457 });

form([
  sec("intro", 8, "stabs hat keys").energy(0.59030193).variant("main"),
  sec("verse", 8, "kick snare clap hat perc bass keys").energy(0.66373897),
  sec("hook", 4, "kick snare clap hat ohat perc bass keys stabs lead").energy(0.8907866).impact(),
  sec("verse", 8, "kick snare clap hat perc bass keys stabs").name("verse2").energy(0.69113046).variant("alt").fills(true).gap(),
  sec("hook", 4, "kick snare clap hat ohat perc bass keys stabs lead pad").name("hook2").energy(0.8966603).impact(),
  sec("break", 4, "kick clap hat bass stabs").name("breakdown").energy(0.4887422).gap(),
  sec("hook", 8, "kick snare clap hat ohat perc bass keys stabs lead pad").name("hook3").energy(0.93664885).gap().impact(),
  sec("hook", 8, "kick snare clap hat ohat perc bass keys stabs lead pad").name("hook4").energy(1),
  sec("outro", 8, "kick clap hat bass stabs").energy(0.40374148).variant("main"),
]);

mix("snare", #{ reverb: 0.3 });
mix("clap", #{ level: -2, reverb: 0.3 });
mix("hat", #{ pan: 0.15 });
mix("ohat", #{ pan: -0.15 });
mix("perc", #{ level: -6, pan: -0.3 });
mix("keys", #{ reverb: 0.3662758 });
mix("stabs", #{ level: -2, reverb: 0.2043202 });
mix("fx", #{ level: -4, reverb: 0.3 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [
  #{ fx: "saturate", drive_db: 3, kind: "tanh", mix: 0.5 },
  #{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 },
]);
fx("keys", [#{ fx: "chorus", delay_ms: 10, depth_ms: 2.5, mix: 0.3, rate_hz: 0.7 }]);
fx("stabs", [
  #{ fx: "chorus", delay_ms: 11, depth_ms: 2.5, mix: 0.4, rate_hz: 0.45 },
  #{ fx: "phaser", depth: 0.69981074, feedback: 0.37219805, mix: sections(0.2858641, #{ drop: 0.42805505, hook: 0.42805505 }), rate_hz: 0.14278676, stages: 6 },
]);
bus("bass", [#{ fx: "saturate", drive_db: 2, kind: "tanh", mix: 0.4 }]);
bus("drums", [#{ fx: "compressor", attack_ms: 6, knee_db: 6, makeup_db: 2, mix: 1, ratio: 3, release_ms: 90, threshold_db: -11 }]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 120, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 28, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 15, knee_db: 6, makeup_db: 0, mix: 1, ratio: 2.2, release_ms: 130, threshold_db: -10 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.3676444, decay: 1.6185715, lowcut: 281.2672, mix: 1, predelay_ms: 15.852709, width: 1.5 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.5, feedback: 0.21008913, mix: 1, pingpong: true } });

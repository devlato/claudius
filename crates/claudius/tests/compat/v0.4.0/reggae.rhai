// Glass Garden — reggae (seed 7)
// song("reggae", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(67); key("D minor"); swing(0.5737693);
humanize(0.4);
prog("A", "i i i VI");
prog("B", "i i VI VI");
bars_per_chord(1);
master(#{ lufs: -11, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "........X.......");
drum("rim", "main", "........X....o..");
drum("hat", "main", "x.x.x.x.x.x.x.xx|x.x.x.x.x.x.x.xx|x.x.x.x.x.x.x.xx|x.x.x.x.x.x.x.x.");
drum("ohat", "main", "..............x.");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "syncopated", preset: "deep_bass", register: [28, 46], density: 0.42864978, seed: -37300132614189792 });
part("guitar", #{ style: "offbeat", preset: "pluck", register: [55, 76], density: 0.5272447, seed: 5396775171407160694 });
part("organ", #{ style: "syncopated", preset: "rock_organ", register: [50, 72], density: 0.5144286, seed: 336892620131319353 });

form([
  sec("intro", 8, "kick rim hat guitar organ").energy(0.34030193).variant("main").filter_in(),
  sec("verse", 8, "kick rim hat bass guitar organ").energy(0.563739),
  sec("hook", 4, "kick rim hat ohat bass guitar organ horns").energy(0.7907866),
  sec("verse", 4, "kick rim hat bass guitar organ").name("verse2").energy(0.6211304).fills(true).gap(),
  sec("hook", 4, "kick rim hat ohat bass guitar organ horns").name("hook2").energy(0.82666034),
  sec("break", 4, "kick rim bass").name("dub").energy(0.4387422).gap(),
  sec("hook", 4, "kick rim hat ohat bass guitar organ horns").name("hook3").energy(0.8366489).gap(),
  sec("hook", 4, "kick rim hat ohat bass guitar organ horns").name("hook4").energy(1),
  sec("outro", 4, "kick rim hat guitar organ").energy(0.3537415).variant("main"),
]);

mix("rim", #{ reverb: 0.3, delay: 0.15 });
mix("hat", #{ pan: 0.2 });
mix("ohat", #{ pan: -0.15 });
mix("guitar", #{ pan: 0.3, reverb: 0.29302064, delay: 0.122124426 });
mix("organ", #{ pan: -0.3, reverb: 0.2043202 });
mix("fx", #{ reverb: 0.3 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [
  #{ fx: "compressor", attack_ms: 15, knee_db: 6, makeup_db: 1, mix: 1, ratio: 3, release_ms: 120, threshold_db: -16 },
  #{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 },
]);
fx("guitar", [
  #{ fx: "filter", cutoff: 300, drive: 1, mix: 1, mode: "hp", res: 0.05 },
  #{ fx: "chorus", delay_ms: 10.380553, depth_ms: 1.6460402, mix: 0.26222828, rate_hz: 0.71941394 },
]);
fx("organ", [
  #{ fx: "chorus", delay_ms: 11, depth_ms: 2.5, mix: 0.4, rate_hz: 0.45 },
  #{ fx: "phaser", depth: 0.69981074, feedback: 0.37219805, mix: sections(0.2858641, #{ drop: 0.42805505, hook: 0.42805505 }), rate_hz: 0.14278676, stages: 6 },
]);
bus("bass", [#{ fx: "saturate", drive_db: 2, kind: "tube", mix: 0.3 }]);
bus("drums", [#{ fx: "compressor", attack_ms: 10, knee_db: 6, makeup_db: 1, mix: 1, ratio: 2.5, release_ms: 100, threshold_db: -14 }]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 120, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 28, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 20, knee_db: 6, makeup_db: 0, mix: 1, ratio: 1.8, release_ms: 160, threshold_db: -12 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.31764442, decay: 1.7264763, lowcut: 281.2672, mix: 1, predelay_ms: 15.852709, width: 1.400633 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.55, feedback: 0.37816042, mix: 1, pingpong: false } });

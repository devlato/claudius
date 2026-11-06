// Glass Garden — ambient (seed 7)
// song("ambient", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(62); key("D minor"); swing(0.5034423);
humanize(0.4);
prog("A", "i i i VI");
prog("B", "i i VI VI");
bars_per_chord(2);
master(#{ lufs: -14, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "x...............|........x.......|x...............|........x.......");
drum("shaker", "main", "..o....o..o..oo.");
drum("perc", "main", "......o.........");
drum_sound("perc", "rim");

// parts written by the theory engine: change the style, preset, register, density or seed
part("pad", #{ style: "sustain", preset: "air_pad", register: [45, 76], density: 0.42864978, seed: -37300132614189792 });
part("keys", #{ style: "half", preset: "keys", register: [55, 80], density: 0.45851198, seed: 6674420348178494801 });
part("lead", #{ style: "motif", preset: "fm_kalimba", register: [62, 86], density: 0.26586422, seed: -7892568474930094457 });

form([
  sec("intro", 8, "pad").energy(0.34030193).variant("main"),
  sec("verse", 8, "pad keys bass").energy(0.31373897).fills(true),
  sec("hook", 8, "pad strings keys lead bass shaker perc").energy(0.59078664).fills(false),
  sec("break", 4, "pad").energy(0.22113045).gap(),
  sec("hook", 4, "pad strings keys lead arp bass kick shaker perc").name("hook2").energy(0.6266603).fills(false).gap(),
  sec("hook", 4, "pad strings keys lead arp bass kick shaker perc").name("hook3").energy(0.98874223).fills(false),
  sec("outro", 4, "pad keys").energy(0.18664883).variant("main"),
]);

mix("kick", #{ level: -4, reverb: 0.2 });
mix("shaker", #{ pan: -0.3, reverb: 0.3 });
mix("perc", #{ pan: 0.4, reverb: 0.4, delay: 0.3 });
mix("pad", #{ reverb: 0.54811853, delay: 0.22325131 });
mix("keys", #{ reverb: 0.7325516, delay: 0.36637327 });
mix("lead", #{ reverb: 0.44950446, delay: 0.6 });
mix("fx", #{ level: -4, reverb: 0.6 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("pad", [#{ fx: "auto_pan", beats: 2, depth: 0.51514673 }]);
fx("keys", [#{ fx: "chorus", delay_ms: 11, depth_ms: 2.5, mix: 0.4, rate_hz: 0.45 }]);
fx("lead", [#{ fx: "tape", drive_db: 1.1752481, flutter: 0.19840398, lowpass_hz: 10785.415, wow: 0.12320626 }]);
bus("drums", [#{ fx: "compressor", attack_ms: 20, knee_db: 6, makeup_db: 0, mix: 1, ratio: 2, release_ms: 150, threshold_db: -16 }]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 60, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 25, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 30, knee_db: 8, makeup_db: 0, mix: 1, ratio: 1.5, release_ms: 250, threshold_db: -16 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.3676444, decay: 4.855715, lowcut: 187.51147, mix: 1, predelay_ms: 45.852707, width: 1.5 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.6, feedback: 0.42017826, mix: 1, pingpong: true } });

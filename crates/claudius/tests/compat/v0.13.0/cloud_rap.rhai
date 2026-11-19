// Glass Garden — cloud_rap (seed 7)
// song("cloud_rap", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(129); key("D minor"); swing(0.5051635);
humanize(0.15);
prog("A", "i VI VII v");
prog("B", "i VI III VII");
bars_per_chord(2);
master(#{ lufs: -10.5, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "alt", "X...............");
drum("kick", "main", "X.........X.....|X.....X.........|X.........X.....|X.....X.........");
drum("snare", "main", "........X.......");
drum_sound("snare", "cp");
drum("hat", "alt", "x.......x.......x.......x.......|x.......x.......x.......x.......|x.......x.......x.......x.......|x.......x...........oxxxx.......", 32);
drum("hat", "main", "x...........x...........x...........x...........|x...........x...........x.................X.xx.x|x...........x...........x...........x...........|x...........ooxxxxxxxxXXx...........x...........", 48);
drum("ohat", "main", "..............x.");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "glide", preset: "sub_bass", register: [24, 40], density: 0.53341293, seed: -2526400537787473278 });
part("pad", #{ style: "sustain", preset: "air_pad", register: [50, 76], density: 0.55524325, seed: 6553130387488681005 });
part("keys", #{ style: "sustain", preset: "house_piano", register: [55, 79], density: 0.54910505, seed: -3681901744659677821 });
part("lead", #{ style: "ambient_line", preset: "vowel_pad", register: [62, 84], density: 0.20566142, seed: 805921219908132358 });

form([
  sec("intro", 8, "pad keys").energy(0.44030192).variant("main"),
  sec("hook", 8, "kick snare hat bass pad keys lead").energy(0.763739),
  sec("verse", 8, "kick snare hat bass pad keys").energy(0.59078664).variant("alt").gap(),
  sec("hook", 8, "*").name("hook2").energy(0.82113045),
  sec("break", 4, "pad lead").name("drift").energy(0.32666034).prog("A"),
  sec("hook", 8, "*").name("hook3").energy(0.8887422).gap(),
  sec("hook", 8, "*").name("hook4").energy(0.98664886),
  sec("outro", 4, "pad keys").energy(0.3076398).variant("main"),
]);

mix("snare", #{ reverb: 0.4 });
mix("hat", #{ pan: 0.15, reverb: 0.15 });
mix("ohat", #{ pan: -0.2, reverb: 0.3 });
mix("pad", #{ reverb: 0.8 });
mix("keys", #{ level: -3, reverb: 0.44950446, delay: 0.31054208 });
mix("lead", #{ level: -4, reverb: 0.79134506, delay: 0.26710713 });
mix("fx", #{ level: -5, reverb: 0.5 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [
  #{ fx: "saturate", drive_db: 3, kind: "tanh", mix: 0.6 },
  #{ fx: "filter", cutoff: 3500, drive: 1, mix: 1, mode: "lp", res: 0.1 },
  #{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 },
]);
fx("pad", [
  #{ fx: "chorus", delay_ms: 14, depth_ms: 3, mix: 0.35, rate_hz: 0.3 },
  #{ fx: "tape", drive_db: 1.5, flutter: 0.1, lowpass_hz: 9000, wow: 0.35 },
]);
fx("keys", [
  #{ fx: "chorus", delay_ms: 11, depth_ms: 2.5, mix: 0.4, rate_hz: 0.45 },
  #{ fx: "phaser", depth: 0.69981074, feedback: 0.37219805, mix: sections(0.2858641, #{ drop: 0.42805505, hook: 0.42805505 }), rate_hz: 0.14278676, stages: 6 },
]);
fx("lead", [#{ fx: "flanger", delay_ms: 3.3502333, depth_ms: 2.713053, feedback: 0.49436772, mix: 0.4030139, rate_hz: 0.25133887 }]);
bus("bass", [#{ fx: "saturate", drive_db: 2, kind: "tanh", mix: 0.4 }]);
bus("drums", [#{ fx: "compressor", attack_ms: 10, knee_db: 6, makeup_db: 1, mix: 1, ratio: 2, release_ms: 120, threshold_db: -12 }]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 130, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 24, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "tape", drive_db: 1.5, flutter: 0.06, lowpass_hz: 15000, wow: 0.12 },
  #{ fx: "compressor", attack_ms: 25, knee_db: 6, makeup_db: 0, mix: 1, ratio: 1.8, release_ms: 180, threshold_db: -11 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.3676444, decay: 4.3161907, lowcut: 234.38934, mix: 1, predelay_ms: 30.852709, width: 1.5 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.6, feedback: 0.33614263, mix: 1, pingpong: true } });

// Glass Garden — alt_hip_hop (seed 7)
// song("alt_hip_hop", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(77); key("D major"); swing(0.55032694);
humanize(0.5);
prog("A", "bVImaj7 bVII9 Imaj9 Imaj9");
prog("B", "IVmaj9 iii7 vi9 ii9 IVmaj9 iv6 Imaj9 Imaj9");
bars_per_chord(1);
master(#{ lufs: -10, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "alt", "X.........X.....");
drum("kick", "half", "X...............|................|X...............|................");
drum("kick", "main", "X......X..X.....");
lane("kick", #{ late: -4 });
drum("snare", "half", "........X.......");
drum("snare", "main", "....X.......X...");
lane("snare", #{ late: 20 });
drum("clap", "half", "........X.......");
drum("clap", "main", "....x.......x...");
lane("clap", #{ late: 20 });
drum("hat", "half", "x...x...x...x.x.");
drum("hat", "main", "x.o.x...x.x.x.xx");
lane("hat", #{ late: -1 });
drum("ohat", "main", "................|..............x.|................|..............x.");
lane("ohat", #{ late: -2 });
drum("rim", "main", "...x..........x.");
lane("rim", #{ late: 16 });
drum("shaker", "main", "xxxxxoxox.x...xx");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "boom_bap", preset: "deep_bass", register: [27, 44], density: 0.42864978, seed: -37300132614189792, late: 13 });
part("keys", #{ style: "lofi_comp", preset: "keys", register: [47, 73], density: 0.5823303, seed: -7892568474930094457 });
part("pad", #{ style: "legato", preset: "strings", register: [44, 70], density: 0.47421524, seed: 805921219908132358 });
part("lead", #{ style: "trap_bells", preset: "brass_lead", register: [59, 81], density: 0.40548947, seed: 6734273059743298459 });
part("strings", #{ style: "legato", preset: "air_pad", register: [53, 79], density: 0.5689536, seed: 5366676654615143931 });
part("fuzz", #{ style: "kick_follow", preset: "moog_bass", register: [36, 53], density: 0.45851198, seed: 6674420348178494801, doubles: "bass", octave: 1 });
part("lead_double", #{ role: "lead", style: "trap_bells", preset: "vox_lead", register: [55, 77], density: 0.40548947, seed: 6734273059743298459, doubles: "lead" });

form([
  sec("intro", 8, "keys pad").energy(0.44030192).variant("main"),
  sec("verse", 8, "kick snare hat rim bass fuzz pad shaker").energy(0.563739).variant("alt"),
  sec("hook", 4, "kick snare clap hat ohat bass fuzz keys pad lead strings lead_double shaker").energy(0.94078666),
  sec("verse", 8, "kick snare hat rim bass fuzz keys strings shaker").name("verse2").energy(0.6211304).variant("alt").fills(true).gap(),
  sec("hook", 4, "kick snare clap hat ohat bass fuzz keys pad lead strings lead_double shaker").name("hook2").energy(0.9766603),
  sec("verse", 4, "kick snare hat bass fuzz lead lead_double shaker").name("switch").energy(0.73874223).prog("B").variant("half").build(true),
  sec("break", 4, "keys strings").name("bridge").energy(0.2866488).gap(),
  sec("hook", 4, "kick snare clap hat ohat bass fuzz keys pad lead strings lead_double shaker").name("hook3").energy(1).gap(),
  sec("hook", 4, "kick snare clap hat ohat bass fuzz keys pad lead strings lead_double shaker").name("hook4").energy(1),
  sec("outro", 4, "keys pad strings").energy(0.31163692).variant("main"),
]);

mix("snare", #{ reverb: 0.15 });
mix("clap", #{ level: -4, reverb: 0.25 });
mix("hat", #{ pan: 0.15 });
mix("ohat", #{ pan: -0.15 });
mix("rim", #{ pan: -0.25, reverb: 0.15 });
mix("shaker", #{ level: -2, pan: 0.3 });
mix("fuzz", #{ level: -8 });
mix("keys", #{ reverb: 0.2043202 });
mix("pad", #{ level: -3, reverb: 0.608727 });
mix("lead", #{ reverb: 0.45776013, delay: 0.2767963 });
mix("strings", #{ level: -2, reverb: 0.6283498 });
mix("lead_double", #{ level: -6, pan: 0.3, reverb: 0.5577601, delay: 0.2767963 });
mix("fx", #{ level: -4, reverb: 0.3 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [
  #{ fx: "saturate", drive_db: 6, kind: "tanh", mix: 0.8 },
  #{ fx: "filter", cutoff: 4500, drive: 1, mix: 1, mode: "lp", res: 0.1 },
  #{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 },
]);
fx("fuzz", [
  #{ fx: "filter", cutoff: 150, drive: 1, mix: 1, mode: "hp", res: 0.05 },
  #{ fx: "saturate", drive_db: 9, kind: "hard", mix: 0.6 },
]);
fx("keys", [#{ fx: "chorus", delay_ms: 12, depth_ms: 2.5, mix: 0.3, rate_hz: 0.4 }]);
fx("pad", [#{ fx: "width", mono_below_hz: 200, width: 1.5 }]);
fx("strings", [#{ fx: "width", mono_below_hz: 200, width: 1.4 }]);
bus("bass", [#{ fx: "saturate", drive_db: 4, kind: "tanh", mix: 0.5 }]);
bus("drums", [
  #{ fx: "saturate", drive_db: 4, kind: "tube", mix: 0.45 },
  #{ fx: "compressor", attack_ms: 11.170334, knee_db: 6, makeup_db: 2, mix: 1, ratio: 3.655613, release_ms: 110.46193, threshold_db: -12.541128 },
]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 110, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 25, gain_db: 0, kind: "highpass", q: 0.7 }] },
  #{ fx: "tape", drive_db: 1.5, flutter: 0.08, lowpass_hz: 18000, wow: 0.08 },
  #{ fx: "compressor", attack_ms: 20, knee_db: 6, makeup_db: 0, mix: 1, ratio: 2, release_ms: 150, threshold_db: -11 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.46764442, decay: 2.1580954, lowcut: 262.51608, mix: 1, predelay_ms: 23.852709, width: 1.5 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.6, feedback: 0.26891407, mix: 1, pingpong: true } });

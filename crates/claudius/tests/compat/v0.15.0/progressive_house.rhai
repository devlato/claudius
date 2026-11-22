// Glass Garden — progressive_house (seed 7)
// song("progressive_house", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(124); key("D minor"); swing(0.5034423);
humanize(0.06);
prog("A", "i VI VII v");
prog("B", "i VI III VII");
bars_per_chord(1);
master(#{ lufs: -8.5, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X...X...X...X...");
drum("clap", "main", "....X.......X...");
drum("hat", "main", "x.x.x.x.x.x.x.x.");
drum("hat", "sparse", "..x...x...x...x.");
drum("ohat", "main", "..x...x...x...x.");
drum("shaker", "main", "..ooo.o..o....oo");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "rolling", register: [34, 51], density: 0.42864978, seed: -37300132614189792 });
part("chords", #{ style: "pump", preset: "stab", register: [53, 77], density: 0.5272447, seed: 5396775171407160694 });
part("piano", #{ style: "house_piano", preset: "house_piano", register: [48, 74], density: 0.5823303, seed: -7892568474930094457 });
part("lead", #{ style: "riff", preset: "pluck", register: [63, 80], density: 0.3587378, seed: 6104030045851553230 });
part("arp", #{ style: "updown", preset: "harp_pluck", register: [59, 85], density: 0.53445977, rate: 0.25, seed: 7280655439012144592 });

form([
  sec("intro", 4, "kick hat shaker bass").energy(0.5403019),
  sec("groove", 8, "kick clap hat ohat shaker bass arp").energy(0.61373895).fills(false),
  sec("break", 8, "piano pad lead").energy(0.3407866),
  sec("build", 4, "clap hat piano pad lead").energy(0.5711304).prog("B").gap(),
  sec("drop", 8, "kick clap hat ohat shaker bass chords lead arp pad").energy(0.9766603).impact(),
  sec("break", 8, "pad arp").name("break2").energy(0.3387422).prog("A"),
  sec("build", 4, "clap hat piano pad arp").name("build2").energy(0.63664883).gap(),
  sec("drop", 16, "kick clap hat ohat shaker bass chords lead arp pad piano").name("drop2").energy(1).impact(),
  sec("drop", 16, "kick clap hat ohat shaker bass chords lead arp pad piano").name("drop3").energy(0.9537415),
  sec("outro", 4, "kick hat shaker bass").energy(0.41163692),
]);

mix("clap", #{ reverb: 0.25 });
mix("hat", #{ pan: 0.12 });
mix("ohat", #{ pan: -0.12 });
mix("shaker", #{ pan: -0.3 });
mix("bass", #{ duck: 0.7 });
mix("chords", #{ reverb: 0.43953097, delay: 0.18318664, duck: 0.55 });
mix("piano", #{ reverb: 0.2860483, delay: 0.15527104, duck: 0.35 });
mix("lead", #{ reverb: 0.42610887, delay: 0.22258927, duck: 0.35 });
mix("arp", #{ pan: 0.15, reverb: 0.39236584, delay: 0.41519445, duck: 0.4 });
mix("fx", #{ level: -2, reverb: 0.35 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [#{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 }]);
fx("chords", [
  #{ fx: "width", mono_below_hz: 200, width: 1.5 },
  #{ fx: "chorus", delay_ms: 10.380553, depth_ms: 1.6460402, mix: 0.26222828, rate_hz: 0.71941394 },
]);
fx("piano", [
  #{ fx: "chorus", delay_ms: 11, depth_ms: 2.5, mix: 0.4, rate_hz: 0.45 },
  #{ fx: "phaser", depth: 0.69981074, feedback: 0.37219805, mix: sections(0.2858641, #{ drop: 0.42805505, hook: 0.42805505 }), rate_hz: 0.14278676, stages: 6 },
]);
fx("lead", [
  #{ fx: "width", mono_below_hz: 200, width: 1.4 },
  #{ fx: "flanger", delay_ms: 3.3502333, depth_ms: 2.713053, feedback: 0.49436772, mix: 0.4030139, rate_hz: 0.25133887 },
]);
bus("bass", [#{ fx: "saturate", drive_db: 2, kind: "tube", mix: 0.3 }]);
bus("drums", [#{ fx: "compressor", attack_ms: 8.936267, knee_db: 6, makeup_db: 1.5, mix: 1, ratio: 3.0463443, release_ms: 99.41573, threshold_db: -10.541128 }]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 150, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 28, gain_db: 0, kind: "highpass", q: 0.7 }, #{ freq: 10000, gain_db: 0.8, kind: "highshelf", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 20, knee_db: 6, makeup_db: 0, mix: 1, ratio: 2, release_ms: 140, threshold_db: -10 },
  #{ fx: "saturate", drive_db: 1.5, kind: "tanh", mix: 0.25 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.3676444, decay: 2.8055239, lowcut: 300.01837, mix: 1, predelay_ms: 30.852709, width: 1.5 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.5, feedback: 0.3193355, mix: 1, pingpong: true } });

// racks: tracks played by several sounds at once, each its band, level, duck and effects
rack("bass", [
  #{ sound: "sub_bass", band: [0, 90], level: -3, duck: 0.35, fold: [28, 40] },
  #{ sound: "pluck_bass", band: [90, 0] },
]);

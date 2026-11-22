// Glass Garden — house (seed 7)
// song("house", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(120); key("D minor"); swing(0.55032694);
humanize(0.25);
prog("A", "i7 iv7 v7 i7");
prog("B", "iv9 v7 i7 VImaj7");
bars_per_chord(1);
master(#{ lufs: -10, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X...X...X...X...");
drum("kick", "sparse", "X...X...X...X...");
drum("clap", "main", "....X.......X...");
lane("clap", #{ late: -4 });
drum("hat", "main", "xx.xxx.xxx.xxx.x");
drum("hat", "sparse", "..x...x...x...x.");
lane("hat", #{ late: -1 });
drum("ohat", "main", "..x...x...x...x.");
drum("shaker", "main", "x.o.o.o.o.o.o.o.");
lane("shaker", #{ late: -4 });
drum("perc", "main", "..x...x...x...x.");
drum("perc", "sparse", ".x.......x......");
drum_sound("perc", "rim");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "offbeat", register: [35, 54], density: 0.53341293, seed: -2526400537787473278 });
part("chords", #{ style: "sustain", preset: "epiano", register: [58, 79], density: 0.55524325, seed: 6553130387488681005 });
part("pad", #{ style: "legato", preset: "pad", register: [51, 77], density: 0.59977853, seed: 4170999698929869575 });
part("lead", #{ style: "garage_chop", preset: "pluck", register: [62, 84], density: 0.41072237, seed: -2284409115224711883 });
part("arp", #{ style: "pinky", preset: "fm_lead", register: [64, 88], density: 0.49820793, rate: 0.5, seed: -4105371792258199394 });

form([
  sec("intro", 8, "kick hat perc").energy(0.4903019),
  sec("intro", 4, "kick hat ohat shaker perc").name("intro2").energy(0.38373896).variant("main"),
  sec("intro", 4, "kick clap hat ohat shaker perc bass").name("intro3").energy(0.4907866).variant("main"),
  sec("intro", 4, "kick clap hat ohat shaker perc bass top").name("intro4").energy(0.53113043).variant("main").fills(true),
  sec("groove", 8, "kick clap hat ohat shaker perc bass chords lead").energy(0.70666033).fills(false),
  sec("groove", 8, "kick clap hat ohat shaker perc bass lead").name("groove2").energy(0.67874223),
  sec("break", 4, "pad lead").energy(0.2866488),
  sec("build", 4, "kick perc chords pad").energy(0.6076398).prog("B").gap(),
  sec("drop", 8, "kick clap hat ohat shaker perc bass chords lead").energy(0.9037415).impact(),
  sec("break", 4, "hat perc pad").name("break2").energy(0.4116369).build(true),
  sec("drop", 8, "kick clap hat ohat shaker perc bass chords arp").name("drop2").energy(0.97864896).impact(),
  sec("drop", 8, "kick clap hat ohat shaker perc bass chords arp").name("drop3").energy(0.9908827),
  sec("outro", 4, "kick clap hat ohat shaker perc bass").energy(0.47809014).variant("main"),
  sec("outro", 4, "kick hat ohat perc shaker").name("outro2").energy(0.41284788).variant("main"),
  sec("outro", 4, "kick hat perc").name("outro3").energy(0.37185276),
]);

mix("clap", #{ reverb: 0.18 });
mix("hat", #{ pan: 0.15 });
mix("ohat", #{ pan: -0.1 });
mix("shaker", #{ pan: -0.3 });
mix("perc", #{ pan: 0.35, delay: 0.15 });
mix("bass", #{ duck: 0.55 });
mix("chords", #{ reverb: 0.29302064, delay: 0.09769954, duck: 0.45 });
mix("pad", #{ reverb: 0.2860483, duck: 0.5 });
mix("lead", #{ reverb: 0.3043635, delay: 0.17807142, duck: 0.2 });
mix("arp", #{ pan: 0.2, reverb: 0.26157722, delay: 0.34599537, duck: 0.3 });
mix("fx", #{ reverb: 0.25 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [#{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 }]);
fx("chords", [#{ fx: "chorus", delay_ms: 11, depth_ms: 2.5, mix: 0.4, rate_hz: 0.45 }]);
fx("pad", [#{ fx: "phaser", depth: 0.69981074, feedback: 0.37219805, mix: sections(0.2858641, #{ drop: 0.42805505, hook: 0.42805505 }), rate_hz: 0.14278676, stages: 6 }]);
fx("lead", [#{ fx: "flanger", delay_ms: 3.3502333, depth_ms: 2.713053, feedback: 0.49436772, mix: 0.4030139, rate_hz: 0.25133887 }]);
bus("drums", [#{ fx: "compressor", attack_ms: 13.4044, knee_db: 6, makeup_db: 1, mix: 1, ratio: 2.4370754, release_ms: 132.5543, threshold_db: -11.541128 }]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 110, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 25, gain_db: 0, kind: "highpass", q: 0.7 }, #{ freq: 10000, gain_db: 1, kind: "highshelf", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 30, knee_db: 6, makeup_db: 0, mix: 1, ratio: 1.6, release_ms: 200, threshold_db: -10 },
  #{ fx: "saturate", drive_db: 2, kind: "tape", mix: 0.35 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.4176444, decay: 2.373905, lowcut: 281.2672, mix: 1, predelay_ms: 25.852709, width: 1.2839136 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.5, feedback: 0.29412478, mix: 1, pingpong: true } });

// racks: tracks played by several sounds at once, each its band, level, duck and effects
rack("bass", [
  #{ sound: "sub_bass", fold: [28, 40] },
]);

// Glass Garden — house (seed 7)
// song("house", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(122); key("A minor"); swing(0.56);
humanize(0.25);
prog("A", "i7 VImaj7 iv7 v7");
prog("B", "iv9 v7 i7 VImaj7");
bars_per_chord(1);
master(#{ lufs: -10, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X...X...X...X...");
drum("kick", "sparse", "X...X...X...X...");
drum("clap", "main", "....X.......X...");
drum("hat", "main", "..xxx..xx.xxx...");
drum("hat", "sparse", "..x...x...x...x.");
drum("ohat", "main", "..x...x...x...x.");
drum("shaker", "main", "x..o.oo...oox.o.");
drum("perc", "main", "x..x..x..x..x...");
drum("perc", "sparse", "x.......x.......");
drum_sound("perc", "rim");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "syncopated", preset: "pluck_bass", register: [33, 52], density: 0.42864978, seed: -37300132614189792 });
part("chords", #{ style: "syncopated", preset: "epiano", register: [55, 76], density: 0.47236422, seed: -771037852412648822 });
part("pad", #{ style: "sustain", preset: "pwm_pad", register: [50, 76], density: 0.5823303, seed: -7892568474930094457 });
part("lead", #{ style: "motif", preset: "tine_piano", register: [64, 86], density: 0.36092228, seed: 6104030045851553230 });

// pattern tracks (mini-notation, Strudel style)
track("rim", s("~ rim ~ rim").gain(0.6).pan(0.2));
track("bass2", note("<a1 f1 d1 e1>").struct("x ~ x ~ x ~ x ~").s("sub_bass").legato(0.8));
track("keys2", chord("prog").voicing().struct("x ~ ~ x ~ ~ x ~").s("epiano").room(0.25));

form([
  sec("intro", 4, "kick hat shaker perc top").energy(0.5403019),
  sec("groove", 8, "kick clap hat ohat shaker perc bass chords top rim bass2 keys2").energy(0.61373895).fills(false),
  sec("break", 8, "pad chords lead hat").energy(0.3407866),
  sec("build", 4, "clap hat chords lead pad").energy(0.57113045).prog("B").gap(),
  sec("drop", 16, "kick clap hat ohat shaker perc bass chords lead top rim bass2 keys2").energy(0.9266603).impact(),
  sec("break", 4, "pad chords arp").name("break2").energy(0.3887422).build(true),
  sec("drop", 12, "kick clap hat ohat shaker perc bass chords lead arp top rim bass2 keys2").name("drop2").energy(0.98664886).impact(),
  sec("drop", 12, "kick clap hat ohat shaker perc bass chords lead arp top rim bass2 keys2").name("drop3").energy(1),
  sec("outro", 4, "kick hat shaker perc bass").energy(0.3537415),
]);

mix("clap", #{ reverb: 0.18 });
mix("hat", #{ pan: 0.15 });
mix("ohat", #{ pan: -0.1 });
mix("shaker", #{ pan: -0.3 });
mix("perc", #{ pan: 0.35, delay: 0.15 });
mix("bass", #{ duck: 0.55 });
mix("chords", #{ reverb: 0.29302064, delay: 0.09769954, duck: 0.45 });
mix("pad", #{ reverb: 0.2860483, duck: 0.5 });
mix("lead", #{ level: -2, reverb: 0.35, delay: 0.17807142, duck: 0.2 });
mix("fx", #{ reverb: 0.25 });

// effects: each track's own chain, then the buses', the master's and the sends'; a setting that
// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })
fx("bass", [#{ fx: "tape", drive_db: 3.364756, flutter: 0.15757337, lowpass_hz: 9135.193, wow: 0.32499444 }]);
fx("chords", [#{ fx: "chorus", delay_ms: 10.380553, depth_ms: 1.6460402, mix: 0.26222828, rate_hz: 0.71941394 }]);
fx("pad", [#{ fx: "phaser", depth: 0.69981074, feedback: 0.37219805, mix: sections(0.2858641, #{ drop: 0.42805505, hook: 0.42805505 }), rate_hz: 0.14278676, stages: 6 }]);
fx("lead", [#{ fx: "flanger", delay_ms: 3.3502333, depth_ms: 2.713053, feedback: 0.49436772, mix: 0.4030139, rate_hz: 0.25133887 }]);
bus("drums", [#{ fx: "compressor", attack_ms: 12, knee_db: 6, makeup_db: 1, mix: 1, ratio: 2, release_ms: 120, threshold_db: -12 }]);
bus("music", [#{ fx: "eq", bands: [#{ freq: 110, gain_db: 0, kind: "highpass", q: 0.7 }] }]);
master(#{ fx: [
  #{ fx: "eq", bands: [#{ freq: 25, gain_db: 0, kind: "highpass", q: 0.7 }, #{ freq: 10000, gain_db: 1, kind: "highshelf", q: 0.7 }] },
  #{ fx: "compressor", attack_ms: 30, knee_db: 6, makeup_db: 0, mix: 1, ratio: 1.6, release_ms: 200, threshold_db: -10 },
  #{ fx: "saturate", drive_db: 2, kind: "tape", mix: 0.35 },
] });
send_fx(#{ reverb: #{ fx: "reverb", damping: 0.4176444, decay: 2.373905, lowcut: 281.2672, mix: 1, predelay_ms: 25.852709, width: 1.2839136 }, delay: #{ fx: "delay", beats: 0.75, damping: 0.5, feedback: 0.29412478, mix: 1, pingpong: true } });

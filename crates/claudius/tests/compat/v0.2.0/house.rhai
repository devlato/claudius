// Glass Garden — house (seed 7)
// song("house", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(120); key("D minor"); swing(0.5103269);
humanize(0.25);
prog("A", "i7 VImaj7 iv7 v7");
prog("B", "VImaj7 v7 iv7 i7");
bars_per_chord(1);
master(#{ lufs: -10, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X...X...X...X...");
drum("kick", "sparse", "X...X...X...X...");
drum("clap", "main", "....X.......X...");
drum("hat", "main", "..xxx..xx.xxx...");
drum("hat", "sparse", "..x...x...x...x.");
// hat: hat cut from 'Snappy Hi-Hat 10' (Apple Loops)
drum("ohat", "main", "..x...x...x...x.");
// ohat: ohat cut from 'Make Believe Hi-Hat 01' (Apple Loops)
drum("shaker", "main", "x..o.oo...oox.o.");
// shaker: shaker cut from 'Big Maracas 02' (Apple Loops)
drum("perc", "main", "x..x..x..x..x...");
drum("perc", "sparse", "x.......x.......");
drum_sound("perc", "rim");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "syncopated", preset: "pluck_bass", register: [33, 52], density: 0.42864978, seed: -37300132614189792 });
part("chords", #{ style: "syncopated", preset: "organ", register: [55, 76], density: 0.47236422, seed: -771037852412648822 });
part("pad", #{ style: "sustain", preset: "vsco2:strings", register: [50, 76], density: 0.5823303, seed: -7892568474930094457 });
part("lead", #{ style: "motif", preset: "supersaw", register: [64, 86], density: 0.36092228, seed: 6104030045851553230 });

form([
  sec("intro", 8, "kick hat shaker perc top").filter_in(),
  sec("groove", 8, "kick clap hat ohat shaker perc bass chords top"),
  sec("break", 8, "pad chords lead hat"),
  sec("build", 4, "clap hat chords lead pad").prog("B").gap(),
  sec("drop", 16, "kick clap hat ohat shaker perc bass chords lead top").impact(),
  sec("break", 4, "pad chords arp").name("break2").energy(0.4).build(true),
  sec("drop", 8, "kick clap hat ohat shaker perc bass chords lead arp top").name("drop2").energy(1).impact(),
  sec("outro", 8, "kick hat shaker perc bass"),
]);

mix("clap", #{ reverb: 0.18 });
mix("hat", #{ pan: 0.15 });
mix("ohat", #{ pan: -0.1 });
mix("shaker", #{ pan: -0.3 });
mix("perc", #{ pan: 0.35, delay: 0.15 });
mix("bass", #{ duck: 0.55 });
mix("chords", #{ reverb: 0.2, delay: 0.08, duck: 0.45 });
mix("pad", #{ reverb: 0.35, duck: 0.5 });
mix("lead", #{ reverb: 0.25, delay: 0.2, duck: 0.2 });
mix("fx", #{ reverb: 0.25 });

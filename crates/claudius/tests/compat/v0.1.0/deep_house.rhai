// Glass Garden — deep_house (seed 7)
// song("deep_house", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(118); key("D minor"); swing(0.53860575);
humanize(0.35);
prog("A", "i7 VImaj7 iv7 v7");
prog("B", "VImaj7 v7 iv7 i7");
bars_per_chord(1);
master(#{ lufs: -11, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X...X...X...X...");
drum("kick", "sparse", "X...X...X...X...");
drum("clap", "main", "....X.......X...");
drum("hat", "main", "..xx..x..xxxxxxx");
drum("hat", "sparse", "..x...x...x...x.");
// hat: hat cut from 'Silver Hi-Hat 04' (Apple Loops)
drum("shaker", "main", "o.o.ooo.ooo.o.oo");
// shaker: shaker cut from 'Ride With Me Shaker' (Apple Loops)
drum("perc", "main", ".x.x.x..x.x.x.x.");
drum("perc", "sparse", "...x....x.....x.");
// perc: perc cut from 'Latin Day Bongo 05' (Apple Loops)

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "syncopated", preset: "pluck_bass", register: [33, 50], density: 0.42864978, seed: -37300132614189792 });
part("chords", #{ style: "syncopated", preset: "keys", register: [53, 74], density: 0.5272447, seed: 5396775171407160694 });
part("pad", #{ style: "sustain", preset: "vsco2:strings", register: [50, 76], density: 0.54910505, seed: -3681901744659677821 });
part("lead", #{ style: "motif", preset: "pluck", register: [62, 84], density: 0.3242152, seed: 805921219908132358 });

form([
  sec("intro", 8, "kick hat shaker perc top").filter_in(),
  sec("groove", 8, "kick clap hat shaker perc bass chords top"),
  sec("break", 8, "pad chords lead shaker"),
  sec("build", 4, "clap hat shaker chords lead pad").energy(0.55).prog("B"),
  sec("drop", 16, "kick clap hat shaker perc bass chords lead pad top").energy(0.9),
  sec("outro", 8, "kick hat shaker perc bass"),
]);

mix("clap", #{ reverb: 0.3 });
mix("hat", #{ pan: 0.18 });
mix("shaker", #{ pan: -0.3 });
mix("perc", #{ pan: 0.4, reverb: 0.1, delay: 0.2 });
mix("bass", #{ duck: 0.45 });
mix("chords", #{ reverb: 0.3, delay: 0.1, duck: 0.35 });
mix("pad", #{ reverb: 0.4, duck: 0.4 });
mix("lead", #{ reverb: 0.35, delay: 0.25, duck: 0.15 });
mix("fx", #{ level: -3, reverb: 0.3 });

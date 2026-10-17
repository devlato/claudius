// Golden Pulse — deep_house (seed 8)
// song("deep_house", 8);   // uncomment to start over from the generator
title("Golden Pulse"); tempo(120); key("A dorian"); swing(0.53493524);
humanize(0.35);
prog("A", "i9 IV7");
prog("B", "i IV7");

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X...X...X...X...");
drum("kick", "sparse", "X...X...X...X...");
drum("clap", "main", "....X.......X...");
drum("hat", "main", "x.xxx.x..xxxx.x.|x.xxx.x..xxxx.x.|x.xxx.x..xxxx.x.|..xx.xx...xxxxxx");
drum("hat", "sparse", "..x...x...x...xx");
drum("shaker", "main", "o.o.o.ooo.ooo.o.");
drum("perc", "main", "x.x..x.x.x..x.x.");
drum("perc", "sparse", "..x.....x....x..");

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "syncopated", preset: "sub_bass", register: [33, 50], density: 0.4658955, seed: 7593466037959292827 });
part("chords", #{ style: "stabs", preset: "keys", register: [53, 74], density: 0.43242535, seed: -1845194844081732826 });
part("pad", #{ style: "sustain", preset: "vsco2:strings", register: [50, 76], density: 0.403872, seed: 5506831138054929778 });
part("lead", #{ style: "motif", preset: "jrhodes:rhodes", register: [62, 84], density: 0.34817874, seed: -2367553514172308588 });

form([
  sec("intro", 16, "kick hat shaker perc top").filter_in(),
  sec("groove", 16, "kick clap hat shaker perc bass chords top"),
  sec("break", 24, "pad chords lead shaker"),
  sec("build", 8, "clap hat shaker chords lead pad").energy(0.55).prog("B"),
  sec("drop", 32, "kick clap hat shaker perc bass chords lead pad top").energy(0.9),
  sec("outro", 16, "kick hat shaker perc bass"),
]);

mix("clap", #{ reverb: 0.3 });
mix("hat", #{ level: -2.9886246, pan: 0.18 });
mix("shaker", #{ pan: -0.3 });
mix("perc", #{ level: 2.3582892, pan: 0.4, reverb: 0.1, delay: 0.2 });
mix("bass", #{ duck: 0.45 });
mix("chords", #{ reverb: 0.3, delay: 0.1, duck: 0.35 });
mix("pad", #{ reverb: 0.4, duck: 0.4 });
mix("lead", #{ reverb: 0.35, delay: 0.25, duck: 0.15 });
mix("fx", #{ level: -3, reverb: 0.3 });

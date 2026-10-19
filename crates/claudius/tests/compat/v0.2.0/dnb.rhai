// Glass Garden — dnb (seed 7)
// song("dnb", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(170); key("D minor"); swing(0.50172114);
humanize(0.1);
prog("A", "i III VII VI");
prog("B", "VI III VII i");
bars_per_chord(2);
master(#{ lufs: -8.5, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "alt", "X......X..X.....");
drum("kick", "main", "X.........X.....");
drum("snare", "main", "o...X.o.....X...");
// snare: snare cut from 'Lazy Haze Clap' (Apple Loops)
drum("hat", "main", "xxxxx.x.x.x.x.x.");
// hat: hat cut from 'Little Synth Snare Topper' (Apple Loops)
drum("ride", "main", "o...o.o.o.o.o.o.");
drum("shaker", "main", "ooo.ooo.ooo.oooo");
// shaker: shaker cut from 'Latin Pop Percussion 02' (Apple Loops)

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "sustain", preset: "wobble", register: [26, 41], density: 0.42864978, seed: -37300132614189792 });
part("pad", #{ style: "sustain", preset: "vsco2:strings", register: [48, 74], density: 0.4710492, seed: -6710496186308540014 });
part("lead", #{ style: "motif", preset: "pluck", register: [64, 86], density: 0.4863813, seed: -3681901744659677821 });
part("arp", #{ style: "up", preset: "pluck", register: [60, 84], density: 0.47421524, rate: 0.25, seed: 6104030045851553230 });

form([
  sec("intro", 8, "pad hat ride").energy(0.35).variant("main").filter_in(),
  sec("build", 8, "pad hat ride snare lead").energy(0.55).gap(),
  sec("drop", 16, "kick snare hat ride shaker bass pad lead").impact(),
  sec("break", 8, "pad lead arp hat").energy(0.4).build(true).gap(),
  sec("drop", 16, "*").name("drop2").energy(1).impact(),
  sec("outro", 8, "pad hat ride").energy(0.35).variant("main"),
]);

mix("snare", #{ reverb: 0.15 });
mix("hat", #{ pan: 0.15 });
mix("ride", #{ pan: -0.25 });
mix("shaker", #{ pan: 0.35 });
mix("bass", #{ duck: 0.5 });
mix("pad", #{ reverb: 0.4, duck: 0.4 });
mix("lead", #{ reverb: 0.3, delay: 0.25, duck: 0.25 });
mix("arp", #{ reverb: 0.25, delay: 0.3, duck: 0.3 });
mix("fx", #{ reverb: 0.3 });

// Glass Garden — future_bass (seed 7)
// song("future_bass", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(141); key("D major"); swing(0.5051635);
humanize(0.1);
prog("A", "IV I V vi");
prog("B", "I IV vi V");
bars_per_chord(1);
master(#{ lufs: -9, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X.........X.....|X.........X.....|X.........X.....|X...............");
drum("snare", "main", "........X.......");
// snare: snare cut from 'Dusty Springs Rim Shot' (Apple Loops)
drum("hat", "main", "x.....x..x..x.....x.....x..x..x.....x.....x..x..|x.....x..x..x.....x.....x..x..x.....X.Xx.xx.xx.x|x.....x..x..X.X.x.x.x.x.x..x..x.....x.....x..x..|x.....x..x..x.....x.....x..x..x.....x.....x..x..", 48);
// hat: hat cut from 'Sky High Drift Hi-Hat' (Apple Loops)
drum("ohat", "main", "..........x.....");
// ohat: ohat cut from 'Loose Hi-Hat 01' (Apple Loops)
drum("perc", "main", "..x.....x.....x.");
drum("perc", "sparse", "..x.............");
drum_sound("perc", "snap");

// parts written by the theory engine: change the style, preset, register, density or seed
part("chords", #{ style: "pump", preset: "supersaw", register: [52, 79], density: 0.42864978, seed: -37300132614189792 });
part("bass", #{ style: "sustain", preset: "sub_bass", register: [33, 48], density: 0.55524325, seed: 6553130387488681005 });
part("lead", #{ style: "motif", preset: "pluck", register: [67, 91], density: 0.5065276, seed: -4694233468511451402 });
part("pad", #{ style: "sustain", preset: "pad", register: [50, 76], density: 0.5752324, seed: 8524255199291047346 });

form([
  sec("intro", 4, "pad lead").energy(0.35).variant("main").filter_in(),
  sec("verse", 8, "kick snare hat bass pad lead").energy(0.55),
  sec("build", 4, "snare hat chords lead").energy(0.65).gap(),
  sec("drop", 8, "kick snare hat ohat perc bass chords lead").energy(1).impact(),
  sec("break", 4, "pad lead arp").energy(0.4),
  sec("build", 4, "snare chords arp").name("build2").energy(0.65).prog("B").gap(),
  sec("drop", 8, "kick snare hat ohat perc bass chords lead arp").name("drop2").energy(1).impact(),
  sec("outro", 4, "pad chords").energy(0.35).variant("main"),
]);

mix("snare", #{ reverb: 0.25 });
mix("hat", #{ pan: 0.15 });
mix("ohat", #{ pan: -0.15 });
mix("perc", #{ pan: 0.4, reverb: 0.3 });
mix("chords", #{ reverb: 0.25, delay: 0.05, duck: 0.85 });
mix("bass", #{ duck: 0.6 });
mix("lead", #{ reverb: 0.3, delay: 0.2, duck: 0.3 });
mix("pad", #{ reverb: 0.4, duck: 0.5 });
mix("fx", #{ reverb: 0.3 });

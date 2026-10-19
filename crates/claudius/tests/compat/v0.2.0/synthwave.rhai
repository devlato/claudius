// Glass Garden — synthwave (seed 7)
// song("synthwave", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(87); key("D minor"); swing(0.5);
humanize(0.1);
prog("A", "i VI III VII");
prog("B", "i VI VII i");
bars_per_chord(1);
master(#{ lufs: -10.5, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "alt", "X...X...X...X...");
drum("kick", "main", "X.......X.......");
drum("snare", "main", "....X.......X...");
// snare: snare cut from 'Cosmic Cruise Snare' (Apple Loops)
drum("hat", "alt", "x.x.xxx.x.xxx.x.");
drum("hat", "main", "x.x.x.x.x.x.x.x.");
// hat: hat cut from 'Level Up Drums 02' (Apple Loops)

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "root8", preset: "pluck_bass", register: [33, 48], density: 0.42864978, seed: -37300132614189792 });
part("arp", #{ style: "up", preset: "chip", register: [60, 84], density: 0.55524325, rate: 0.25, seed: -6710496186308540014 });
part("pad", #{ style: "sustain", preset: "strings", register: [50, 76], density: 0.44522208, seed: -4694233468511451402 });
part("lead", #{ style: "motif", preset: "supersaw", register: [64, 86], density: 0.5252324, seed: 8524255199291047346 });
part("chords", #{ style: "half", preset: "supersaw", register: [55, 76], density: 0.49820793, seed: 4272912310042672631 });

form([
  sec("intro", 4, "pad arp").energy(0.35).variant("main").filter_in(),
  sec("verse", 8, "kick snare hat bass arp pad").energy(0.6).fills(true),
  sec("hook", 8, "*").name("chorus").energy(0.85).impact(),
  sec("verse", 8, "kick snare hat bass arp pad").name("verse2").energy(0.6).fills(true),
  sec("hook", 8, "*").name("chorus2").energy(0.9).impact(),
  sec("break", 4, "pad lead").name("bridge").energy(0.45).build(true),
  sec("hook", 8, "*").name("chorus3").energy(1).impact(),
  sec("outro", 4, "pad arp").energy(0.35).variant("main"),
]);

mix("snare", #{ reverb: 0.45 });
mix("hat", #{ pan: 0.2 });
mix("bass", #{ duck: 0.3 });
mix("arp", #{ pan: 0.15, reverb: 0.25, delay: 0.3, duck: 0.2 });
mix("pad", #{ reverb: 0.4, duck: 0.25 });
mix("lead", #{ reverb: 0.3, delay: 0.25 });
mix("chords", #{ reverb: 0.3, duck: 0.3 });
mix("fx", #{ level: -3, reverb: 0.35 });

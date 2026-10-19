// Glass Garden — techno (seed 7)
// song("techno", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(126); key("D minor"); swing(0.5034423);
humanize(0.1);
prog("A", "i9 i9 i9 VII");
prog("B", "i");
bars_per_chord(4);
master(#{ lufs: -9, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X...X...X...X...");
drum("hat", "main", "..x...x...x...x.");
drum("hat", "sparse", "..x...x...x...x.");
drum("ohat", "main", "..............x.");
drum("clap", "main", "....X.......X...");
// clap: clap cut from 'Flow Solo Clap 01' (Apple Loops)
drum("ride", "main", "ooo.ooo.o.o.ooo.");
drum("perc", "main", "..x.....x...x...");
drum("perc", "sparse", "............x...");
// perc: perc cut from 'Remix Conga Fever 02' (Apple Loops)

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "rolling", preset: "sub_bass", register: [30, 45], density: 0.53341293, seed: -2526400537787473278 });
part("chords", #{ style: "offbeat", preset: "stab", register: [52, 70], density: 0.4710492, seed: -6710496186308540014 });
part("pad", #{ style: "sustain", preset: "warm_pad", register: [45, 70], density: 0.54910505, seed: -3681901744659677821 });
part("lead", #{ style: "motif", preset: "pluck", register: [62, 82], density: 0.20566142, seed: 805921219908132358 });

form([
  sec("intro", 16, "kick hat perc top").energy(0.45).filter_in(),
  sec("groove", 16, "kick hat ohat clap perc ride bass top").energy(0.7),
  sec("break", 8, "pad chords lead ride").prog("A"),
  sec("build", 4, "clap hat ride chords pad").gap(),
  sec("drop", 16, "kick hat ohat clap perc ride bass acid chords top").name("peak").energy(1).impact(),
  sec("outro", 8, "kick hat perc ride").energy(0.45),
]);

mix("hat", #{ pan: 0.1 });
mix("ohat", #{ pan: -0.15 });
mix("clap", #{ reverb: 0.25 });
mix("ride", #{ pan: 0.3 });
mix("perc", #{ pan: -0.35, reverb: 0.15, delay: 0.2 });
mix("bass", #{ duck: 0.65 });
mix("chords", #{ reverb: 0.35, delay: 0.3, duck: 0.45 });
mix("pad", #{ reverb: 0.45, duck: 0.5 });
mix("lead", #{ reverb: 0.4, delay: 0.35 });
mix("fx", #{ reverb: 0.3 });

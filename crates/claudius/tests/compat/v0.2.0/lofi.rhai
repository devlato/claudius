// Glass Garden — lofi (seed 7)
// song("lofi", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(71); key("D major"); swing(0.5720481);
humanize(0.6);
prog("A", "Imaj7 IVmaj7");
prog("B", "ii9 V9 Imaj9 vi9");
bars_per_chord(1);
master(#{ lufs: -12.5, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "main", "X......X..X.....|X......X..X.....|X......X..X.....|X.........X.....");
drum("snare", "main", "....X.......X...");
drum("hat", "main", "x.x.x...x.x.x.xx");
// hat: hat cut from 'Deep Hi-Hat 02' (Apple Loops)
drum("shaker", "main", "..oooooo.oo...oo");
// shaker: shaker cut from 'Bright Tambourine Topper' (Apple Loops)

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "walking", preset: "organ_bass", register: [33, 50], density: 0.5995956, seed: 3796763344308755361 });
part("keys", #{ style: "sustain", preset: "freepats:upright_piano", register: [52, 74], density: 0.45851198, seed: 6674420348178494801 });
part("lead", #{ style: "motif", preset: "vcsl:kalimba", register: [64, 84], density: 0.3823303, seed: -7892568474930094457 });

form([
  sec("intro", 4, "keys").energy(0.3).variant("main").filter_in(),
  sec("verse", 8, "kick snare hat bass keys").name("a").energy(0.6).fills(true),
  sec("hook", 8, "*").name("b").energy(0.75),
  sec("verse", 8, "kick snare hat bass keys pad").name("a2").fills(true),
  sec("outro", 4, "keys pad lead").energy(0.3).variant("main"),
]);

mix("snare", #{ reverb: 0.1 });
mix("hat", #{ pan: 0.2 });
mix("shaker", #{ pan: -0.3 });
mix("keys", #{ reverb: 0.25 });
mix("lead", #{ reverb: 0.35, delay: 0.2 });

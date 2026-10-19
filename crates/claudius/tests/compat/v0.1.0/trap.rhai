// Glass Garden — trap (seed 7)
// song("trap", 7);   // uncomment to start over from the generator
title("Glass Garden"); tempo(131); key("D minor"); swing(0.50688463);
humanize(0.15);
prog("A", "i VI iv V");
prog("B", "i VII VI V");
bars_per_chord(2);
master(#{ lufs: -9, ceiling: -1 });

// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows
drum("kick", "alt", "X.........X.....|X......X........|X.........X.....|X......X........");
drum("kick", "main", "X......X..X.....|X..X............|X......X........|X.....X.........");
drum("snare", "main", "........X...o...");
// snare: snare cut from 'Keep Shining Snare 02' (Apple Loops)
drum("clap", "main", "........X.......");
// clap: clap cut from 'Crowd Clapping Beat' (Apple Loops)
drum("hat", "alt", "x.....x.....X.Xx.xx.xx.xx.....x.....x.....x.....|x.....x.....x.....x.....x.....x.....x.....x.....|x.....x.....x.....x.....x.....x.....x.....x.....|x.....x.....ooxxxxxxxxXXx.....x.....x.....x.....", 48);
drum("hat", "main", "x.....x..x..x..x..x.....x..x..x..x..X.Xx.xx.xx.x|x.....x..x..x..x..x.....x..x..x..x..x.....x..x..|x.....x..x..x..x..x.....x..x..x..x..ooxxxxxxxxXX|x.....x..x..x..x..x.....x..x..x..x..x.....x..x..", 48);
drum("ohat", "main", "................");
drum("perc", "main", "x.....x...x.....");
drum("perc", "sparse", "..........x.....");
// perc: perc cut from 'Latin Day Conga 01' (Apple Loops)

// parts written by the theory engine: change the style, preset, register, density or seed
part("bass", #{ style: "kick_follow", preset: "808", register: [28, 43], density: 0.5726087, seed: 2642474102756894640 });
part("keys", #{ style: "half", preset: "salamander:piano", register: [55, 76], density: 0.4710492, seed: -6710496186308540014 });
part("lead", #{ style: "motif", preset: "pluck", register: [67, 88], density: 0.46008074, seed: -1629742732023437997 });

form([
  sec("intro", 4, "keys pad hat").energy(0.35).variant("main").filter_in(),
  sec("hook", 8, "kick snare clap hat ohat perc bass keys lead pad").energy(0.9).fills(false).impact(),
  sec("verse", 8, "kick snare hat perc bass keys").variant("alt"),
  sec("hook", 8, "kick snare clap hat ohat perc bass keys lead pad").name("hook2").fills(false).impact(),
  sec("verse", 4, "kick snare hat bass keys pad").name("verse2").energy(0.6).variant("alt").gap(),
  sec("hook", 8, "kick snare clap hat ohat perc bass keys lead pad").name("hook3").energy(1).fills(false).impact(),
  sec("outro", 4, "keys pad lead").energy(0.35).variant("main"),
]);

mix("snare", #{ reverb: 0.12 });
mix("clap", #{ level: -4, reverb: 0.15 });
mix("hat", #{ pan: 0.12 });
mix("ohat", #{ pan: -0.15 });
mix("perc", #{ pan: -0.4, reverb: 0.25 });
mix("keys", #{ reverb: 0.3, delay: 0.1 });
mix("lead", #{ reverb: 0.3, delay: 0.25 });
mix("fx", #{ level: -2, reverb: 0.2 });

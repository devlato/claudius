// liquid drum and bass in C minor at 174 BPM: a grand piano, strings and a flute over a recorded kit
blank("dnb"); title("Low Light"); tempo(174); key("C minor");
prog("A", "i9 VImaj7 IIImaj7 VII"); bars_per_chord(2);

track("kick",   s("bd ~ ~ ~ ~ ~ ~ ~ ~ ~ bd ~ ~ ~ ~ ~").bank("uzu"));
track("snare",  s("~ ~ ~ ~ sd ~ ~ ~ ~ ~ ~ ~ sd ~ ~ ~").bank("acoustic"));
track("ghosts", s("~ ~ ~ ~ ~ ~ ~ sd ~ sd ~ ~ ~ ~ ~ sd").bank("acoustic").gain(0.35));
track("hats",   s("hh*8").bank("acoustic").gain("0.8 0.5"));
track("ride",   s("~ rd ~ rd").bank("acoustic").gain(0.4).pan(0.25));
track("bass",   note("<c2 c2 ab1 ab1 eb2 eb2 bb1 bb1>").struct("x ~ ~ ~ ~ ~ x ~ ~ ~ x ~ ~ ~ ~ ~").s("sub_bass").legato(0.9));
track("piano",  chord("prog").struct("x ~ ~ ~ ~ ~ ~ ~ ~ ~ x ~ ~ ~ ~ ~").voicing(55, 76).s("piano").room(0.35));
track("strings", chord("prog").voicing(53, 72).s("string_ensemble").level(-7).room(0.5));

let tune = "<[4 ~ ~ 2 4 ~ 6 ~] [7 ~ ~ ~ ~ ~ ~ ~] [2 ~ ~ 0 2 ~ 4 ~] [4 ~ ~ ~ ~ ~ 3 2] "
         + "[4 ~ ~ 2 4 ~ 8 ~] [6 ~ ~ ~ ~ ~ ~ ~] [3 ~ ~ 1 3 ~ 5 ~] [6 ~ ~ ~ ~ ~ ~ ~]>";
track("flute",  n(tune).scale("C:minor").octave(5).s("flute").level(-4).room(0.4).delay(0.2));

form([sec("intro", 16, "piano strings").filter_in(), sec("build", 8, "piano strings hats ride kick").build(),
      sec("drop", 32, "*").impact(), sec("break", 16, "piano strings flute").prog("A").gap(),
      sec("drop", 32, "*").impact(), sec("outro", 8, "piano strings")]);

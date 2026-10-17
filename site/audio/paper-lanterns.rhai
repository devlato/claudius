// lofi in D minor at 82 BPM: a Rhodes, an upright bass, a vibraphone and strings over a recorded kit
blank("lofi"); title("Paper Lanterns"); tempo(82); key("D minor"); swing(0.6); humanize(0.35);
prog("A", "i9 iv9 VImaj7 V7"); bars_per_chord(1);

track("kick",  s("bd ~ ~ ~ ~ ~ bd ~ ~ ~ bd ~ ~ ~ ~ ~").bank("uzu"));
track("snare", s("~ sd ~ sd").bank("acoustic").gain(0.8));
track("hats",  s("hh*8").bank("acoustic").gain("0.9 0.55").degradeBy(0.1));
track("tamb",  s("~ tambourine ~ tambourine").gain(0.5).pan(0.3));
track("keys",  chord("prog").struct("x ~ ~ ~ ~ ~ x ~").voicing(53, 74).s("rhodes").room(0.3));
track("pad",   chord("prog").voicing(57, 72).s("string_ensemble").level(-8));
track("bass",  note("<[d2 ~ ~ a1 ~ ~ d2 ~] [g1 ~ ~ d2 ~ ~ g1 ~] [bb1 ~ ~ f2 ~ ~ bb1 ~] [a1 ~ ~ e2 ~ ~ a1 ~]>")
                 .s("upright_bass"));

let tune = "<[3 ~ 4 3 1 ~ 0 ~] [~ ~ 4 5 2 ~ 1 ~] [5 ~ 6 5 3 ~ 2 ~] [3 ~ ~ ~ 2 ~ ~ ~] "
         + "[3 ~ 4 3 1 ~ 0 ~] [~ ~ 4 5 2 ~ 1 ~] [6 ~ 7 6 5 ~ 3 ~] [2 ~ 3 ~ ~ ~ ~ ~]>";
track("vibes", n(tune).scale("D:minor:pentatonic").octave(4).s("vibes").delay(0.25));

form([sec("intro", 4, "keys pad"), sec("groove", 8, "kick snare hats keys bass"), sec("groove", 16, "*"),
      sec("break", 8, "keys vibes pad").prog("A"), sec("groove", 16, "*"), sec("outro", 4, "keys pad")]);

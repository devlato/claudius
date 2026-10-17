// techno in F minor at 130 BPM: a TR-909, a rolling bass between the kicks, an acid line and a chord stab
blank("techno"); title("Night Shift"); tempo(130); key("F minor");
prog("A", "i7"); bars_per_chord(1);

track("kick",  s("bd*4").bank("tr909"));
track("clap",  s("~ cp ~ cp").bank("tr909").room(0.25));
track("ohat",  s("[~ oh]*4").bank("tr909").gain(0.7).pan(0.3));
track("hats",  s("hh*16").bank("tr909").gain("0.6 0.3 0.45 0.3").pan(-0.3));
track("rim",   s("~ ~ rim ~ ~ ~ rim ~ ~ rim ~ ~ ~ ~ rim ~").bank("tr909").gain(0.5).pan(0.5).delay(0.3));
track("congas", s("~ ~ conga ~ ~ conga ~ ~ ~ ~ conga ~ conga ~ ~ ~").gain(0.6).pan(-0.5).room(0.3));
track("bass",  note("f1").struct("~ x x x ~ x x x ~ x x x ~ x x x").s("saw_bass").lpf(500).legato(0.6));
track("acid",  note("<[f2 f2 f3 f2 ab2 f2 c3 f2 f2 eb3 f2 f2 c3 f2 bb2 eb3] [f2 f2 f3 f2 ab2 f2 c3 f2 f2 eb3 f2 ab2 c3 eb3 f3 c3]>")
                 .s("acid").role("lead").delay(0.15));
track("stab",  chord("prog").struct("~ ~ ~ x ~ ~ x ~").voicing(55, 70).s("stab").room(0.4).delay(0.3));
track("strings", chord("prog").voicing(53, 72).s("string_ensemble").level(-9).room(0.5));

form([sec("intro", 8, "kick hats"), sec("groove", 16, "kick hats ohat congas bass clap"),
      sec("build", 8, "kick hats ohat bass stab").build(), sec("drop", 16, "kick clap hats ohat rim congas bass stab"),
      sec("break", 8, "stab acid strings").prog("A").gap(), sec("drop", 16, "*").impact(), sec("outro", 8, "kick hats congas")]);

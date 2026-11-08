// "Midnight in Jozi": 2020s amapiano, the South African house that spread worldwide in the first half of the
// decade (Kabza De Small, Tyla, DJ Maphorisa). An original song in that style.
//
// How it is made:
// - 113 BPM and swung, a soft, steady kick on every beat under shakers in sixteenths, open hats on the offbeats, a
//   late clap and scattered rimshots: house's frame, played gently.
// - The log drum is the bass and the hook: a pitched, woody percussive bass playing syncopated phrases on the roots
//   and fifths. It is what amapiano is known for.
// - Jazzy ninths and major sevenths (Fm9 Bbm9 Ebmaj7 Abmaj7) on a percussive piano, from the township jazz and deep
//   house the genre grew out of.
// - A marimba-like melody and soft pads float over the drums, unhurried, and the grooves hold back for a long time
//   before the log drum comes in.
blank("amapiano");
title("Midnight in Jozi");
tempo(113); key("F minor"); swing(0.57);
prog("A", "i9 iv9 VIImaj7 IIImaj7");
bars_per_chord(1);

// drums in steps, as the Drums tab shows them: X accent, x hit, o ghost, . rest, sixteen a bar
drum("kick", "x...x...x...x...");
drum("clap", "....x.......x...");
drum("shaker", "xoxoxoxoxoxoxoxx");
drum("ohat", "..x...x...x...x.");
drum("rim", "...x.....o....x.");
lane("clap", #{ late: 5 });
lane("rim", #{ late: 6 });
mix("clap", #{ reverb: 0.2 });
mix("shaker", #{ pan: -0.3 });
mix("ohat", #{ pan: 0.2, level: -2 });
mix("rim", #{ pan: 0.35, reverb: 0.15 });

track("log", note("<[f1 ~ ~ f1 ~ ~ c2 ~ ~ ~ f1 ~ ab1 ~ ~ ~] [bb1 ~ ~ bb1 ~ ~ f2 ~ ~ ~ bb1 ~ db2 ~ ~ ~] [eb1 ~ ~ eb1 ~ ~ bb1 ~ ~ ~ eb1 ~ g1 ~ ~ ~] [ab1 ~ ~ ab1 ~ ~ eb2 ~ ~ ~ ab1 ~ c2 ~ eb2 ~]>")
               .s("log_drum").legato(0.6).role("bass").level(2));
track("piano", chord("prog").voicing(53, 74).struct("~ x ~ ~ x ~ ~ x").s("house_piano").legato(0.4).room(0.3).delay(0.1).role("keys"));
fx("piano", [#{ fx: "width", width: 1.3, mono_below_hz: 200 }]);
track("pad", chord("prog").voicing(52, 70).s("warm_pad").role("pad").level(-2).room(0.45));
fx("pad", [#{ fx: "width", width: 1.5, mono_below_hz: 200 }]);
track("marimba", n("<[4 ~ 6 ~ 7 ~ 6 4] [3 ~ ~ 4 ~ ~ 2 ~] [6 ~ 4 ~ 2 ~ 4 6] [7@3 ~]>")
                   .octave(4).s("fm_marimba").legato(0.5).delay(0.3).room(0.35).pan(0.2).role("lead"));

form([
  sec("intro", 8, "shaker ohat piano pad").filter_in(),
  sec("verse", 16, "kick clap shaker ohat rim piano pad"),
  sec("hook", 16, "kick clap shaker ohat rim log piano pad marimba").impact(),
  sec("verse", 16, "kick clap shaker ohat rim log piano"),
  sec("hook", 16, "kick clap shaker ohat rim log piano pad marimba").impact(),
  sec("break", 8, "shaker pad marimba"),
  sec("hook", 16, "kick clap shaker ohat rim log piano pad marimba").impact(),
  sec("outro", 8, "shaker ohat log"),
]);

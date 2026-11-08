// "Calm Waters": 2020s afrobeats, the West African pop that topped charts around the world in the first half of the
// decade (Burna Boy, Wizkid, Rema). An original song in that style.
//
// How it is made:
// - 104 BPM and lilting: a kick on the tresillo (1, the "and" of 2, 4), a rimshot answering it, shakers and congas
//   carrying the sixteenths. The groove does the work a four-on-the-floor would.
// - Major and warm, I iii vi IV (Bb Dm Gm Eb), on a soft electric piano.
// - Plucked, highlife-style guitar chords on the offbeats, and a round bass that sings its own line.
// - A vocal-like lead with short, repeated phrases, the call-and-response of the genre's toplines.
blank("afrobeats");
title("Calm Waters");
tempo(104); key("Bb major"); swing(0.55);
prog("A", "I iii vi IV");
bars_per_chord(1);

// drums in steps, as the Drums tab shows them: X accent, x hit, o ghost, . rest, sixteen a bar
drum("kick", "X..x..x...x..x..");
drum("rim", "....x..x....x...");
drum("shaker", "oxoxoxoxoxoxoxox");
drum("perc", "..x...x.x...x.x.");
mix("rim", #{ pan: 0.2, reverb: 0.15 });
mix("shaker", #{ pan: -0.3 });
mix("perc", #{ pan: 0.35 });

track("bass", note("<[bb1 ~ ~ bb1 ~ ~ f2 ~] [d2 ~ ~ d2 ~ ~ a1 ~] [g1 ~ ~ g1 ~ ~ d2 bb1] [eb2 ~ ~ eb2 ~ ~ bb1 f2]>")
                .s("rubber_bass").legato(0.7));
track("guitar", chord("prog").voicing(60, 76).struct("~ x ~ x ~ x ~ x").s("harp_pluck").legato(0.3).pan(-0.35).room(0.2).role("chords"));
fx("guitar", [#{ fx: "width", width: 1.6, mono_below_hz: 200 }]);
track("keys", chord("prog").voicing(52, 71).struct("x ~ ~ ~ x ~ ~ ~").s("epiano").legato(0.9).room(0.25).pan(0.3).role("keys"));
fx("keys", [#{ fx: "chorus", rate_hz: 0.5, depth_ms: 3, delay_ms: 12, mix: 0.4 }, #{ fx: "width", width: 1.5, mono_below_hz: 200 }]);
track("pad", chord("prog").voicing(52, 70).s("warm_pad").role("pad").level(-3));
fx("pad", [#{ fx: "width", width: 1.6, mono_below_hz: 200 }]);
track("vox", n("<[4 ~ 4 2 ~ 4 ~ ~] [6 ~ 6 4 ~ 2 ~ ~] [5 ~ 7 5 ~ 2 ~ ~] [7 ~ 5 ~ 3 ~ ~ ~]>")
               .octave(4).s("vox_lead").legato(0.7).delay(0.2).room(0.25).role("lead"));

form([
  sec("intro", 8, "shaker perc guitar keys").filter_in(),
  sec("verse", 16, "kick rim shaker perc bass guitar keys"),
  sec("hook", 16, "kick rim shaker perc bass guitar keys pad vox").impact(),
  sec("verse", 16, "kick rim shaker perc bass guitar keys"),
  sec("hook", 16, "kick rim shaker perc bass guitar keys pad vox").impact(),
  sec("break", 8, "shaker perc guitar pad"),
  sec("hook", 16, "kick rim shaker perc bass guitar keys pad vox").impact(),
  sec("outro", 8, "shaker perc guitar"),
]);

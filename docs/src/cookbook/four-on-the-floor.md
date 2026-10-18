# House, deep house and techno

Three styles built on the same foundation, a kick on every beat, and different in nearly everything else: house is
warm and swung with piano and vocals, deep house is slower and jazzier, techno is hypnotic, dark and dry. Each recipe
below is a complete song from a blank start, with the reasons for its choices and the critic's verdict when we
rendered it. They use sampled instruments and drum machines, so they need the default sample packs
(`claudius setup`).

Save a recipe as a `.rhai` file and render it with `claudius script house.rhai`.

## House: "Glass Avenue"

```js packs
blank("house");
title("Glass Avenue");
tempo(122); key("A minor"); swing(0.58);
prog("A", "Am7 Fmaj7 Dm7 Em7");
prog("B", "Fmaj7 G Em7 Am7");
bars_per_chord(1);

track("kick",  s("bd*4").bank("tr909"));
track("clap",  s("~ cp ~ cp").bank("tr909").room(0.2));
track("hat",   s("[~ hh]*4").bank("tr808").gain(0.7).pan(0.15));
track("ohat",  s("[~ oh]*4").bank("tr909").gain(0.5).pan(-0.15));
track("shaker", s("shaker*8").gain("[0.3 0.6]*4").pan(-0.3).level(-3));
track("roll",  s("<sd*2 sd*2 sd*4 sd*4 sd*8 sd*8 sd*8 sd*8>").bank("tr909")
                 .gain("<0.4 0.45 0.5 0.55 0.65 0.75 0.85 1>"));

track("bass",   note("<a1 f1 d2 e2>").struct("~ x ~ x ~ x ~ [x x]").s("organ_bass").legato(0.7));
track("chords", chord("prog").struct("x ~ ~ x ~ ~ x ~").voicing(55, 74).s("piano").legato(0.5).room(0.2).level(2));
track("pad",    chord("prog").voicing(52, 72).s("string_ensemble").role("pad").level(-2).room(0.35));
track("vox",    note("<[~ ~ e3 ~ ~ ~ c3 ~] [~ ~ a2 ~ ~ ~ c3 ~] [~ ~ a2 ~ ~ ~ f2 ~] [~ ~ b2 ~ ~ ~ ~ ~]>").s("voice").role("vocal").legato(0.9).room(0.3).delay(0.15).level(-4));
track("lead",   n("<[4 ~ 4 2 4 ~ 6 4] [5 ~ 5 4 2 ~ 0 ~] [3 ~ 3 2 0 ~ 2 3] [4 ~ ~ 6 4 ~ ~ ~]>")
                  .octave(4).s("rhodes").room(0.25).delay(0.2));

form([
  sec("intro", 8, "kick hat shaker chords").filter_in(),
  sec("groove", 16, "kick clap hat shaker bass chords"),
  sec("break", 8, "pad chords"),
  sec("build", 8, "kick clap hat pad lead roll").gap(),
  sec("drop", 16, "kick clap hat ohat shaker bass chords pad lead vox").impact(),
  sec("outro", 8, "kick hat shaker bass"),
]);
```

Why it is built this way:

- **Tempo and swing.** 122 BPM sits in the middle of house's range. Swing 0.58 is on the heavy side for house; it
  makes the shaker and the bass pickups lean, which reads as warmth rather than as techno's grid.
- **Harmony.** `Am7 Fmaj7 Dm7 Em7` is the minor-soul progression (`i7 VImaj7 iv7 v7`): sevenths everywhere, no
  dominant pull, so it loops forever. The break moves to `B`, which lifts to G before falling back.
- **Drums.** A 909 kick and clap, the instruments of house. The closed hat comes from the 808 because it is softer
  than the 909's, and the open hat on the same offbeats gives the classic "tss" lift in the drop only. The roll
  doubles every two bars across the 8-bar build, and stops at eighths: house builds less violently than trance.
- **Bass.** An organ bass on the offbeats, with a sixteenth pickup at the end of each bar (`[x x]`), plays between
  the kicks instead of on them, which is the house bounce. It sits on the roots of the progression.
- **Chords.** A sampled grand piano on the tresillo rhythm (`x ~ ~ x ~ ~ x ~`), the sound of 90s house.
- **Vocal.** The `voice` pack's sung vowel plays a two-note answer in each bar of the drop. Pitched low, inside the
  recording's range, it sounds like a vocal chop rather than a synth.
- **Lead.** A Rhodes motif with chord tones on beats 1 and 3, enters in the build so the drop is a return, not a
  surprise.
- **Form.** The intro's `filter_in()` opens a lowpass on the piano over eight bars; the drums are never filtered, so
  the intro needs a music part for the sweep to be heard. The build keeps the kick (pattern tracks get no automatic
  kick drop-out), so its energy rises into the drop rather than dipping; the `gap()` gives the drop its breath.

The critic's verdict:

```text
  ears: 'house music' z +3.6 (closest confusion 'techno music' +3.4); production z +1.6 vs 'a low quality recording with hiss' +1.0; p_real 0.65
        sounds like: house music, techno music, tech house music, drum and bass music
  verdict: 87/100 (arc 0.85, genre 0.79, mix 0.93, production 0.66, theory 1.00). Fix first: dull 4-8 kHz: hats, claps and snare lack bite (presence -28.7 dB, healthy from -26); fizzy top end (air -16.1 dB, healthy up to -17)
```

House and techno sit close together for the ears, and a 909 groove pulls towards techno. An earlier version of this
recipe had only drums in the intro and read as techno (`sounds more like 'techno music' than 'house music'`, by z +3.5
to +3.4); putting the filtered piano in the intro tipped it to house. The mix notes point at the hats: there is too
much above 8 kHz and not enough at 4 to 8 kHz, which a brighter clap and a darker hat would balance.

Try:

- Swap the piano for a Rhodes (`.s("rhodes")`) and the lead for a piano for a deeper, softer track.
- Replace the offbeat bass with the root on every beat, `struct("x*4")`, for a harder, more driving groove.
- Put the vocal in the build too, so its hook is introduced before the drop.

## Deep house: "Velvet Hours"

```js packs
blank("deep_house");
title("Velvet Hours");
tempo(121); key("F minor"); swing(0.57);
prog("A", "Fm9 Bbm9 Dbmaj7 Cm7");
prog("B", "Dbmaj7 Cm7 Bbm9 Bbm9");
bars_per_chord(1);

track("kick",   s("bd*4").bank("tr909").gain(0.9));
track("rim",    s("~ ~ rim ~ ~ ~ rim ~ ~ ~ rim ~ ~ rim ~ ~").bank("tr909").gain(0.6).pan(0.3).delay(0.1));
track("hat",    s("[~ hh]*4").bank("tr808").gain(0.6).pan(0.2));
track("shaker", s("shaker*8").gain("[0.3 0.55]*4").pan(-0.3).level(-2));
track("clap",   s("~ cp ~ cp").bank("tr808").room(0.3).level(-2));

track("bass",   note("<f1 bb1 db2 c2>").struct("x ~ ~ x ~ ~ x ~ ~ ~ x ~ ~ ~ ~ ~").s("sub_bass").legato(0.8));
track("chords", chord("prog").struct("x ~ ~ x ~ ~ ~ ~").voicing(53, 72).s("rhodes").room(0.3).delay(0.12));
track("pad",    chord("prog").voicing(50, 74).s("warm_pad").role("pad").level(-3).room(0.4));
track("lead",   n("<[4 ~ ~ 6 4 ~ 2 ~] [2 ~ ~ 4 3 ~ ~ ~] [4 ~ 5 4 2 ~ ~ ~] [~ ~ 1 2 4 ~ ~ ~]>")
                  .octave(4).s("vibes").room(0.35).delay(0.25));

form([
  sec("intro", 8, "kick hat shaker rim chords").filter_in(),
  sec("groove", 16, "kick hat shaker rim clap bass chords"),
  sec("break", 8, "pad chords"),
  sec("build", 8, "kick hat shaker pad chords lead"),
  sec("drop", 16, "kick hat shaker rim clap bass chords pad lead"),
  sec("outro", 8, "kick hat shaker rim bass"),
]);
```

Why:

- **Slower and swung.** 121 BPM and swing 0.57: deep house walks where house runs.
- **Ninth chords.** C is a note of all four chords in `Fm9 Bbm9 Dbmaj7 Cm7`, so the voice-led Rhodes voicings barely
  move and the harmony shimmers rather than steps. The lead does the same: its strong beats land on C and A♭, notes
  most of the chords share.
- **The Rhodes plays only twice a bar**, on the charleston rhythm (`x ~ ~ x`), leaving air. Space is the style.
- **Softer drums.** The 808's clap and hat are rounder than the 909's; the rim shot on a syncopated pattern with a
  touch of delay is the deep house percussion signature.
- **Sub bass** on a syncopated pattern (`x ~ ~ x ~ ~ x ~ ~ ~ x`): felt more than heard, under warm sounds.
- **No impact.** Deep house prefers a riser and a downlifter to a crash and a boom; the genre's transitions leave the
  impact out, and the drop has no `impact()`.

```text
  ears: 'house music' z +3.8 (closest confusion 'minimal electronic music' +3.1); production z +1.9 vs 'a low quality recording with hiss' +1.3; p_real 0.52
        sounds like: house music, tech house music, minimal electronic music, drum and bass music
  verdict: 86/100 (arc 0.81, genre 0.91, mix 0.90, production 0.63, theory 1.00). Fix first: dull 4-8 kHz: hats, claps and snare lack bite (presence -31.2 dB, healthy from -26)
```

The deep house genre score is strong: the ears hear house clearly over its confusions. The one issue, a dull
presence range, is the style's own softness (808 hats, a warm pad, a lowpassed master); raising the clap and the hat
by 2 to 3 dB fixes it if you want it brighter.

Try:

- `prog("A", "Gm9 C9 Gm9 C9")` with `key("G dorian")`: the dorian vamp, two chords for the whole track.
- A walking bass through each chord's notes, one per beat:
  `note("<[f1 ab1 c2 eb2] [bb1 db2 f2 ab1] [db2 f2 ab2 c2] [c2 eb2 g2 bb1]>")` without the `struct()`.

## Techno: "Hollow Engine"

```js packs
blank("techno");
title("Hollow Engine");
tempo(130); key("F phrygian"); swing(0.5);
prog("A", "i i II i");
prog("B", "i");
bars_per_chord(2);

track("kick",  s("bd*4").bank("tr909"));
track("hat",   s("hh*16").bank("tr909").gain("[0.35 0.25 0.7 0.25]*4").pan(0.35));
track("ohat",  s("[~ oh]*4").bank("tr909").gain(0.6).pan(-0.3).room(0.2));
track("clap",  s("~ cp ~ cp").bank("tr909").room(0.25).level(-2));
track("rim",   s("rim(5,16,2)").bank("tr909").gain(0.7).pan(0.35).delay(0.2));
track("ride",  s("[~ rd]*4").bank("tr909").gain(0.5).pan(0.25));
track("roll",  s("<cp*2 cp*4 cp*8 [cp*16]>").bank("tr909").gain("<0.5 0.65 0.8 1>"));

track("bass",  note("<f1!4 gb1!2 f1!2>").struct("[~ x x x]*4").s("saw_bass").legato(0.6).level(-3));
track("acid",  note("f2 f2 f3 f2 ab2 f2 gb2 f2 f2 c3 f2 eb3 f2 f2 db3 f2")
                 .velocity("1 0.6 0.9 0.6 0.7 0.6 1 0.6 0.6 0.9 0.6 1 0.6 0.6 0.9 0.7")
                 .legato(0.7).s("acid").role("synth").delay(0.3).pan(0.2).level(-4));
track("stab",  chord("prog").struct("~ x ~ ~ ~ ~ x ~").voicing(52, 70).s("stab").room(0.5).delay(0.4).pan(-0.2).level(-1));
track("pad",   chord("prog").voicing(45, 70).s("warm_pad").role("pad").room(0.45).level(-2));

form([
  sec("intro", 8, "kick hat rim stab").filter_in(),
  sec("groove", 16, "kick hat ohat clap rim bass"),
  sec("break", 8, "pad stab acid").prog("A"),
  sec("build", 8, "kick hat clap acid roll").gap(),
  sec("drop", 16, "kick hat ohat clap rim ride bass stab pad").impact(),
  sec("outro", 8, "kick hat rim"),
]);
```

Why:

- **Phrygian.** F phrygian has a G♭ a semitone above the tonic, and the progression only moves between F minor and
  G♭ (`i i II i`, two bars a chord). That half step is the darkness of techno; there is almost no harmony beyond it.
- **A rolling bass.** Three sixteenths after every kick (`[~ x x x]*4`), never on the beat, so kick and bass interlock
  into one continuous low-end engine. `saw_bass` keeps it audible without piling on sub.
- **An acid line**, sixteen steps with accents written as velocities: the `acid` preset's resonant filter opens on
  the loud steps, which is the 303 sound. It plays in the break and the build, then leaves the drop to the drums and
  stabs.
- **Euclidean percussion.** `rim(5,16,2)` spreads five rim shots over the bar against the four kicks, and the ride
  joins in the drop. Panned hats, rim and ride keep the dry, mono-heavy low end from making the whole track mono.
- **Stabs with long delay and reverb** fill the space between hits, two per bar, and turn into echoes.
- **A clap roll** instead of a snare: techno builds on the clap.

```text
  ears: 'techno music' z +4.5 (closest confusion 'drum and bass music' +3.5); production z +1.3 vs 'a distorted clipped recording' +1.5; p_real 0.63
        sounds like: techno music, drum and bass music, tech house music, house music
  verdict: 86/100 (arc 0.82, genre 1.00, mix 0.92, production 0.45, theory 1.00). Fix first: nearly mono (correlation 0.97): widen pads, chords and effects; sounds about as much like a clipped recording as a professional mix: less drive and saturation, or a quieter master
```

A perfect genre score: nothing about this reads as anything but techno. The production score is the weak point; the
ears hear the saturated 909, the drive of the acid and the techno master's saturation as close to "distorted". That is
partly the style. Lowering the acid further, or keeping it out of the build too, raises production at some cost to
character; the call is yours.

Try:

- A 32-bar intro and groove, as techno DJs expect: change the bar counts and nothing else.
- `prog("B", "i")` is there for a reason: put `.prog("B")` on the break for eight bars of pure F minor.
- Replace the acid with stabs on every offbeat, `struct("[~ x]*4")`, for a 90s Detroit feel.

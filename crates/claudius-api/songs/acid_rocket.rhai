// "Acid Rocket": 1990s big beat, the rock-sized breakbeat music at the end of the decade (The Prodigy, The Chemical
// Brothers, Fatboy Slim). An original song in that style.
//
// How it is made:
// - A funk break sped up to 132 BPM and hit hard: the drums are distorted and crushed in parallel, louder than
//   anything else in the song.
// - A squelching acid bass line on sixteenths, the TB-303's sound, the hook of the whole track.
// - Two chords only (Em, D): big beat is riffs and drums, not harmony.
// - Stabs and a square lead's two-note siren for the drop, and a break with only the stabs, before the beat crashes
//   back in.
blank("big_beat");
title("Acid Rocket");
tempo(132); key("E minor"); swing(0.52);
prog("A", "i i VII VII");
bars_per_chord(1);

// drums in steps, as the Drums tab shows them: X accent, x hit, o ghost, . rest, sixteen a bar
drum("kick", "X.....x.....x...");
drum("snare", "....X.......x..x");
drum("hat", "..x...x...x...x.");
// the whole kit crushed and distorted in parallel on its bus, under the clean drums
bus("drums", [
  #{ fx: "parallel", dry: 1, chains: [
    #{ amount: 0.5, fx: [#{ fx: "compressor", threshold_db: -30, ratio: 8, attack_ms: 2, release_ms: 80 },
                         #{ fx: "saturate", kind: "hard", drive_db: 9 }] },
  ] },
]);
track("acid", note("<[e2 e2 e3 e2 g2 e2 a2 e2 e2 e3 e2 b2 e2 d3 e2 g2]!2 [d2 d2 d3 d2 f#2 d2 a2 d2 d2 d3 d2 a2 d2 c3 d2 f#2]!2>")
                .s("acid").legato("0.5 0.9").role("bass"));
track("stab", chord("prog").voicing(55, 72).struct("x ~ ~ x ~ ~ x ~").s("stab").legato(0.3).pan(-0.3).role("chords"));
fx("stab", [#{ fx: "width", width: 1.6 }]);
track("siren", note("<[e4 b4]!2 [d4 a4]!2>").s("square_lead").legato(1).delay(0.3).room(0.3).pan(0.3).role("lead"));
// the delay's echoes from side to side, which widen a mix of mono drums and a mono bass
send_fx(#{ delay: #{ fx: "delay", beats: 0.75, feedback: 0.35, damping: 0.5, pingpong: true, mix: 1 } });

form([
  sec("intro", 8, "kick snare hat"),
  sec("build", 8, "kick snare hat acid").build(),
  sec("drop", 16, "kick snare hat acid stab siren").impact(),
  sec("break", 8, "stab siren"),
  sec("build", 8, "kick snare hat acid").build().gap(),
  sec("drop", 16, "kick snare hat acid stab siren").impact(),
  sec("break", 8, "kick snare hat stab"),
  sec("drop", 16, "kick snare hat acid stab siren").impact(),
  sec("outro", 8, "kick snare hat acid"),
]);

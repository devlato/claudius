# Arrangement and energy

The arrangement is the form: a list of sections, each with a kind, a length, an energy and the tracks that play in
it. In the studio it is the grid of tracks against sections; in a script it is [`form()`](../../language/form.md).
The arranger then does what a producer does by hand: tiles the parts across their sections, switches drum variants,
adds fills at phrase ends, rolls and risers into drops, crashes on downbeats, filter sweeps in intros and outros.

## Forms by genre

The generator picks one of the genre's forms. They are a useful starting point for your own:

| genre | form (bars) |
|---|---|
| house | intro 16, groove 16, break 16, build 8, drop 32, break 8, drop 16, outro 16 (or a 64-bar form) |
| deep house | intro 16, groove 16, break 16, build 8, drop 32, outro 16 |
| techno | intro 32, groove 32, break 16, build 8, peak 32, outro 16 |
| trap | intro 8, hook 16, verse 16, hook 16, verse 8, hook 16, outro 8 |
| lofi | intro 4, A 16, B 16, A 16, outro 8 |
| drum and bass | intro 16, build 16, drop 32, break 16, drop 32, outro 16 |
| future bass | intro 8, verse 16, build 8, drop 16, break 8, build 8, drop 16, outro 8 |
| synthwave | intro 8, verse 16, chorus 16, verse 16, chorus 16, bridge 8, chorus 16, outro 8 |

`song(genre, #{ short: true })` halves every section (to at least 4 bars), which is handy for sketching.

## The tools for energy

Energy in a track comes from what plays, how dense it is, and the transitions between sections. claudius gives you:

- **layers**: which tracks play in each section, the strongest tool;
- **drum variants**: a lane's `sparse`, `main`, `alt` or `half` pattern per section;
- **progressions**: a break on progression `B` changes colour without changing key (a song without a `B` plays `A`
  there);
- **flags**: `filter_in` (a lowpass opening across the section), `build` (riser, highpass sweep, snare roll, kick out
  for the last bar), `gap` (a beat of silence), `impact` (crash, impact, downlifter on the downbeat), `fills`
  (drum fills every 8 bars);
- **automatic** touches: a crash every 8 bars in drops and hooks, and a lowpass closing over a final outro.

The exact rules (how long the roll is, which tracks a gap silences, where fills fall) are in
[What the flags do](../../language/form.md#what-the-flags-do).

## Energy values and the critic

Each section's `energy` (0 to 1) is a statement of intent, and the critic holds you to it. It measures the master's
short-term loudness (3-second windows, every half second), takes the median of each section (skipping its first
1.5 seconds and last second), and checks:

- that loudness rises and falls with energy: the correlation between the two across sections, which sets most of the
  arc score;
- that the loudest drop or hook is at least 3 LU louder than the quietest break (aim for 3 to 7).

A low arc score means the energies and the layers disagree. The usual causes:

- an intro with the full kick: drums are never filtered by `filter_in`, so an intro at energy 0.4 can be as loud as a
  groove at 0.65; give the intro fewer drum layers or a higher energy;
- a build without a kick that is quieter than the break before it: either keep the kick until the build's last bar
  (drum lanes do that by themselves), or give the build a lower energy;
- a break that keeps the bass or the drums: the drop cannot be 3 LU louder than a break that is almost as full.

## Building an arrangement

On a generated song, `form()` rewrites the arrangement with the song's own tracks:

```js
form([
  sec("intro", 16, "kick hat perc pad").filter_in(),
  sec("groove", 16, "kick clap hat ohat perc bass chords"),
  sec("break", 16, "pad chords lead").energy(0.3),
  sec("build", 8, "clap hat pad lead chords").gap(),
  sec("drop", 32, "*").impact(),
  sec("break", 8, "pad arp").build(),
  sec("drop", 16, "*").energy(1.0).impact(),
  sec("outro", 16, "kick hat perc bass"),
]);
```

The second break plays progression `B` (breaks do by default) and builds back into `drop2` with a riser, a highpass
sweep and a roll on the clap lane (this song has no snare lane). The two drops are named `drop` and `drop2`.

Smaller edits leave the rest alone:

```js
section("drop", #{ bars: 32 });                  // a longer drop
section("break", #{ layers: "pad lead" });        // a sparser break
layer("groove", "shaker", true);                  // the shaker in the groove too
section("intro", #{ variant: "main" });           // the full drum patterns in the intro
```

## Patterns in the arrangement

A pattern track plays its pattern from the first bar of each section it is in. It is not rewritten by builds or
fills, so a song made of patterns arranges its own builds:

```js packs
blank("house");
tempo(124); key("A minor"); prog("A", "Am7 Fmaj7 Dm7 Em7"); bars_per_chord(1);
track("kick",  s("bd*4").bank("tr909"));
track("hats",  s("[~ hh]*4").bank("tr909"));
track("clap",  s("~ cp ~ cp").bank("tr909"));
track("roll",  s("<sd*2 sd*2 sd*4 sd*4 sd*8 sd*8 sd*16 sd*16>").bank("tr909")
                 .gain("<0.4 0.45 0.5 0.55 0.65 0.75 0.85 1>"));
track("keys",  chord("prog").struct("x ~ ~ x ~ ~ x ~").s("epiano"));
track("pad",   chord("prog").s("pad").role("pad"));
form([
  sec("intro", 8, "kick hats keys").filter_in(),
  sec("groove", 16, "kick hats clap keys"),
  sec("break", 8, "pad keys").energy(0.3),
  sec("build", 8, "hats clap roll pad").gap(),
  sec("drop", 16, "kick hats clap keys pad").impact(),
]);
```

The `roll` track only plays in the build, rising from half notes to sixteenths over its eight bars, and the build flag
adds the riser and the highpass sweep. The break plays progression `A`, since this song has no `B`. Because the roll's
alternation starts from its first item in every section, it lines up with the build however long the sections before it
are.

## Length and phrasing

The critic expects a song of 90 to 330 seconds, and every section a whole number of 4-bar phrases; anything else
costs form points. Fills fall every 8 bars from the start of a section, and drops and hooks crash every 8 bars, so
sections of 8, 16 or 32 bars keep phrases, fills and crashes aligned.

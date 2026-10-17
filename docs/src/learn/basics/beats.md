# Beats: tempo, bars and steps

Play the kick from the last page and tap along with it. Each tap is a **beat**: the steady pulse you would nod your
head to. Everything in this page is about where sounds fall in relation to that pulse.

## Tempo

How fast the beats come is the **tempo**, counted in beats per minute (BPM). Set it at the top of the script:

```js
blank("house");
tempo(120);
track("kick", s("bd*4"));
```

At 120 BPM there are two beats every second. House music lives around 120 to 126 BPM, hip-hop around 85 to 95,
drum and bass around 174. Try `tempo(90)` and `tempo(160)` and hear how much the same kick changes character.

## Bars

Now count along with the kick: *one, two, three, four, one, two, three, four*. Each group of four beats is a
**bar**. Musicians count in bars because almost everything in a song repeats in groups of four beats, and bigger
ideas come in groups of 4, 8 or 16 bars.

In claudius, **one pattern string is one bar**. `s("bd*4")` means "fit four kicks into the bar", one on each beat.
`s("bd bd bd bd")` is the same thing written out: the words share the bar equally. A kick on every beat is called
**four on the floor**, and it is the heartbeat of house and techno.

## The backbeat

Add a clap. Listen for where it lands before reading the explanation:

```js
blank("house");
tempo(120);
track("kick", s("bd*4"));
track("clap", s("~ cp ~ cp"));
```

The clap falls on beats two and four. `~` is a **rest**, a beat of silence, so `"~ cp ~ cp"` reads as *rest, clap,
rest, clap*. Hits on two and four are called the **backbeat**, and you have heard it in nearly every pop, rock and
dance song.

## Between the beats

Each beat can be divided into smaller **steps**. Square brackets squeeze several sounds into one step:

```js
blank("house");
tempo(120);
track("kick", s("bd*4"));
track("clap", s("~ cp ~ cp"));
track("hats", s("[~ hh]*4"));
```

`[~ hh]` is a rest and a hi-hat sharing one beat, so the hat lands exactly halfway between two kicks; `*4` repeats
that in all four beats. Count *one AND two AND three AND four AND*: the hats are on every *AND*, the **offbeats**.
This "tss" between the thumps is what makes house sound like house.

The usual ways of dividing a beat:

| written | per bar | sounds like |
|---|---|---|
| `bd*4` | 4 | one per beat |
| `hh*8` | 8 | two per beat: *one and two and* |
| `hh*16` | 16 | four per beat: *one e and a two e and a* |
| `[~ hh]*4` | 4, between the beats | the offbeats |

Sixteen steps per bar are a common grid. Producers call each one a **step**, and talk about "a hit on step 5" (the
second beat) or "a ghost note on step 8".

## A fuller groove

Add a shaker playing sixteen soft steps, so you can hear the grid filled in:

```js
blank("house");
tempo(120);
track("kick",   s("bd*4"));
track("clap",   s("~ cp ~ cp"));
track("hats",   s("[~ hh]*4"));
track("shaker", s("sh*16").gain(0.4));
```

`.gain(0.4)` plays the shaker softly: gain is how hard each sound is hit, from 0 to 1. Soft, busy parts like this
make a groove feel alive without getting in the way.

## Swing

Straight sixteenth steps sound mechanical. **Swing** delays every second sixteenth a little, so the steps go
long-short, long-short, the way a drummer's hands do:

```js
blank("house");
tempo(120);
swing(0.58);
track("kick",   s("bd*4"));
track("clap",   s("~ cp ~ cp"));
track("hats",   s("[~ hh]*4"));
track("shaker", s("sh*16").gain(0.4));
```

0.5 is straight; 0.66 is a full triplet swing. Listen to the shaker: at 0.58 it bounces. House usually swings a
little (0.52 to 0.56), lofi hip-hop a lot (0.58 to 0.62).

## Try this

- Make the kick play only on beats one and three: `s("bd ~ bd ~")`. The groove relaxes into half the energy.
- Add a kick on the very last sixteenth of the bar, a little push into the next bar: `s("bd bd bd [bd ~ ~ bd]")`.
  The last beat is split into four sixteenths, and the extra kick sits on the fourth.
- Replace the hats with eight-per-bar hats, `s("hh*8")`, and hear how the feel gets busier.
- Play the drums on a real drum machine, the Roland TR-909 used on countless house records (this needs the sample
  packs, which `claudius setup` installs):

```js packs
blank("house");
tempo(120);
track("kick", s("bd*4").bank("tr909"));
track("clap", s("~ cp ~ cp").bank("tr909"));
track("hats", s("[~ hh]*4").bank("tr909"));
```

## The song so far

Keep this in your `song.rhai`; the next pages add to it.

```js
blank("house");
title("First Light");
tempo(120);
track("kick",   s("bd*4"));
track("clap",   s("~ cp ~ cp"));
track("hats",   s("[~ hh]*4"));
track("shaker", s("sh*16").gain(0.4));
```

[Mini-notation](../../language/mini-notation.md) has every symbol you can use in a pattern string.

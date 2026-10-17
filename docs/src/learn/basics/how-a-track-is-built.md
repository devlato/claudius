# How a track is built

This course takes you from no music knowledge to a finished song you understand, one page at a time. Each page adds
one layer to the same song, explains the musical idea behind it in plain words, and gives you something to listen
to. You need claudius installed (see [Install](../../getting-started/install.md)) and a text editor.

## Listen first

Before writing anything, listen to a track claudius makes on its own:

```sh
claudius make --genre house --seed 9
```

It takes under a minute. When it finishes, it prints where it wrote the song; open the `song.mp3` in that
folder and listen with headphones if you have them. Then listen again and try to follow one sound at a time:

- **the kick**, the deep thump that comes four times in every count of four;
- **the clap** answering it on every second thump;
- **the hi-hats**, the short ticking sounds between the thumps;
- **the bass**, the low, round notes that move underneath;
- **the chords**, several notes played together, which give the music its mood;
- **the melody**, a single line of notes on top that you could hum;
- **the pad**, a soft, long sound that fills the background in the quieter parts.

Notice also that the song changes over time. It starts thin, fills up, drops to almost nothing in the middle, builds
up with a rising sound, and comes back full. Those changes are **sections**.

## Layers and sections

Almost every track in electronic music, and most pop, is built this way:

| layer | its job | in this course |
|---|---|---|
| drums | the beat you move to | [Beats](beats.md) |
| bass | weight, and the ground the chords stand on | [Bass lines](bass.md) |
| chords | harmony: the mood, happy or sad, calm or tense | [Chords](chords.md) |
| melody | the part you remember | [Melody](melody.md) |
| structure | when each layer plays | [Song structure](structure.md) |
| mix | how loud each layer is, and where it sits | [Mixing](mixing.md) |

Notes and keys ([Notes, scales and keys](notes-and-keys.md)) come between drums and bass, because bass, chords and
melody all need them. The last page, [Listening, judging, finishing](finishing.md), is about hearing what is wrong
and fixing it.

In claudius, a song is a set of **tracks** (one per layer, or more) and a list of **sections**, each saying which
tracks play in it. You describe both in a **script**, a short text file, and claudius turns it into audio.

## Your first script

Make a file called `song.rhai` with these two lines:

```js
blank("house");
track("kick", s("bd*4"));
```

`blank("house")` starts an empty song. "House" is the style; it decides things you will meet later, like how the
finished song is mixed. `track("kick", s("bd*4"))` adds a track called `kick` that plays the sound `bd` (bass drum)
four times in every bar. You will learn what a bar is on the next page.

Render it:

```sh
claudius script song.rhai
```

claudius prints a summary and writes the song into a folder such as `out/house-untitled-1/`. Open `song.mp3` there:
eight bars of a steady kick drum, about fifteen seconds. That is the start of the song you will build. The studio
does the same with no files at all: paste the script into its script tab (see [The studio](../../getting-started/studio.md)).

On every page, the code is the whole song so far, or a part of it you can add to your file. When a page shows
something to try, change one thing, render, and listen to what changed.

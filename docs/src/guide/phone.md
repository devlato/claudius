# Playing from a phone

A phone can play the open song with drum pads, a keyboard and a turntable, in the song's own sounds. What you record
there goes into the song: drums as its drum lanes, keys as a new part, a scratch as a take. It works in the studio
app and in the browser studio that `claudius serve` serves, with a phone on the same Wi-Fi, and in the
[studio demo](https://claudius.devlato.com/studio/?mock) on the site, with a phone on any network.

## Pairing

The phone button after **Record** in the studio's top bar opens **Play from a phone**. claudius starts serving a page
for phones, and the window shows its address as a QR code: point the phone's camera at it and open the link. The
address carries a key of its own, which the phone keeps, so the page stays open on that phone until the studio stops
serving it (**Stop serving the page**) or claudius quits; the next time, the address has a new key. The window says
when a phone has the page open, and the phone button stays lit while the page is served.

In the demo, the code opens the same page on claudius' site instead, which reaches the demo's tab through a relay on
the site ([below](#with-the-demo)).

On the phone, tap **Start**: a phone plays sound only after a tap. The page shows the song, the section and bar the
studio is at, and the beat. The sounds are the song's own, rendered by claudius as the song mixes them, and the phone
plays them itself, so a tap sounds at once, whatever the Wi-Fi does. An iPhone plays them with its ring switch on
silent too.

## Recording a take

The phone plays the song in the studio as well as following it: **▶** plays it from where it is, and **■** stops it.
Tap ● and the take records along with the song; with the song stopped, ● starts it in the studio a bar before the
section it is at, so you hear a bar first, and the take starts with that section. Tap ● again to send the take: the
song changes as if you had made the edit in the studio, and undo takes it back. The phone keeps its clock with the
computer's and follows the studio's transport, so what you play lands where you heard it, and the delay of the
phone's own speaker is taken off.

- **Pads.** A pad for each of the song's drum lanes, which sounds harder the nearer its top you strike it. A take
  becomes the lanes you struck, folded into a pattern of 1, 2 or 4 bars (as many as you played, up to 4) on the
  sixteenth grid and written as their `main` patterns, as a beat
  [played or beatboxed into the microphone](samples.md#turning-a-take-into-drums) is: a step struck only some of the
  times round plays with that chance, as hard as you struck it on average.
- **Keys.** Two octaves, with **−** and **+** for the octave, playing the sound of one of the song's parts, a synth
  preset or an installed instrument; a key sounds harder the nearer its bottom you strike it. A take becomes a new
  part playing what you played in that sound, in the role you choose (keys, lead, chords, bass or pad), as
  [notes played on a keyboard](samples.md#notes-played-on-a-keyboard) do.
- **Turntable.** A record of two bars of one of the song's tracks, cut from a section the track plays in: a voice, a
  sample, a lead, a stab or horns when the song has one, which scratch as records do. The record waits at its cue, its
  first sound, which a sticker marks as it comes round under the needle. Drag it round to scratch (one turn is 1.8
  seconds of it, as on a record at 33⅓): it follows your finger as a platter follows a hand, its speed changing
  smoothly through each turn back, and moved fast it whistles no highs folded back, as a needle reads a record. The
  crossfader opens within a hair of its travel, as a scratch DJ's does, so a flick cuts the sound in and out. Let go,
  and with **motor** on the record comes up to speed in about a tenth of a second; without it, it comes to rest. A take
  becomes a take of the scratch, `scratch1`, `scratch2` ..., that plays from the start of the section it began in,
  read along your moves as you heard it.

The pads, keys and turntable play without a take too, for practice, and while the studio is stopped.

## MIDI controllers

A MIDI controller plugged into what opens the page plays what is on its screen: drum pads by the General MIDI drum
map (as in [the Record window](samples.md#drums-played-on-pads)), keys by their notes, and the turntable by its jog
wheel and crossfader. For the turntable, tap **Learn MIDI**, turn the jog wheel, then move the crossfader: the page
keeps what it learned. A browser lets a page use MIDI only on a secure connection, so this works on the demo's page,
which is on claudius' site, and, for the app and `claudius serve`, on the computer itself: **Open it on this computer**
in the **Play from a phone** window opens the page there, where a controller plugged into the computer plays it. Chrome
and Edge have MIDI, on Android too; Firefox has it once you allow the site's MIDI add-on it offers; Safari has none, on
an iPhone either, but there the phone is the controller.

## With the demo

The demo runs claudius in the browser tab, which a phone cannot reach, so the tab and the phone both join a room of a
relay on claudius' site, which passes what they say to each other. The room's name is random and in the QR code
alone; the relay keeps nothing of what passes through it. The phone needs the internet rather than your Wi-Fi, and the
tab has to stay open: closing it closes the room (the phone says the demo is not open), and reloading it keeps the
room, so the phone plays on. The demo's engine renders the phone's sounds and takes in the tab, after any render it is
busy with, so the first sounds can take half a minute to arrive.

## When it does not connect

- **The page does not open.** The phone and the computer have to be on the same network, and some networks (a café's,
  a hotel's, a guest network) keep their devices from reaching each other. The computer's firewall may ask whether
  claudius may accept incoming connections: allow it. The page's port is 7879, or another one when 7879 is taken.
- **"This page is closed"** means the studio stopped serving the page, or claudius restarted: open
  **Play from a phone** again and scan the new code.
- **"stopped: play the song in the studio"** stays while the studio is not playing: a take records along with the
  song, so the song has to play.

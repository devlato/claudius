# Playing from a phone

A phone on the same Wi-Fi as your computer can play the open song with drum pads, a keyboard and a turntable, in the
song's own sounds. What you record there goes into the song: drums as its drum lanes, keys as a new part, a scratch as
a take. It works in the studio app and in the browser studio that `claudius serve` serves; the demo does not have it
yet.

## Pairing

The phone button after **Record** in the studio's top bar opens **Play from a phone**. claudius starts serving a page
for phones, and the window shows its address as a QR code: point the phone's camera at it and open the link. The
address carries a key of its own, which the phone keeps, so the page stays open on that phone until the studio stops
serving it (**Stop serving the page**) or claudius quits; the next time, the address has a new key. The window says
when a phone has the page open, and the phone button stays lit while the page is served.

On the phone, tap **Start**: a phone plays sound only after a tap. The page shows the song, the section and bar the
studio is at, and the beat. The sounds are the song's own, rendered by claudius as the song mixes them, and the phone
plays them itself, so a tap sounds at once, whatever the Wi-Fi does. An iPhone plays them with its ring switch on
silent too.

## Recording a take

Play the song in the studio, then tap ● on the phone, and the take records along with the song. Tap it again to send
the take: the song changes as if you had made the edit in the studio, and undo takes it back. The phone keeps its
clock with the computer's and follows the studio's transport, so what you play lands where you heard it, and the
delay of the phone's own speaker is taken off.

- **Pads.** A pad for each of the song's drum lanes, which sounds harder the nearer its top you strike it. A take
  becomes the lanes you struck, folded into a pattern of 1, 2 or 4 bars (as many as you played, up to 4) on the
  sixteenth grid and written as their `main` patterns, as a beat
  [played or beatboxed into the microphone](samples.md#turning-a-take-into-drums) is: a step struck only some of the
  times round plays with that chance, as hard as you struck it on average.
- **Keys.** Two octaves, with **−** and **+** for the octave, playing the sound of one of the song's parts, a synth
  preset or an installed instrument; a key sounds harder the nearer its bottom you strike it. A take becomes a new
  part playing what you played in that sound, in the role you choose (keys, lead, chords, bass or pad), as
  [notes played on a keyboard](samples.md#notes-played-on-a-keyboard) do.
- **Turntable.** A record of two bars of one of the song's tracks, cut from a section the track plays in. Drag it round
  to scratch (one turn is 1.8 seconds of it, as on a record at 33⅓) and move the crossfader to cut the sound; with
  **motor** on, the record plays on when you let go. A take becomes a take of the scratch, `scratch1`, `scratch2` ...,
  that plays from the start of the section it began in.

The pads, keys and turntable play without a take too, for practice, and while the studio is stopped.

## When it does not connect

- **The page does not open.** The phone and the computer have to be on the same network, and some networks (a café's,
  a hotel's, a guest network) keep their devices from reaching each other. The computer's firewall may ask whether
  claudius may accept incoming connections: allow it. The page's port is 7879, or another one when 7879 is taken.
- **"This page is closed"** means the studio stopped serving the page, or claudius restarted: open
  **Play from a phone** again and scan the new code.
- **"stopped: play the song in the studio"** stays while the studio is not playing: a take records along with the
  song, so the song has to play.

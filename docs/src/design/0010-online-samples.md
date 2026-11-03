# 0010 Samples from online libraries

Status: accepted

## Context

Many genres are built from samples: trip-hop from slowed breaks and film-score strings lifted off records, boom bap
from chopped soul and jazz, jungle from breaks cut up and replayed, filter house from disco loops. Synthesizers and
multi-sampled instruments ([0007](0007-sample-licensing.md)) cannot stand in for them, and songs built without them
sound clean and polite, the stock background music the critic now catches
([0006, amended](0006-critic.md#amendment-stock-music)).

The places people hear records online cannot supply them:

- YouTube's terms forbid downloading and automated access, except through YouTube's own features or with its written
  permission, and nearly everything on it belongs to someone;
- Spotify and Apple Music stream behind DRM, and their terms forbid copying it out, previews included;
- SoundCloud's tracks belong to their uploaders unless they chose a Creative Commons licence, and its API registers
  applications only for paying Artist Pro subscribers, under terms of its own;
- for all of them, a sample needs a licence from whoever owns the recording and from whoever owns the composition.

Some libraries do exist to be used: Freesound (sounds under CC0 or CC BY, recorded and uploaded by their authors),
Wikimedia Commons (reviewed recordings under free licences, and records old enough to be in the public domain),
ccMixter (stems and a cappellas shared to be remixed), and Openverse, which indexes Freesound, Commons and Jamendo
and answers without a key.

## Decision

`claudius samples` (`claudius_packs::online`) searches Openverse, Wikimedia Commons and ccMixter, and downloads into a
samples folder of its own, with a `.json` beside each sound naming its title, author, licence and page.

- **Licences** follow 0007: CC0, an author's public domain mark and CC BY are kept; recordings in the public domain by
  age (public domain in the United States, while a tune on them may still be protected where copyright lasts 70
  years after the composer's death) and CC BY-SA sounds come only when asked for (`--us-public-domain`,
  `--share-alike`); non-commercial and no-derivatives licences never come.
- **Suspect uploads**, whose titles or tags say they were cut from someone's record, are left out: an uploader cannot
  license a record they do not own.
- **Credits** are automatic: a song that plays a downloaded sound names it in its `credits.txt`, with the licence and
  the page.
- **No keys**: Openverse's anonymous limits (20 searches a minute, 200 a day) are enough for a person searching, and
  nothing claudius ships holds a secret.

Rejected: grabbing from YouTube, Spotify, Apple Music or SoundCloud (their terms, and the records' owners); a
Freesound key in claudius' source (Freesound asks that keys stay secret, and a shared one would hit its daily limit
for everyone); the Internet Archive's Musopen recordings, which it offers only as ZIP archives of whole albums.

## Consequences

- Sampled genres can be built from material people may publish, and claudius says where every sound came from.
- Openverse serves Freesound's previews, MP3s of about 128 kbps; the originals need a Freesound login (OAuth2), which
  claudius does not ask for.
- claudius depends on the libraries' APIs staying up and keeping their shape; a library that does not answer is
  reported and the others still answer.
- A licence is what the uploader chose. The suspect filter catches only uploads that say what they are, so a sound
  to publish is worth a look at its page.

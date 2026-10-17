# 0007 Which samples we ship

Status: accepted

## Context

Real audio ([0001](0001-real-audio-only.md)) needs recordings, and people publish what they make with claudius. Free
samples are plentiful; samples whose licence is clear are not. In the sets live coders use every day:

- many drum machine sets trace back to anonymous uploads with no stated terms;
- TidalCycles' Dirt-Samples has no licence file, and some of its folders are cut from commercial records;
- some well-made libraries are free to use in music but forbid redistributing the samples themselves (Audio Realism's
  TR-909 pack, the jRhodes3d Rhodes);
- share-alike licences can reach into the music made with the samples.

Putting samples in the repository or the release archives would also make claudius a redistributor, and the
repository huge.

## Decision

claudius ships no samples. Sample packs are installed on demand from their authors' own hosts, at pinned revisions,
by `claudius setup` or `claudius-packs install` (`crates/claudius-packs`). The curated registry records, for each
pack, its licence, its credit line, whether credit is required, and a `licence_note` quoting the licence text and where
it was read.

- **The default set** (14 packs, 162 instruments) contains only packs whose licence lets you make and publish music with
  them, with at most an attribution: CC0, public domain, the Unlicense, CC BY, and free-for-music licences such as
  jRhodes' and Audio Realism's. Packs that forbid redistribution are acceptable because each machine downloads them
  from their author.
- **Credits** are automatic: a song that plays CC BY packs gets a `credits.txt` naming exactly the packs it used.
- **Opt-in** packs, whose licence is unknown or carries conditions (`tidal-drum-machines`, `dirt-samples`,
  `mridangam` under CC BY-SA), install only with `--accept-licence` and print a warning.
- **User maps** added with `add-map` record the licence the map or its repository declares, or `unknown` with a
  warning that also appears in the credits of every song that uses them.
- **Apple Loops** are read where GarageBand or Logic installed them, under Apple's licence for using them in your own
  music, and never copied, except the drum one-shots claudius cuts into its cache.
- Where no clearly licensed recording exists, claudius does without or builds one: the choir is a single sung vowel,
  CC0, played as a five-voice ensemble, and the uzu kit's processed Amen break is left out.

Rejected: bundling packs (redistribution, and a gigabyte-sized repository); enabling the popular unlicensed sets by
default (users would publish samples they have no right to); limiting claudius to CC0 (it would lose the best piano,
the electric pianos and the acoustic kit, all CC BY).

## Consequences

- Music made with the default set can be published, with the credits claudius writes.
- Every licence decision can be checked by reading `registry.json`, without repeating the research.
- The first run downloads about 2.2 GB from a dozen hosts, and claudius depends on those hosts staying up. GitHub-hosted
  packs are pinned by commit; the two archives (the 909 and the FreePats organ) are not pinned by hash, so a compromised
  host could serve different audio ([Security](../engineering/security.md#downloads)).
- Strudel's default drum-machine banks, which Strudel users expect, are opt-in.
- The library has gaps where no clearly licensed recording exists: no multi-sampled choir, no vocals with words, a piano
  cut to 5 of its 16 layers for size.
- Adding a pack means reading its licence at the source and quoting it ([Sample packs](../engineering/packs.md#adding-a-pack)).

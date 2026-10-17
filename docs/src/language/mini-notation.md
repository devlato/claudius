# Mini-notation

Mini-notation is the little language inside pattern strings: `s("bd*4, [~ cp]*2")`. It comes from TidalCycles and
Strudel, and claudius implements most of it. This page is the exact grammar, taken from the tokenizer and parser in
`crates/claudius/src/pattern/mini.rs`, followed by what each construct means and where claudius differs from Strudel.

A string is parsed when the pattern is built, so a syntax error stops the script at the line that wrote it.

## Grammar

Two levels: the tokenizer cuts the string into tokens, the parser builds a pattern from them. Whitespace separates
tokens and is otherwise ignored, so `a !` and `a!` are the same.

### Tokens

```ebnf
token       = symbol | rest | hold | number | word ;
symbol      = "[" | "]" | "<" | ">" | "{" | "}" | "(" | ")"
            | "," | "|" | "*" | "/" | "@" | "!" | "?" ;
rest        = "~"
            | "-" ;                (* only when the next character is not a digit or "." *)
hold        = "_" ;                (* only when the next character is not a word character *)
number      = num-start { digit | "." } ;
                                   (* not followed by a word character other than ".";
                                      the text must read as a decimal number, else "bad number" *)
word        = word-start { word-char | "/" ascii-letter }
                                   (* a slash followed by a letter stays in the word: "F/A", "V7/V" *)
            | num-start { digit | "." } word-char-not-dot { word-char } ;
                                   (* "808bd", "3am", "1-2": a number run that continues into letters *)
word-start  = letter | "#" | ":" | "_" | "'" | "^" ;
num-start   = digit | "-" | "." ;
word-char   = letter | digit | "#" | ":" | "." | "_" | "'" | "-" | "^" ;
word-char-not-dot = word-char - "." ;
letter      = (* any Unicode alphabetic character *) ;
ascii-letter = "a" .. "z" | "A" .. "Z" ;
digit       = (* any Unicode digit *) ;
```

A `/` followed by a letter belongs to the word it is in, so slash chords (`F/A`) and applied chords (`V7/V`) are
single words. Anywhere else `/` is the "slower" modifier: `[a b]/2`, `a/2`.

Any other character is an error: `unexpected '%' in "bd%4"`. That includes `%`, `$`, `=`, `+`, `"`, `°`, `♭` and
`♯`, so write `bb` rather than `B♭`, and `o` or `dim` rather than `°`.

### Patterns

```ebnf
pattern     = stack ;                                (* the whole string *)
stack       = sequence { ( "," | "|" ) sequence } ;
sequence    = { step | "_" | "!" } ;                 (* may be empty: silence *)
step        = atom { modifier } ;
atom        = "[" stack "]"
            | "{" stack "}"
            | "<" alternation
            | "~" | "-"
            | word
            | number ;
alternation = alt-sequence { "," alt-sequence } ">" ;
alt-sequence = { step | "!" } ;
modifier    = "*" number
            | "/" number
            | "@" number
            | "!" [ number ]
            | "?" [ number ]
            | "(" number "," number [ "," number ] ")" ;
```

Rules the grammar alone does not show:

- In a `sequence`, `_` adds one step of length to the step before it, and a bare `!` repeats the step before it.
  Either one with no step before it is an error.
- Inside `< >`, `_` and `|` are errors, and `@` weights are read but ignored.
- `*` and `/` take a number above 0 and up to 256.
- A `stack` mixes `,` and `|` badly: write `[a | b], c`, not `a | b, c` (see [Differences](#differences-from-strudel)).
- Size limits: a string is at most 4096 characters, brackets nest at most 64 deep, a sequence has at most 1024
  steps, `!n` repeats a step at most 256 times, and a euclidean rhythm has at most 256 slots.

## What each construct means

A pattern string describes one **cycle**, and in claudius one cycle is one bar. The constructs below divide the cycle,
layer things in it, or change from one cycle to the next. [Patterns and time](patterns.md) explains cycles in full.

| syntax | example | meaning |
|---|---|---|
| sequence | `bd sd bd sd` | the steps share the cycle equally: four steps are four beats |
| rest | `bd ~ bd ~`, `bd - bd -` | a step of silence |
| group | `bd [sd sd]` | a sequence squeezed into one step |
| braces | `{bd sd}` | the same as `[bd sd]` |
| stack | `bd*4, hh*8` | play together, each with the whole cycle |
| alternation | `<a1 f1 c2 g1>` | one item per cycle, in turn |
| choice | `bd \| cp \| rim` | one sequence per cycle, picked at random |
| fast | `hh*8` | the step plays 8 times within its slot |
| slow | `[bd sd]/2` | the step's content spreads over 2 cycles |
| weight | `a@3 b` | the step takes 3 shares of the cycle (here 3/4) |
| hold | `a _ _ b` | the same as `a@3 b` |
| repeat | `a!3 b`, `a ! b` | the step repeated as separate steps (`a a a b`, `a a b`) |
| drop | `hh?`, `hh?0.3` | the step plays with probability 1 − p (p = 0.5, or the number) |
| euclid | `bd(3,8)`, `bd(3,8,2)` | k hits spread over n slots, rotated left by r |
| index | `bd:3` | a word carrying a variation number (see [Sounds](sounds.md#variations)) |

Modifiers apply to the step they follow and chain left to right: `hh*2?` drops some of the doubled hits,
`[bd sd](3,8)` plays the pair inside each euclidean hit.

### Sequences and weights

A sequence divides the cycle by weight. Every step weighs 1 unless `@` says otherwise, and each `_` after a step
adds 1:

```js
blank("house");
track("lead", note("e4@3 d4 c4@2 d4 e4").s("pluck"));   // 3/8, 1/8, 2/8, 1/8, 1/8 of the bar
```

`!n` writes the step n times; each copy is a full step. `a!` with no number means one more copy, and `!` after
whitespace is the same token, so `a ! b` is `a a b`. A number after a bare `!` is read as the count: `a ! 3` is
`a a a`, not `a a 3`.

### Groups, stacks and alternation

`[ ]` makes a sub-sequence that takes one step. A comma at the top level, inside `[ ]` or inside `< >` stacks layers
that all take the full span:

```js
blank("house");
track("drums", s("bd*4, [~ cp]*2, [~ hh]*4, hh*16?0.6"));
```

`< >` plays one item per cycle: `<a1 f1 c2 g1>` is A in bar 1, F in bar 2, C in bar 3, G in bar 4, then again.
Items can be groups, and alternations nest: `<a <b c>>` plays a, b, a, c. Inside `< >`, `!` repeats an item:
`<c2!2 ab1!2>` plays C in bars 1 and 2, then A♭ in bars 3 and 4, a new event in every bar. `<c2 ab1>/2` covers the
same four bars with two events, each two bars long.

### Speed

`*n` plays the step's content n times within its slot; `/n` stretches it over n slots, so it continues in the next
cycle. In `x [a b c d]/2`, the second half of bar 1 plays `a b` and the second half of bar 2 plays `c d`. The
number can be a decimal above 0 and up to 256; it cannot be a pattern (`bd*<2 3>` is an error).

### Euclidean rhythms

`x(k,n,r)` puts k hits on n equal slots as evenly as possible, with Bjorklund's algorithm as Tidal and Strudel run
it, and rotates the result left by r slots. A k larger than n fills every slot; an n of 0 is silence.

| pattern | slots (x hit, . rest) |
|---|---|
| `x(3,8)` | `x..x..x.` |
| `x(3,8,2)` | `.x..x.x.` |
| `x(5,8)` | `x.xx.xx.` |
| `x(5,8,2)` | `xx.xx.x.` |
| `x(3,16)` | `x....x....x.....` |
| `x(5,16)` | `x..x..x..x..x...` |
| `x(7,16)` | `x..x.x.x..x.x.x.` |

### Randomness

`?` and `|` are random, but not from run to run: their random numbers are seeded from the text of the string. The
same string gives the same choices on every render, a different bar gets a different choice, and any edit to the
string (even a space) gives new choices. `hh*16?0.3` thins a 16th-note hat differently in each bar of a section, and
identically every time you render.

## Values

What a word or number means depends on the pattern it is in:

| pattern | words | numbers |
|---|---|---|
| `s()` / `sound()` | sound names, `name:2` for a variation, `pack:instrument` | sound names (`808`) |
| `note()` | note names: `c`, `c#4`, `eb3`, `fs2` | MIDI note numbers: `60` is middle C |
| `n()` | an error | scale degrees from 0 |
| `chord()` | chord symbols, slash chords, Roman numerals, `prog`, `prog:B` | an error |
| `seq()`, and arguments of `.gain()`, `.struct()` ... | `x`, `t`, `true` | amounts |

In a sound name, only a number after the last colon is a variation: `bd:3` and `tr808:sd:2` carry one, `tr808:sd`
is a single name. [Pitch](pitch.md) has the note, scale and chord rules. In `.struct()` and `.mask()`, a value is
"on" when it is a non-zero number or any word except `0`, `false` and `f`.

## Errors

Errors name the token the parser did not expect. These are the messages, as `claudius script` prints them:

```js error
blank("house");
track("x", s("[bd sd"));
```

```text
Error: Runtime error: expected ']', found the end (line 2, position 12)
```

| string | error |
|---|---|
| `bd sd]` | `unexpected ']'` |
| `[bd sd` | `expected ']', found the end` |
| `<bd sd` | `missing '>'` |
| `bd(3,8` | `expected ')', found the end` |
| `bd(3)` | `expected ',', found ')'` |
| `bd*<2 3>` | `expected a number, found '<'` |
| `bd/0` | `'/' takes a number above 0 and up to 256, not 0` |
| `bd*1000` | `'*' takes a number above 0 and up to 256, not 1000` |
| `_ bd` | `'_' needs a step before it` |
| `! bd` | `'!' needs a step before it` |
| `<bd \| sd>` | `unexpected '\|'` |
| `<bd _ sd>` | `unexpected '_'` |
| `bd . sd` | `bad number '.'` |
| `bd%4` | `unexpected '%' in "bd%4"` |
| `7/V` | `expected a number, found 'V'` (a word starting with a digit does not take a slash) |
| `bd!300` | `a step repeats at most 256 times` |
| more than 1024 steps in one sequence | `a sequence has at most 1024 steps` |
| `bd(3,300)` | `a euclidean rhythm has at most 256 steps` |
| 65 nested `[` | `brackets are nested more than 64 deep` |
| a 5000-character string | `mini-notation is limited to 4096 characters (this one has 5000)` |

A word that parses but means nothing is not an error at this stage; it is one when the track is made, and the
message says what was expected: `s("bdd")` is an unknown sound, `note("h3")` cannot be read as a note,
`chord("Xyz")` cannot be read as a chord, and `n("0 e")` is not a scale degree. See
[Errors and limits](limits.md#errors-by-function).

## Differences from Strudel

| Strudel | claudius |
|---|---|
| `{a b, c d e}` is a polymeter | braces are plain groups; there is no polymeter and no `%` |
| `a . b c` groups with dots | `.` is an error |
| `<a@3 b>` holds `a` for 3 cycles | weights inside `< >` are ignored; write `<a!3 b>` |
| `a ! 3` is `a a 3` | `a ! 3` is `a a a` |
| `bd*<2 3>` takes a pattern | `*` and `/` take numbers only, above 0 and up to 256 |
| `a \| b, c` | `\|` binds across the comma: it plays `b` with a random pick of `a` or `c`; bracket it: `[a \| b], c` |
| `"c e g"` with a cps of your choice | one cycle is always one bar at the song's tempo |

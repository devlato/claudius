# Rhai in five minutes

Scripts are written in [Rhai](https://rhai.rs), a small scripting language that reads like JavaScript or Rust. You
need very little of it: most scripts are a list of function calls. This page covers the parts that come up in
music scripts. The [Rhai book](https://rhai.rs/book/) has the rest.

## Statements and comments

Statements end with `;`. Line breaks inside a call are fine, which is how long patterns stay readable:

```js
blank("house");   // a comment runs to the end of the line
/* a block comment
   spans lines */
track("lead", n("0 2 4 7")
                .scale("A:minor")
                .s("pluck"));
```

## Values

| value | written | notes |
|---|---|---|
| integer | `8`, `-3` | whole numbers; some functions accept only these |
| decimal | `0.5`, `124.0`, `-6.5` | |
| string | `"bd sd"` | patterns are strings; `\"` and `\\` escape |
| boolean | `true`, `false` | |
| array | `["kick", "hat"]` | |
| map | `#{ level: -3, pan: 0.2 }` | the options of `mix()`, `part()`, `section()`, `song()`, `master()`; a key the function does not know is an error |
| closure | `\|x\| x.fast(2)` | a small function, passed to `every()`, `sometimes()`, `off()` ... |

Integers and decimals are different types. Functions that take a count (`ply(2)`, `every(4, ...)`, `octave(4)`,
`bars_per_chord(2)`, the bars of `sec()`) need an integer, and a decimal there is an error:

```js error
blank("house");
track("lead", n("0 2 4").octave(4.5));
```

```text
Error: Function not found: octave (Pattern, f64) (line 2, position 26)
```

"Function not found" with a list of argument types almost always means one argument has the wrong type. Functions
that take an amount (`tempo()`, `gain()`, `level()`, `swing()`) accept both.

## Variables

`let` makes a variable, `const` a constant. Variables hold anything, patterns included:

```js
blank("house");
let groove = "x ~ ~ x ~ ~ x ~";
const ROOTS = "<a1 f1 c2 g1>";
track("bass", note(ROOTS).struct(groove).s("sub_bass"));
track("keys", chord("<Am F C G>").struct(groove).s("epiano"));
```

Both tracks share one rhythm, so the bass and the chords hit together.

## Calling methods

A pattern method is called with a dot and returns a new pattern, so calls chain left to right. Each method applies
to everything before it:

```js
blank("house");
track("hats", s("hh*8").gain(0.6).pan(0.2).every(4, |x| x.fast(2)));
```

## Closures

Higher-order methods take a closure: `|x| ...` is a function of one pattern, `x`, that returns a pattern. A closure
can use variables from around it:

```js
blank("house");
let up = 7;
track("lead", n("0 2 4 2").scale("A:minor").every(2, |x| x.add(up)).s("pluck"));
```

When the song prints as a script, the closure prints with the value it used: `.every(2, |x| x.add(7))`.

## Functions, loops and conditions

Rhai has `fn`, `for`, `while`, `if` and `switch`. They are useful for building strings and for repeated edits:

```js
song("house", 9);

// the same reverb send on several tracks
for t in ["chords", "pad", "lead"] {
    mix(t, #{ reverb: 0.3 });
}

// a function that writes a pattern string
fn every_beat(sound) { sound + " " + sound + " " + sound + " " + sound }
track("rim", s(every_beat("rim")).gain(0.5));

let late_night = true;
if late_night { tempo(120); swing(0.56); } else { tempo(126); }
```

A function defined with `fn` cannot see variables from outside it; pass what it needs as arguments. Ranges are
written `0..4` (0 to 3) and `0..=4` (0 to 4).

## Strings

Strings join with `+`. Backtick strings fill in `${...}`:

```js
blank("house");
let n = 4;
track("kick", s(`bd*${n}`));
track("hats", s("[~ hh]" + "*4"));
```

## Printing

`print(x)` and `debug(x)` write to the script's log: the terminal for `claudius script`, the log panel in the studio.
Lines printed by the script come after the lines claudius writes for each `track()`.

```js
blank("house");
track("kick", s("bd*4"));
print("kick written");
```

```text
track kick: bd
kick written
```

## What a script cannot do

A script cannot read or write files (except audio files named in `.s()`, see [Sounds](sounds.md#your-own-files)),
import modules, call `eval`, or run forever. It stops with an error after 10 seconds or 5,000,000 operations. The
full list is in [Errors and limits](limits.md#limits).

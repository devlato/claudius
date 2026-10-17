# Making a change

This page walks through one small, complete change: adding a pattern method to the script language, with its test, its
documentation, and its pull request. The same steps, in the same order, fit most changes. [A genre
tweak](#a-genre-tweak) at the end covers the other common kind.

## The change

Strudel has `palindrome()`: a pattern plays forwards on one cycle and backwards on the next. claudius has `rev()` and
`lastOf()`, so you can write it as `.lastOf(2, |x| x.rev())`, but people coming from Strudel look for the name. The
change adds `.palindrome()` as a method on patterns.

First, check it does not exist and see how the neighbours are built:

```sh
rg -n 'palindrome' crates/claudius/src            # nothing
rg -n '"rev"|"lastOf"' crates/claudius/src/script/patterns.rs
```

`lastOf(n, f)` builds `Node::Every(n, n - 1, original, transformed)`: the transformed pattern plays on every cycle where
`cycle % n == n - 1`. A palindrome is `Every(2, 1, p, Rev(p))`, so no new kind of node is needed, and queries, the
work and event budgets and printing need no changes either.

```sh
git switch -c feat/palindrome
```

## The test first

Write the test before the code and watch it fail, so you know it tests the change. Pattern methods are tested in
`crates/claudius/tests/script.rs`:

```rust
#[test]
fn palindrome_plays_backwards_every_other_cycle() {
    let p = script::eval_pattern(r#"s("bd sd hh").palindrome()"#).unwrap();
    let names = |cycle: f64| p.events(cycle, cycle + 1.0).iter().map(|e| format!("{:?}", e.v)).collect::<Vec<_>>();
    let mut backwards = names(0.0);
    backwards.reverse();
    assert_eq!(names(1.0), backwards);
    assert_eq!(names(2.0), names(0.0));
    // the code is kept, so a song with it prints back into itself
    assert_eq!(p.repr, r#"s("bd sd hh").palindrome()"#);
}
```

```sh
cargo test -p claudius --test script palindrome
```

It fails with Rhai's `Function not found: palindrome (Pattern)`.

A palindrome applied twice gives back the original pattern, which is a law, and laws belong in
`crates/claudius/tests/pattern_laws.rs`, inside the `proptest!` block next to `reversing_twice_is_the_original`:

```rust
#[test]
fn palindrome_twice_is_the_original(src in mini_source()) {
    let a = pattern(&format!("s({src:?})"));
    let b = pattern(&format!("s({src:?}).palindrome().palindrome()"));
    prop_assert_eq!(onsets(&a, 0.0, 4.0), onsets(&b, 0.0, 4.0));
}
```

## The code

Pattern methods are registered in `crates/claudius/src/script/patterns.rs`, in `register_patterns`. The time
transformations sit together; add this after `rev`:

```rust
engine.register_fn("palindrome", |p: &mut Pattern| {
    p.with(Node::Every(2, 1, Box::new(p.node()), Box::new(Node::Rev(Box::new(p.node())))), ".palindrome()")
});
```

`Pattern::with` wraps the node and appends `.palindrome()` to the pattern's `repr`, which is what makes a track that
uses it print back as written. It keeps the pattern's kind (sounds, notes, degrees, chords) and its track settings, so
`note("c3 e3 g3").palindrome().s("pluck")` works too. Seeds and randomness need nothing: `Every` and `Rev` are
deterministic.

```sh
cargo test -p claudius --test script palindrome    # passes
cargo test -p claudius --test pattern_laws         # 256 generated patterns, palindrome twice included
cargo test -p claudius --test script               # nothing else changed
```

Then try it the way a user would, and check the round trip by eye:

```sh
echo 'blank("house"); track("hats", s("hh hh*2 oh hh").palindrome());' | claudius script - --print
```

The output should contain `track("hats", s("hh hh*2 oh hh").palindrome());` exactly. Render a short song with it and
listen: `claudius script` with the same code and `--no-ears` writes a song folder in a few seconds.

## The documentation

A language change is not finished until the people using the language can find it:

1. **The language reference** (`docs/src/language/pattern-functions.md`): an entry in the section on time, with what it
   does, an example and what you hear. Use a plain `js` fence, so `docs_examples` runs it; once the method exists it
   passes. On this page the example is marked `no-run`, because the method does not exist yet:

   ```js no-run
   track("perc", s("rim ~ rim [rim rim]").palindrome());   // the figure, then the figure backwards
   ```

2. **The quick reference** (`docs/src/language/quick-reference.md`): one row.
3. **`crates/claudius/src/script_reference.md`**: add `.palindrome()` to the time row. This file is built into the
   program; `claudius reference`, `GET /api/reference` and the MCP tool `script_reference` return it, so it is what
   assistants read before writing scripts. `the_reference_examples_run` in `tests/script.rs` runs its examples.
4. **[Coming from Strudel](../learn/producers/strudel.md)**, if it lists `palindrome` as missing.

Then run the examples:

```sh
cargo test -p claudius --test docs_examples
```

## Before the pull request

```sh
cargo xtask ci
```

runs formatting, clippy with warnings as errors, every test and the docs examples, as CI will. Fix what it reports
rather than silencing it; [Code style and review](review.md) explains the few lints the engine allows and why.

## The pull request

The title is a Conventional Commit, written for the people who use claudius, because it becomes the commit message on
`main` and the line in the changelog:

```text
feat(script): add .palindrome() to patterns
```

The description follows the template: what changes and why ("Strudel users look for palindrome; it is lastOf(2, rev)
under a name they know"), how you checked it (the tests, and that you listened to a render), and anything a reviewer
should look at closely (here, nothing: no new node, no change to printing).

A maintainer reviews it against the checklist in [Code style and review](review.md#what-reviewers-check). Changes
requested on the branch are pushed as new commits; the pull request is squash-merged with its title, so the history on
`main` stays one line per change. You never touch the version or the released sections of `CHANGELOG.md`: the release
process writes the changelog from the titles ([Releasing](releasing.md)). A note that needs more than the title, such
as an upgrade step, goes under `## [Unreleased]` in the same pull request.

## A genre tweak

The other common change is musical: a different weight, template or form in a genre. Say house's hats should play the
open hat less often. In `crates/claudius/data/genres/house.json`, the `ohat` lane's template has a probability per
sixteenth; lower the ones you want rarer, then:

```sh
cargo test -p claudius --test generators       # fails: the house snapshot changed
cargo insta review                             # read the diff, accept it if it is what you meant
claudius make --genre house --seed 42 --short --no-ears    # and listen
```

The snapshot in `crates/claudius/tests/snapshots/generators__house.snap` shows the change as step strings, so the
reviewer reads `drum ohat main (drum open_hat): ..x...x...x...x.|..x...x...x...x.|...` becoming something sparser and
can judge it without running anything. Say in the pull request how it sounds, before and after. A genre file can also
be tried without rebuilding: put a copy with the same `name` in your genres folder ([Files and
settings](../guide/files.md#your-genres)).

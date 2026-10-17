# Contributing

Thanks for wanting to help. Fixes, new genres, better defaults, documentation and good bug reports are all welcome.

## Before you start

- For anything bigger than a fix, open an issue first and say what you want to change. It saves you from writing
  something that does not fit.
- Read [a tour of the repository](docs/src/contributing/onboarding.md) and
  [development setup](docs/src/contributing/setup.md). They take ten minutes and cover everything below in detail.

## How a change gets in

1. Branch from `main`.
2. Make the change, with tests. Bugs get a test that fails before the fix.
3. Run `cargo xtask ci`. It runs what CI runs: formatting, clippy, the tests and the docs examples.
4. Open a pull request whose title is a [Conventional Commit](https://www.conventionalcommits.org/):
   `feat(script): add .chop()`, `fix(render): clicks at loop ends`, `docs: explain swing`. The title becomes the
   commit message and the changelog line, so write it for people who use claudius.
5. CI must pass and a maintainer reviews it. We squash-merge.

Releases are cut from `main` by the release workflow; you never bump versions by hand. See
[releasing](docs/src/contributing/releasing.md).

## What reviewers look for

- The change does one thing, and the pull request says why.
- New behaviour is tested, and music-facing changes say how they sound (a render, a verdict, a snapshot diff).
- Code reads like the code around it. Comments explain what the code cannot: a constraint, a reason, a unit.
- Docs change with the behaviour. Every `js` example in the docs is run by the tests.

The full guide is in [code style and review](docs/src/contributing/review.md), and the writing guide in
[writing docs](docs/src/contributing/docs.md).

## Using an AI agent

That is fine and expected here; see [working with AI agents](docs/src/contributing/agents.md) and `AGENTS.md`. You are
responsible for what you submit, whoever typed it.

## Licence

By contributing you agree that your work is released under the [MIT licence](LICENSE).

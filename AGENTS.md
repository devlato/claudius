# Instructions for AI coding agents

These rules are for any agent working in this repository (Claude Code, Cursor, Codex, and others), and they are
the same rules people follow. The contributor guide in `docs/src/contributing/` explains the reasons; this file is
what you need to act.

<!-- ANCHOR: rules -->
## Rules

- **Never work on `main`.** Make a branch named `<type>/<topic>`, where the type is the kind of change:
  `feat/euclid-patterns`, `fix/render-error-session`, `docs/swing-chapter`. Branches named `release/*` and
  `hotfix/*` belong to the release process; do not make them unless you were asked to release.
- **Every change goes in through a pull request whose title is a Conventional Commit**, because the title becomes
  the commit on `main` and the line in the changelog: `feat(script): add euclid() to patterns`. The type is one of
  `feat`, `fix`, `docs`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`; a `!` after it marks a breaking change
  (`feat(api)!: rename /api/state`). Write it for someone reading the changelog.
- **Run `cargo xtask ci` before you push.** It runs formatting, clippy with warnings as errors (also on the builds
  without the ears or the studio window), the tests and the examples in the docs, which is what CI checks first.
  Fix what it finds. If you touched the docs, the site, dependencies or workflows, run the matching checks from
  `docs/src/contributing/setup.md` too.
- **Keep the two features apart.** Code that needs earshot goes behind `#[cfg(feature = "ears")]`, code that needs
  the native window behind `studio`. The Intel Mac and Windows builds have no ears, and the headless Linux build
  has no window, so both have to keep compiling.
- **Leave `rust-toolchain.toml` alone in a change of your own.** Moving to a newer Rust is a pull request of its
  own, with the lints the new clippy brings.
- **Keep a pull request to one change**, and describe in it how you checked it.
- **Notes for the changelog** that need more than the PR title (an upgrade step, a highlight) go under
  `## [Unreleased]` in `CHANGELOG.md`, in the same pull request.

## Commands

Make a change and open a pull request:

```sh
git switch -c feat/euclid-patterns
# edit, then
cargo xtask ci
git add -A && git commit -m "feat(script): add euclid() to patterns"
git push -u origin HEAD
gh pr create --title "feat(script): add euclid() to patterns" --body "What changes, why, and how I checked it."
```

Watch the checks, and let GitHub merge once the required check, *ci ok*, passes:

```sh
gh pr checks --watch
gh pr merge --auto --squash
```

If a check fails, read its log (`gh run view <run id> --log-failed`), fix the cause on the same branch, and push
again.

Release, only when the person you work for asks for it:

```sh
gh workflow run release-pr.yml -f bump=minor      # or patch, major, or an exact version such as 0.4.0
gh run list --workflow release-pr.yml --limit 1   # the run; `gh run watch <id>` follows it
gh pr view release/v0.4.0                          # the release PR, "release: v0.4.0"; read its notes
gh pr checks release/v0.4.0 --watch                # if no checks appear: gh workflow run ci.yml --ref release/v0.4.0
gh pr merge release/v0.4.0 --squash                # this publishes the release
```

Then check what was published:

```sh
gh run list --workflow release.yml --limit 1       # `gh run watch <id>` until it finishes
gh release view v0.4.0
gh release download v0.4.0 --pattern 'claudius-0.4.0-aarch64-apple-darwin.tar.gz' --pattern SHA256SUMS
shasum -a 256 --check --ignore-missing SHA256SUMS
gh attestation verify claudius-0.4.0-aarch64-apple-darwin.tar.gz --repo devlato/claudius
gh release download v0.4.0 --pattern install.sh --pattern SHA256SUMS --clobber && shasum -a 256 --check --ignore-missing SHA256SUMS
```

## Never

- Push to `main` or force-push it, or rewrite its history.
- Get a check to pass by going around it: no `--no-verify`, no `#[allow(...)]`, `#[ignore]` or deleted assertions to
  silence a failure, no edits to workflows or `deny.toml` that drop a failing check. If a check is wrong, say so in
  the pull request.
- Commit secrets, tokens, `.env` files, downloaded models or sample packs, or anything from `target/` and `out/`.
- Change the version in `Cargo.toml`, the released sections of `CHANGELOG.md`, or `v*` tags by hand. Versions
  change only through the release PR that `release-pr.yml` opens.
- Start a release or merge a release PR without being asked to.
<!-- ANCHOR_END: rules -->

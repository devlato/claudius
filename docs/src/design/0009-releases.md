# 0009 Releases through pull requests

Status: accepted

## Context

claudius is published as binaries for several platforms, with a web studio inside, a changelog, an install script, a
Homebrew formula and a website. Every release has to bump the version in every manifest and `Cargo.lock`, write the
changelog, tag the right commit, build five targets, publish checksums and provenance, and update the formula and the
site. Done by hand, that is a checklist of a dozen steps where any skipped one ships something wrong. Contributors
should never have to touch versions, and the people using claudius should be able to verify what they download.

## Decision

A release is a pull request, and merging it is what releases:

- **Pull request titles are Conventional Commits** (`feat(script): add euclid() to patterns`), checked by
  `pr-title.yml`. Pull requests are squash-merged with their title as the commit message, so `main`'s history is one
  line per change.
- **Starting a release** is running the `release-pr.yml` workflow with a bump (`patch`, `minor`, `major` or a version).
  It runs `cargo xtask release prepare`, which bumps the workspace version, the path dependencies and `Cargo.lock`,
  turns the commits since the last tag into a changelog section with git-cliff (adding anything written under
  "Unreleased" by hand), and opens the pull request `release: vX.Y.Z` from `release/vX.Y.Z`. The notes can be edited on
  that branch before merging.
- **Merging** pushes the new version to `main`, which runs `release.yml`: it checks that the version is unreleased and
  has a changelog section, builds the web studio and the archives for each target, checks and collects their SHA-256
  sums, attests their build provenance, creates the GitHub release as a draft and publishes it (which creates the tag
  on the merge commit), then updates the Homebrew tap and deploys the site. Run again by hand, it retries a release that
  failed halfway, replacing the draft.

The details are in [Releasing](../contributing/releasing.md).

Rejected: tagging releases by hand, which is where versions and tags drift apart; releasing on every merge to `main`,
which would ship half-finished work and make the changelog noise; a third-party release bot, which would put a
dependency with write access in the most sensitive workflow when a few hundred lines of `xtask` and two workflows do
the job and can be read.

## Consequences

- Every release is reviewed like any other change, with its notes in front of the reviewer, and CI runs on it.
- Nobody bumps versions by hand, and the changelog writes itself from titles people already had to write.
- Archives can be verified: `SHA256SUMS` for integrity and `gh attestation verify` for where they were built. The install
  scripts check the sums.
- Pull request titles carry weight: a vague title becomes a vague changelog line. The title check stops malformed ones,
  not uninformative ones.
- Squash merging loses the commits inside a pull request from `main`'s history.
- A pull request opened with the workflow's own token starts no workflows, so the release workflow starts CI on the
  release branch itself, or opens the pull request with a `RELEASE_TOKEN` when one is configured.
- Two targets are experimental: Intel macOS (no ONNX Runtime build exists for it) and Windows (not verified yet). Their
  failures leave them out of a release instead of stopping it.
- Fixes go into the next release only; there are no maintenance branches for older versions.

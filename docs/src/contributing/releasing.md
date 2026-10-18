# Releasing

A release is made by merging a pull request. Nobody bumps a version or pushes a tag by hand: one workflow opens the
release PR, and merging it builds the archives, publishes the GitHub release, updates Homebrew and deploys the
website. A person can do all of it in the GitHub web interface, and an agent with `gh` (see
[Working with AI agents](agents.md)).

## Versions

claudius follows [semantic versioning](https://semver.org). The version lives in one place, `version` under
`[workspace.package]` in the root `Cargo.toml`, and every crate that says `version.workspace = true` has it.
(earshot keeps a version of its own, and `xtask` is never released.)

Before 1.0, the minor version is the one that may break things:

| bump | from 0.4.2 | for |
|---|---|---|
| `patch` | 0.4.3 | fixes, and changes that leave scripts, saved songs, the HTTP API and the MCP tools working as they did |
| `minor` | 0.5.0 | new features, and anything that changes the script language, the song format, the API or the tools (the changelog says how) |
| `major` | 1.0.0 | the promise that none of that breaks again without a major version; a decision, not a routine bump |

An exact version works too: `0.5.0-rc.1` makes a pre-release, which GitHub does not mark as the latest, so the
install scripts keep installing the last full release (`CLAUDIUS_VERSION=0.5.0-rc.1` installs the pre-release).
Homebrew and the website stay on the last full release as well.

## Commit messages are the changelog

Pull requests are squash-merged, and the PR title becomes the commit message on `main`, so the title has to be a
[Conventional Commit](https://www.conventionalcommits.org): `type(scope): what changed`, with `!` after the type or
scope for a breaking change. The `pr title` check enforces it. When a release is prepared, git-cliff (`cliff.toml`)
sorts the commits since the last tag into the sections of `CHANGELOG.md`:

| title | section |
|---|---|
| `feat(script): add euclid() to patterns` | Added |
| `fix(server): keep the session when a render fails` | Fixed |
| `fix(security): ...` | Security |
| `perf(render): mix the stems in parallel` | Changed |
| `docs: a chapter on swing` | Documentation |
| `refactor`, `test`, `build`, `ci`, `chore` | left out, unless breaking (`refactor!: ...`), which goes to Changed |

Breaking changes are marked **Breaking:** in their section, and `(#123)` at the end of a title becomes a link to the
pull request.

Anything worth more than a line (what a release is about, how to update scripts that break) goes under
`## [Unreleased]` in `CHANGELOG.md`, in the pull request that makes the change. The release keeps it at the top of
its section and adds the generated entries under the headings of the same name.

## Making a release

1. **Start it.** From the command line:

   ```sh
   gh workflow run release-pr.yml -f bump=minor
   ```

   or in the web interface: Actions, *release pr*, *Run workflow*. The workflow runs
   `cargo xtask release prepare minor`, which changes three things: the version in `Cargo.toml` (and the
   `version = "..."` of path dependencies between workspace crates, which `cargo add --path` writes), the workspace
   entries in `Cargo.lock`, and `CHANGELOG.md`, where Unreleased becomes `## [0.5.0] - <today>` with the notes from
   git-cliff. It commits that to the branch `release/v0.5.0` through GitHub's API, so that GitHub signs the commit
   (`main` accepts only signed commits), and opens the PR **release: v0.5.0**, with the notes in its description.

2. **Let CI run.** GitHub starts no workflows for a pull request opened with a workflow's own `GITHUB_TOKEN`, so the
   workflow starts CI on the release branch itself (`gh workflow run ci.yml --ref release/v0.5.0`); the checks show
   on the PR as usual. If they are missing, run that command yourself. With the optional `RELEASE_TOKEN` secret (see
   below) the PR is opened with that token instead and starts its checks like any other.

3. **Read the notes, then merge.** To reword them, edit `CHANGELOG.md` on the release branch. Merge the PR once
   *ci ok* passes.

4. **The release builds itself.** The merge changes `Cargo.toml` on `main`, which starts `release.yml`:

   - *plan* decides whether this commit is a release: it is on `main` and is the merge of the release PR,
     `CHANGELOG.md` has a section for the version in `Cargo.toml`, and there is no published release for it yet;
   - *web studio* builds the WebAssembly studio once, for every archive;
   - *build* runs `cargo xtask dist --target <target>` on one runner per target (below), on Linux
     `cargo xtask dist --target <target> --headless` and `scripts/package/linux.sh` too, which makes the `.deb`,
     `.rpm` and AppImage, and on Windows `scripts/package/windows.ps1`, which makes the installer with WiX;
   - *macOS app* puts the two Mac builds together into claudius.app, a universal app, and its disk image, with
     `scripts/package/macos.sh`, signing and notarizing them when the Apple secrets below are set;
   - *release notes* writes the notes: `cargo xtask release notes 0.5.0` and install instructions;
   - *publish* checks that each build uploaded only its own archives and that they match their `.sha256`, writes
     `SHA256SUMS` for the archives and the install scripts, signs a build provenance attestation for them, creates
     the release as a draft, uploads the files, and publishes it, which creates the tag `v0.5.0` on the merge commit;
   - *homebrew tap* updates `Formula/claudius.rb` in `devlato/homebrew-tap`;
   - *site* deploys the website with the new version, changelog and docs.

   No job in `release.yml` restores a cache: everything a release links is built or downloaded (and checked) in
   that run, so nothing another workflow wrote into the Actions cache can end up in an archive. *publish* and
   *homebrew tap*, which hold the tokens, run no cargo at all, and run in the environment `release`, which only
   `main` and the hotfix branches may use. When `RELEASE_REVIEWERS` was given to `scripts/github-setup.sh`, the run
   waits at *publish* until one of them approves it (*Review deployments* on the run's page).

5. **Check it.** [Verifying a release](#verifying-a-release) below.

To see what `release prepare` would do without opening a PR, run it locally and look at the diff, then throw it
away: `cargo xtask release prepare minor && git diff`, then `git checkout -- .`.

## What a release contains

| file | what |
|---|---|
| `claudius-0.5.0-<target>.tar.gz` (`.zip` on Windows) | the programs, the web studio and the docs that go with them |
| `claudius-0.5.0-<target>-headless.tar.gz` (Linux) | the same without the studio window, for servers |
| `claudius-0.5.0-universal-apple-darwin.dmg` | claudius.app, one app for Apple silicon and Intel Macs, to drag to Applications |
| `claudius-0.5.0-x86_64-pc-windows-msvc.msi` | the Windows installer: the programs on the user's `PATH` and the studio in the Start menu |
| `claudius-0.5.0-<target>.deb`, `.rpm`, `.AppImage` (Linux) | packages for Debian and Ubuntu, Fedora and its relatives, and any distribution, with the studio in the app menu |
| `install.sh`, `install.ps1` | the install scripts, which `releases/latest/download/install.sh` points at |
| `SHA256SUMS` | the SHA-256 of every archive and both install scripts; the install scripts check downloads against it |

Each archive unpacks into a directory of the same name, laid out the way `claudius` looks for its files:

```text
claudius-0.5.0-aarch64-apple-darwin/
  bin/claudius
  bin/claudius-packs
  share/claudius/web/        the web studio; claudius finds it at <its own directory>/../share/claudius/web
  README.md  LICENSE  CHANGELOG.md
```

| target | built on | archives |
|---|---|---|
| `aarch64-apple-darwin` | `macos-latest` | the studio and the ears |
| `x86_64-apple-darwin` | `macos-latest`, cross-compiled | no ears: ort has no ONNX Runtime for Intel Macs |
| `x86_64-unknown-linux-gnu` | `ubuntu-24.04` | the studio and the ears, and a `-headless` one; glibc 2.38 or newer |
| `aarch64-unknown-linux-gnu` | `ubuntu-24.04-arm` | the same |
| `x86_64-pc-windows-msvc` | `windows-latest` | experimental, no ears |

Without the ears, verdicts use meters and music theory. `xtask/src/dist.rs` says which targets leave them out
(`NO_EARS`): ort has no ONNX Runtime for Intel Macs, and its Windows ONNX Runtime always links DirectML and
DirectX 12, which nobody has run yet. The `-headless` Linux archives, `claudius-0.5.0-<target>-headless.tar.gz`,
leave out the studio window, so they need neither ALSA nor a display; they still contain the web studio, which
`claudius serve` serves. `install.sh` picks the headless archive when ALSA's `libasound.so.2` is missing, or with
`--headless`.

The Windows build is experimental: when it fails, the release goes out without it, and the install script tells
people on Windows that there is no build for them. The Linux builds cannot target an older glibc: the ONNX Runtime
library they link was itself built against glibc 2.38.

The archives do not include ffmpeg, the sample packs or the listening models. `claudius setup` downloads the packs
and models; ffmpeg is optional.

## Verifying a release

```sh
gh release view v0.5.0
gh release download v0.5.0 --pattern 'claudius-0.5.0-aarch64-apple-darwin.tar.gz' --pattern SHA256SUMS
shasum -a 256 --check --ignore-missing SHA256SUMS
gh attestation verify claudius-0.5.0-aarch64-apple-darwin.tar.gz --repo devlato/claudius \
  --signer-workflow devlato/claudius/.github/workflows/release.yml --source-ref refs/heads/main
```

The checksum says the file is the one that was uploaded. The attestation says more: that the archive was built by
`release.yml` in `devlato/claudius`, from a specific commit on `main`, on GitHub's runners. `gh attestation verify`
checks its Sigstore signature and prints the workflow and the commit. Without `--signer-workflow` and
`--source-ref` it would accept a build by any workflow on any branch of the repository. A hotfix release is built
from its branch, `--source-ref refs/heads/hotfix/v0.5.1`; the release notes give the command with the right one.

The installers come with the release and are in its checksums, so `curl ... releases/latest/download/install.sh`
now installs the new version:

```sh
gh release download v0.5.0 --pattern install.sh --pattern SHA256SUMS --clobber
shasum -a 256 --check --ignore-missing SHA256SUMS
```

## The website

`site.yml` builds the site with `cargo xtask site`, without a cache or secrets, and hands it to a deploy job that
does nothing but upload it to a Cloudflare Pages project, in the environment `site`. It deploys once the site has an
address: the repository variable `SITE_URL` (`gh variable set SITE_URL --body https://claudius.example`) and the
project's name in `SITE_PROJECT`, with the Cloudflare secrets below. Until then it builds and checks the site and
deploys nothing. It deploys `main`:

- after a release, called by `release.yml`;
- on pushes to `main` that change `docs/`, `site/`, `CHANGELOG.md` or the install scripts, so the docs follow
  `main` (except the merge of a release PR, which waits for the release, so that the site never shows a version
  that cannot be downloaded yet);
- by hand: `gh workflow run site.yml`.

Pull requests build the site and deploy nothing: a preview would run the Cloudflare token next to a branch nobody
has reviewed yet.

`cargo xtask site` fills in `{{version}}` (from `Cargo.toml`), `{{site_url}}` and `{{repo_url}}` in the files of
`site/`. The install commands on the site point at the release's own `install.sh` and `install.ps1`, so they work
wherever the site is served.

## Secrets and settings

| secret | environment | used by | without it |
|---|---|---|---|
| `CLOUDFLARE_API_TOKEN` | `site` | `site.yml` | the site is built but not deployed (a notice says so) |
| `CLOUDFLARE_ACCOUNT_ID` | `site` | `site.yml` | the same |
| `HOMEBREW_TAP_TOKEN` | `release` | `release.yml` | the Homebrew formula is not updated |
| `RELEASE_TOKEN` (optional) | `release-pr` | `release-pr.yml` | the workflow opens the release PR with its own token and starts CI itself |
| `MACOS_CERTIFICATE`, `MACOS_CERTIFICATE_PASSWORD` | `release` | `release.yml` | the Mac app is signed ad hoc, and macOS asks before opening it |
| `MACOS_NOTARY_KEY`, `MACOS_NOTARY_KEY_ID`, `MACOS_NOTARY_ISSUER` | `release` | `release.yml` | the Mac app is signed but not notarized, so macOS still asks |

Each secret lives in an environment, not in the repository, so it reaches only the job that needs it, and only when
that job runs on `main` (or, for `release`, a hotfix branch): a workflow pushed to some other branch cannot read it.
Set each one with `gh secret set <NAME> --repo devlato/claudius --env <environment>`, which reads the value from
standard input.

**Cloudflare.** In the Cloudflare dashboard, *My Profile*, *API Tokens*, *Create Token*: a custom token with the
permission *Account*, *Cloudflare Pages*, *Edit*, for your account only, with an expiry date. The account ID is on the *Workers & Pages*
overview. The Pages project has to exist before the first deploy. Its name is also its address,
`<name>.pages.dev`, and has to be free, so pick it first, then tell the workflows about it:

```sh
CLOUDFLARE_API_TOKEN=... npx wrangler pages project create <name> --production-branch main
gh variable set SITE_PROJECT --body <name>
gh variable set SITE_URL --body https://<name>.pages.dev # or the custom domain you add to the project
```

**Apple.** Both need a membership of the [Apple Developer Program](https://developer.apple.com/programs/). In
Xcode, *Settings*, *Accounts*, *Manage Certificates*, add a **Developer ID Application** certificate, then export it
from Keychain Access as a `.p12` file with a password: `MACOS_CERTIFICATE` is the file in base64
(`base64 -i developer-id.p12 | gh secret set MACOS_CERTIFICATE --repo devlato/claudius --env release`), and
`MACOS_CERTIFICATE_PASSWORD` its password. For notarization, in [App Store Connect](https://appstoreconnect.apple.com),
*Users and Access*, *Integrations*, *Team Keys*, make a key with the *Developer* role: `MACOS_NOTARY_KEY` is the text
of the `.p8` file it downloads, `MACOS_NOTARY_KEY_ID` its key ID, and `MACOS_NOTARY_ISSUER` the issuer ID above the
list. With all five, the *macOS app* job signs the app and its disk image with the hardened runtime, has Apple
notarize both, and staples the tickets to them, so the app opens without a warning.

**Homebrew.** Create the repository `devlato/homebrew-tap` (a README is enough). Then a fine-grained personal
access token (GitHub, *Settings*, *Developer settings*, *Fine-grained tokens*) for that repository only, with
*Contents: Read and write*.

**Release PRs.** `RELEASE_TOKEN` is a fine-grained token for `devlato/claudius` with *Contents* and *Pull requests:
Read and write*. A GitHub App token works as well.

Fine-grained tokens expire, after a year at most. When one has, the job that uses it fails with a 401; make a new
token and set the secret again.

The repository settings that the release depends on (squash merges with the PR title, the ruleset that requires *ci
ok* on `main`, the ones that keep `v*` tags from being moved or deleted and let only admins push `hotfix/*`
branches, immutable releases, the three environments, and permission for workflows to open pull requests, which
`release-pr.yml` needs without `RELEASE_TOKEN`) are made by `scripts/github-setup.sh`, once the repository exists,
and again once it is public. Running it again is harmless. Two of its variables change how releases work:

- `RELEASE_REVIEWERS=devlato` makes every release wait for that person's approval before *publish* runs. Without it,
  merging a release PR releases.
- `REVIEWS=1` requires an approving review on every pull request. It also stops workflows from opening (and so from
  approving) pull requests, so `release-pr.yml` then needs `RELEASE_TOKEN`.

An approval only means a person looked if the person is the only one holding their token. An agent that works with
the maintainer's own `gh` login can approve as the maintainer; give agents a token or an account of their own.

## Homebrew

```sh
brew install devlato/tap/claudius
```

The formula is rendered from `scripts/homebrew/claudius.rb.tmpl`: the release workflow fills in the version and the
checksums from `SHA256SUMS` and pushes `Formula/claudius.rb` to the tap. It installs the archive with the studio
window on macOS (Apple silicon and Intel) and Linux; on Intel Macs its caveats say that the build has no ears.

## When a release fails

**A build failed.** Nothing was published and no tag exists. Fix the cause in a normal pull request, then start
the release again by hand: `gh workflow run release.yml`. It builds `main` as it is then, so the release includes
the fix. (The workflow starts by itself only for changes to `Cargo.toml` or `CHANGELOG.md`.)

**Publishing failed halfway.** A draft release may be left over; the next run of `release.yml` deletes it and
publishes again.

**Only Windows failed.** It is experimental, so the release is out without its archive. Fix it, and it is in the
next release.

**The Homebrew or site job failed.** The release itself is out. Rerun the failed jobs:
`gh run rerun <run id> --failed`.

## Rolling back

A published release stays published: people have downloaded it, and the formula points at it. Do not delete it or
move its tag. To stop handing it out while a fix is on its way:

```sh
gh release edit v0.5.0 --prerelease     # the install scripts stop picking it
gh release edit v0.4.3 --latest         # releases/latest points at the previous one again
```

then revert the formula commit in `devlato/homebrew-tap`, and roll the site back in the Cloudflare dashboard
(*Workers & Pages*, the project, *Deployments*, *Rollback* on the deployment before). The fix itself goes out as the
next patch release.

## Hotfix releases

A fix usually ships with a patch release from `main`. When `main` already has changes that should not go out yet,
release the fix from the last tag instead:

```sh
git fetch --tags
git switch -c hotfix/v0.5.1 v0.5.0
git cherry-pick <the fix's commit on main>
cargo xtask release prepare patch
git add --update && git commit -m "release: v0.5.1"
git push -u origin hotfix/v0.5.1
gh workflow run ci.yml --ref hotfix/v0.5.1          # wait for "ci ok"
gh workflow run release.yml --ref hotfix/v0.5.1
```

Only admins can create and push `hotfix/*` branches (a ruleset), and `release.yml` releases from no other branch
but `main`. The release is built and tagged on the hotfix branch; the site is only deployed from `main`. Then bring the
version and the changelog back to `main`: open a pull request that cherry-picks the release commit, keeping
0.5.1's section under Unreleased.

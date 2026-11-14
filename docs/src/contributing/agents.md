# Working with AI agents

AI coding agents work on claudius the way people do: on a branch, through a pull request with a Conventional
Commit title, with `cargo xtask ci` run before every push. Everything after that runs on GitHub and is driven with
`gh`, so an agent can take a change from a branch to a published release, and a person can follow every step in
the web interface.

Agents read `AGENTS.md` at the root of the repository. This page shows the same text, so the rules for people and
for agents cannot drift apart. [Releasing](releasing.md) explains what each release step does.

{{#include ../../../AGENTS.md:rules}}

## Claude Code

Claude Code reads `CLAUDE.md`, which loads `AGENTS.md` and adds what is particular to Claude Code.
`.claude/settings.json` has it run `.claude/hooks/guard.sh` before each command and each edit, to hold it to the
rules above that a script can check:

- An edit to a file git tracks needs a branch named `<type>/<topic>`: not `main`, and not a detached commit unless a
  rebase, merge or cherry-pick is under way.
- `--no-verify` is refused, and so is pushing a `v*` tag.
- `git push` and `gh pr create` wait until `cargo xtask ci` has passed on the commit, with nothing left uncommitted.

The checks run through `.claude/hooks/ci.sh`, which you can also start yourself. It runs `cargo xtask ci` the way
GitHub's runners see a machine: with `CLAUDIUS_PACKS` pointing at nothing and a home of its own, so the packs and
models on your computer do not change what the tests do. It writes its log to `target/claude-ci.log` and remembers
the tree it passed on, so a push of that tree goes out without a second run.

Settings of your own go in `.claude/settings.local.json`, and notes for your own sessions in `CLAUDE.local.md`; git
leaves both out. `/effort max` lasts only for the session it is typed in; with
`{"env": {"CLAUDE_CODE_EFFORT_LEVEL": "max"}}` in `.claude/settings.local.json`, every session in the repository
starts at the highest effort, at the cost of more tokens.

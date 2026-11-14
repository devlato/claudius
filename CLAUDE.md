# Claude Code in claudius

@AGENTS.md

The line above loads `AGENTS.md`, the rules for every agent and person working here. This file adds what is
particular to Claude Code; [Working with AI agents](docs/src/contributing/agents.md) explains the setup for people.

## The hooks

`.claude/settings.json` runs `.claude/hooks/guard.sh` before each command and each edit. It holds you to the rules
of `AGENTS.md` that a script can check:

- A file git tracks changes only on a branch named `<type>/<topic>`: not on `main`, and not on a detached commit
  unless a rebase, merge or cherry-pick is under way.
- `--no-verify` is refused, and so is pushing a `v*` tag.
- `git push` and `gh pr create` go out only once `cargo xtask ci` has passed on the commit, with nothing left
  uncommitted.

The checks run through `.claude/hooks/ci.sh`, the way GitHub's runners would: without the sample packs and models
installed on this computer, in a home of their own. They take minutes, so start `.claude/hooks/ci.sh` in the
background once the last commit is made and carry on with something else; when it has passed, the push goes out at
once. A refusal says what to fix. If the hook is wrong, say so instead of working around it.

## Keeping what you are told

- Write a standing instruction down when it is given, one that outlasts the task at hand: in `CLAUDE.local.md` when
  it is about how the person you work for likes things done (the file is theirs, and git leaves it out), and in
  `AGENTS.md` or the contributor guide, through a pull request, when it is a rule for everyone.
- Long work keeps its plan and its progress in a file under `target/`, which git leaves out, so that they survive
  the conversation being compacted. Read that file again after a compaction.

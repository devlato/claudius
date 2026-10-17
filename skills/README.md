# Skills for AI assistants

These folders teach an assistant how to make music with claudius: what to do first, how to read the critic and act on
it, how to write the script language, and how to talk about the changes. They are [Agent
Skills](https://agentskills.io): a `SKILL.md` with a short description the assistant reads up front, and reference files
it opens when it needs them.

| skill | for |
|---|---|
| [claudius-songs](claudius-songs/SKILL.md) | making and editing songs: the loop of listening, reading the verdict and changing one thing; the script language; the eight genres |
| [claudius-remix](claudius-remix/SKILL.md) | working from a recording: analysing it, remaking its skeleton in another genre, chopping it and flipping it into a song |
| [claudius-setup](claudius-setup/SKILL.md) | installing claudius, connecting an assistant, and fixing what goes wrong |

The MCP tools give an assistant the controls; the skills give it the craft. They work with the tools and with the
`claudius` command alike.

## Installing them

`claudius connect` installs them along with the MCP server, and `--remove` takes both out again:

| assistant | command | where the skills go |
|---|---|---|
| Claude Code | `claudius connect claude-code` | `~/.claude/skills/` |
| Cursor | `claudius connect cursor` | `~/.cursor/skills/` |
| Codex | `claudius connect codex` | `~/.codex/skills/` |
| Claude Desktop and claude.ai | `claudius connect claude-desktop` | zips in `~/Music/claudius/skills/`, to upload in Claude's settings under Capabilities |

`claudius skills --out <folder>` writes them anywhere, as folders and as zips: for a project's own `.claude/skills/`
or `.cursor/skills/`, or for another assistant that reads skills. An assistant that doesn't read skills can still be
given `claudius-songs/SKILL.md` as instructions, for example in a ChatGPT project.

The skills are built into the `claudius` binary, so the ones it installs always match the claudius that runs them.

## Keeping them right

Every `js` block in these files is run by `cargo test -p claudius --test docs_examples`, like the examples in the
book, so a skill cannot teach a function that no longer exists. Plain `js` blocks run without the sample packs; mark
a block `js packs` when it needs them, and `js no-run` for a fragment. `skills::FILES` in
`crates/claudius-cli/src/skills.rs` lists every file a skill has, and a test fails when a file is added here but not
there.

The tool names, commands and messages they quote come from the code: when one of those changes, change the skill in
the same pull request.

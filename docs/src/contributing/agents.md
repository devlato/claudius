# Working with AI agents

AI coding agents work on claudius the way people do: on a branch, through a pull request with a Conventional
Commit title, with `cargo xtask ci` run before every push. Everything after that runs on GitHub and is driven with
`gh`, so an agent can take a change from a branch to a published release, and a person can follow every step in
the web interface.

Agents read `AGENTS.md` at the root of the repository. This page shows the same text, so the rules for people and
for agents cannot drift apart. [Releasing](releasing.md) explains what each release step does.

{{#include ../../../AGENTS.md:rules}}

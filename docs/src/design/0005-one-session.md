# 0005 One session, many clients

Status: accepted

## Context

The studio exists as a native window and as a web page, and assistants edit songs over MCP. The experience claudius
aims for is a person listening in the studio while an assistant works, hearing every change and stepping in. That means
several clients edit one song at the same time, and each must see the others' changes quickly.

The options were to give each client its own engine and synchronise them, to merge concurrent edits with a CRDT or
operational transforms, or to keep exactly one song and make every client a view of it.

## Decision

A process holds one `App` (`crates/claudius-server`) with one `Session`: the song, its undo and redo stacks, the latest
render and verdict, a log, and at most one running job. Every client goes through it:

- the studio, native and web, uses the HTTP API only, long-polling `GET /api/state?since=N` and sending commands;
- MCP over HTTP calls the session in process;
- `claudius mcp`, started by a desktop assistant, first looks for a claudius already running on the port and drives it
  through its HTTP API; only if there is none does it start a session of its own.

Edits are serialised by a mutex, and each runs on a copy of the current blueprint and replaces it on success. Every
change bumps a version that long polls wait on. Jobs (renders, mutation, evolution, saves) run one at a time, and a
render or a save asked for during a job runs after it.

Rejected: an engine per client, which would leave the person and the assistant on different songs the moment one of
them edits. Merging concurrent edits, which suits documents edited by many people offline and is far more machinery
than one person and one assistant need, when serialising edits gives a clear "last edit wins, nothing is lost to undo".
WebSockets for updates, which would add a protocol for what a long poll does, and long polls pass through tunnels and
proxies without configuration.

## Consequences

- There is no synchronisation protocol and no conflict to resolve: whoever edits second edits the result of the first.
  Undo is shared too, so you can take back an assistant's change from the studio with Cmd+Z.
- The studio is an ordinary HTTP client, which keeps the API complete: anything the studio can do, `curl`, scripts and
  assistants can do.
- One process holds one song. Working on two songs means two processes on two ports.
- One job at a time: while an evolution runs, a render or a save waits for it, and a second mutation is refused with
  `busy`.
- Optimistic edits in the studio can be refused by the server, and then flash back to the server's state.
- A client holds a connection open per long poll, up to 20 s (or the timeout it asks for), which is cheap for the
  handful of clients a session has and would not scale to many.
- When `claudius mcp` finds nothing on its port (the studio runs on another port, or was started after the
  assistant), it starts its own session without failing, and the assistant edits a different song from the one on
  screen. stderr says which way it went, but people rarely look there. A claudius that asks for a token the assistant
  was not given is recognised, and `claudius mcp` fails with a message instead of starting a second session.

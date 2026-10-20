// claudius' usage statistics: the events claudius sends after a person turns them on (crates/claudius-cli/src/
// telemetry.rs). Each field is checked against what claudius can send, and the event is written to Workers Analytics
// Engine with those fields only: no IP address, no identifier, nothing else from the request. Analytics Engine keeps
// data for three months. docs/src/guide/privacy.md describes it for the people who send them.

const COMMANDS = new Set([
  "make", "evolve", "mutate", "analyze", "remake", "chop", "render", "export", "judge", "genres", "sounds", "script",
  "reference", "setup", "doctor", "connect", "skills", "studio", "serve", "mcp", "update",
]);
const VERSION = /^\d{1,3}\.\d{1,3}\.\d{1,3}(-[0-9A-Za-z.]{1,20})?$/;
const OS = new Set(["macos", "linux", "windows"]);
const ARCH = new Set(["aarch64", "x86_64"]);
const DETAIL = {
  started: (d) => d === "",
  setup: (d) => /^(ok|models|packs:[a-z0-9-]{1,40})$/.test(d),
  panic: (d) => /^[A-Za-z0-9_.\-]{1,60}(\/[A-Za-z0-9_.\-]{1,60}){0,2}:\d{1,6}$/.test(d),
};

function valid(e) {
  return (
    e !== null &&
    typeof e === "object" &&
    Object.keys(e).length === 7 &&
    e.v === 1 &&
    Object.hasOwn(DETAIL, e.event) &&
    typeof e.command === "string" &&
    COMMANDS.has(e.command) &&
    typeof e.version === "string" &&
    VERSION.test(e.version) &&
    OS.has(e.os) &&
    ARCH.has(e.arch) &&
    typeof e.detail === "string" &&
    DETAIL[e.event](e.detail)
  );
}

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    if (url.pathname !== "/v1/event") {
      return new Response("claudius usage statistics: see docs/src/guide/privacy.md in github.com/devlato/claudius\n", {
        status: 404,
      });
    }
    if (request.method !== "POST") {
      return new Response("POST an event\n", { status: 405, headers: { allow: "POST" } });
    }
    const text = await request.text();
    if (text.length > 512) {
      return new Response("too large\n", { status: 413 });
    }
    let e;
    try {
      e = JSON.parse(text);
    } catch {
      return new Response("not JSON\n", { status: 400 });
    }
    if (!valid(e)) {
      return new Response("not an event claudius sends\n", { status: 400 });
    }
    if (!env.EVENTS) {
      return new Response("not stored: Analytics Engine is not bound\n", { status: 503 });
    }
    env.EVENTS.writeDataPoint({
      indexes: [e.event],
      blobs: [e.event, e.command, e.version, e.os, e.arch, e.detail],
      doubles: [1],
    });
    return new Response(null, { status: 204 });
  },
};

// People's feedback on claudius: what they write in the studio's Send feedback window (crates/claudius-ui/src/panels/
// feedback.rs), the web demo's, the site's form (site/feedback/), or what an assistant passes on with their OK (the MCP
// tool send_feedback). Each message is checked (what they wrote, 1 to 4000 characters; where to answer them, at most
// 200; claudius' version; the system; where it was sent from; the song's genre) and kept in D1 with those fields and the
// day it came: no IP address, nothing else from the request. Five a minute from one address at most: the limiter sees
// the address and nothing keeps it. docs/src/guide/privacy.md describes it for the people who send it. To read it:
//   npx wrangler d1 execute claudius-feedback --remote --command "select * from feedback order by id desc limit 20"

const PLACES = new Set(["studio", "demo", "site", "assistant"]);
const OS = new Set(["macos", "linux", "windows", "web"]);
const VERSION = /^\d{1,3}\.\d{1,3}\.\d{1,3}(-[0-9A-Za-z.]{1,20})?$/;
const GENRE = /^[a-z0-9_]{1,40}(\+[a-z0-9_]{1,40})*$/;
const FIELDS = new Set(["v", "text", "contact", "version", "os", "where", "song"]);
/** the pages that may send it from a browser */
const ORIGINS = new Set(["https://claudius.devlato.com"]);

/** whether `m` is a message claudius sends: its text, and nothing it does not know */
function valid(m) {
  if (m === null || typeof m !== "object" || Array.isArray(m) || m.v !== 1) return false;
  if (!Object.keys(m).every((k) => FIELDS.has(k))) return false;
  if (typeof m.text !== "string" || m.text.trim().length < 1 || m.text.length > 4000) return false;
  if (!PLACES.has(m.where)) return false;
  if (m.contact !== undefined && (typeof m.contact !== "string" || m.contact.length > 200)) return false;
  if (m.version !== undefined && (typeof m.version !== "string" || !VERSION.test(m.version))) return false;
  if (m.os !== undefined && !OS.has(m.os)) return false;
  if (m.song !== undefined && (typeof m.song !== "string" || m.song.length > 200 || !GENRE.test(m.song))) return false;
  return true;
}

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    const origin = request.headers.get("origin");
    const cors = ORIGINS.has(origin) ? { "access-control-allow-origin": origin, vary: "origin" } : {};
    if (request.method === "OPTIONS") {
      const allow = { "access-control-allow-methods": "POST", "access-control-allow-headers": "content-type" };
      return new Response(null, { status: 204, headers: { ...cors, ...allow, "access-control-max-age": "86400" } });
    }
    if (url.pathname !== "/v1/feedback") {
      return new Response("claudius feedback: see docs/src/guide/privacy.md in github.com/devlato/claudius\n", { status: 404 });
    }
    if (request.method !== "POST") {
      return new Response("POST feedback\n", { status: 405, headers: { ...cors, allow: "POST" } });
    }
    if (env.LIMIT) {
      const { success } = await env.LIMIT.limit({ key: request.headers.get("cf-connecting-ip") || "unknown" });
      if (!success) return new Response("too many at once: try again in a minute\n", { status: 429, headers: cors });
    }
    const text = await request.text();
    if (text.length > 6000) {
      return new Response("too large\n", { status: 413, headers: cors });
    }
    let m;
    try {
      m = JSON.parse(text);
    } catch {
      return new Response("not JSON\n", { status: 400, headers: cors });
    }
    if (!valid(m)) {
      return new Response("not a message claudius sends\n", { status: 400, headers: cors });
    }
    if (!env.FEEDBACK) {
      return new Response("not kept: the database is not bound\n", { status: 503, headers: cors });
    }
    const day = new Date().toISOString().slice(0, 10);
    const row = [day, m.text.trim(), m.contact?.trim() || null, m.version || null, m.os || null, m.where, m.song || null];
    await env.FEEDBACK.prepare(
      "insert into feedback (day, text, contact, version, os, place, song) values (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )
      .bind(...row)
      .run();
    return new Response(null, { status: 204, headers: cors });
  },
};

// The site's code on the server: the studio demo's samples, and the relay between the demo and a phone. A song built
// on samples asks for them as the demo makes it, and the demo asks this site and no one else: this Worker searches
// Openverse for recordings on Freesound, Jamendo and Wikimedia Commons under CC0 or CC BY and serves them, caching
// both at Cloudflare's edge. It passes on nothing about the visitor: the libraries see this site ask, with the demo's
// words. A phone playing along with the demo reaches it through a room of the relay (room.js), which only the site's
// own pages may join. Everything else the site serves is static (`run_worker_first` in wrangler.toml sends only these
// paths here).
//
//   GET /studio/samples/search?q=disco%20strings   up to 20 recordings, as the demo reads them
//   GET /studio/samples/audio/<folder>/<sound>_<user>.ogg   a preview, as Freesound serves it
//   GET /studio/samples/audio/jamendo/<track>.mp3   a song, as Jamendo serves it
//   GET /studio/samples/audio/commons/<a>/<ab>/<file>   a recording, as Wikimedia Commons serves it
//   GET /relay/<room>?as=host|phone   a WebSocket into the room (room.js)
//
// A search is a few words of letters and digits: the kinds of record a genre digs for (its profile's `sampling`), or a
// person's own. A visitor may ask for 30 a minute that the edge has not kept (SEARCHES in wrangler.toml), so that
// Openverse, which limits how often this site asks it, is asked for songs.

export { Room } from "./room.js";

/** a search's words, once read: a letter or digit, then letters, digits, spaces and & ' . - (60 at most) */
export const WORDS = /^[\p{L}\p{N}][\p{L}\p{N} &'.-]{0,59}$/u;

const OPENVERSE = "https://api.openverse.org/v1/audio/";
const AGENT = "claudius-site/1 (https://claudius.devlato.com)";
const DAY = 86_400;
/// the largest recording served: a song of five minutes is less
const MOST = 12 * 1024 * 1024;

/** where each library keeps a recording, as Openverse names it, and the path the demo asks this site for it by */
const FREESOUND = /^https:\/\/cdn\.freesound\.org\/previews\/(\d{1,6})\/(\d{1,10})_(\d{1,10})-(?:hq|lq)\.(?:mp3|ogg)$/;
const JAMENDO = /^https:\/\/prod-1\.storage\.jamendo\.com\/\?trackid=(\d{1,10})&format=mp3[12]$/;
const COMMONS = /^https:\/\/upload\.wikimedia\.org\/wikipedia\/commons\/([0-9a-f])\/([0-9a-f]{2})\/([A-Za-z0-9%._(),'!~-]{1,200}\.(mp3|ogg|oga))$/;
const AUDIO_FREESOUND = /^\/studio\/samples\/audio\/(\d{1,6})\/(\d{1,10})_(\d{1,10})\.ogg$/;
const AUDIO_JAMENDO = /^\/studio\/samples\/audio\/jamendo\/(\d{1,10})\.mp3$/;
const AUDIO_COMMONS = /^\/studio\/samples\/audio\/commons\/([0-9a-f])\/([0-9a-f]{2})\/([A-Za-z0-9%._(),'!~-]{1,200}\.(mp3|ogg|oga))$/;
const ROOM = /^\/relay\/([0-9a-f]{32})$/;

const plain = (body, status, headers = {}) =>
  new Response(body, {
    status,
    headers: { "content-type": "text/plain; charset=utf-8", "x-content-type-options": "nosniff", ...headers },
  });

/** a search's words as the demo writes them: lower case, single spaces */
export const words = (q) => (q || "").trim().toLowerCase().replace(/\s+/g, " ");

/** what file a library's recording is, and where the demo asks for it; null for one it cannot play */
function where(url) {
  let m = FREESOUND.exec(url);
  if (m) return { id: `freesound:${m[2]}`, audio: `audio/${m[1]}/${m[2]}_${m[3]}.ogg`, ext: "ogg" };
  m = JAMENDO.exec(url);
  if (m) return { id: `jamendo:${m[1]}`, audio: `audio/jamendo/${m[1]}.mp3`, ext: "mp3" };
  m = COMMONS.exec(url);
  if (m && !m[3].includes("..")) {
    let name = m[3];
    try {
      name = decodeURIComponent(m[3]);
    } catch {
      return null;
    }
    return { id: `commons:${name}`, audio: `audio/commons/${m[1]}/${m[2]}/${m[3]}`, ext: m[4] };
  }
  return null;
}

/** an Openverse result as the demo reads it, or null for one it cannot play */
export function trim(x) {
  const at = where(x?.url || "");
  if (!at || !["cc0", "by"].includes(x.license) || Number(x.filesize ?? 0) > MOST) return null;
  return {
    id: at.id,
    provider: String(x.provider ?? x.source ?? ""),
    title: String(x.title ?? ""),
    creator: String(x.creator ?? ""),
    license: x.license,
    license_version: String(x.license_version ?? ""),
    license_url: String(x.license_url ?? ""),
    page: String(x.foreign_landing_url ?? ""),
    audio: at.audio,
    ext: at.ext,
    seconds: Number(x.duration ?? 0) / 1000,
    tags: (x.tags ?? []).map((t) => String(t?.name ?? "")).filter(Boolean).slice(0, 24),
  };
}

async function search(q) {
  const up = new URL(OPENVERSE);
  up.search = new URLSearchParams({
    q,
    license: "cc0,by",
    source: "freesound,jamendo,wikimedia_audio",
    page_size: "20",
    mature: "false",
  }).toString();
  const r = await fetch(up, { headers: { "user-agent": AGENT, accept: "application/json" } });
  if (r.status === 429) return plain("the library is busy: try again in a minute", 503, { "retry-after": "60" });
  if (!r.ok) return plain(`the library answered ${r.status}`, 502);
  const results = ((await r.json()).results ?? []).map(trim).filter(Boolean);
  return new Response(JSON.stringify({ results }), {
    headers: {
      "content-type": "application/json",
      "cache-control": `public, max-age=${DAY}`,
      "x-content-type-options": "nosniff",
    },
  });
}

/** a response's body, read until it passes `MOST` bytes (null then, the rest of it never read) */
export async function bounded(r) {
  const reader = r.body.getReader();
  const chunks = [];
  let size = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    size += value.byteLength;
    if (size > MOST) {
      await reader.cancel();
      return null;
    }
    chunks.push(value);
  }
  const out = new Uint8Array(size);
  let at = 0;
  for (const c of chunks) {
    out.set(c, at);
    at += c.byteLength;
  }
  return out;
}

/** the file at `url`, as a library serves it, with its kind, unless it is larger than the demo takes */
async function audio(url, type) {
  const r = await fetch(url, { headers: { "user-agent": AGENT } });
  if (!r.ok) return plain(`the library answered ${r.status}`, r.status === 404 ? 404 : 502);
  if (Number(r.headers.get("content-length") ?? 0) > MOST) return plain("too long for the demo", 413);
  // a length the library leaves out, or understates, is not taken at its word
  const body = await bounded(r);
  if (!body) return plain("too long for the demo", 413);
  return new Response(body, {
    headers: {
      "content-type": type,
      "cache-control": `public, max-age=${365 * DAY}, immutable`,
      "x-content-type-options": "nosniff",
    },
  });
}

/** the library's address for an audio path the demo asks for, and the kind of file it is; null for no such path */
export function source(path) {
  let m = AUDIO_FREESOUND.exec(path);
  if (m) return [`https://cdn.freesound.org/previews/${m[1]}/${m[2]}_${m[3]}-hq.ogg`, "audio/ogg"];
  m = AUDIO_JAMENDO.exec(path);
  if (m) return [`https://prod-1.storage.jamendo.com/?trackid=${m[1]}&format=mp32`, "audio/mpeg"];
  m = AUDIO_COMMONS.exec(path);
  if (m && !m[3].includes("..")) {
    return [`https://upload.wikimedia.org/wikipedia/commons/${m[1]}/${m[2]}/${m[3]}`, m[4] === "mp3" ? "audio/mpeg" : "audio/ogg"];
  }
  return null;
}

/** what the edge's cache keeps for `key`, if it is there (a Worker run elsewhere, as the tests run it, has none) */
async function kept(key) {
  const cache = typeof caches === "undefined" ? null : caches.default;
  return cache ? await cache.match(key) : undefined;
}

/** what `make` answers, kept in the edge's cache when it went well */
async function keep(key, ctx, make) {
  const res = await make();
  if (typeof caches !== "undefined" && res.ok) ctx?.waitUntil?.(caches.default.put(key, res.clone()));
  return res;
}

/** the name of the room a request joins, or the answer that turns it away */
export function room(request) {
  const url = new URL(request.url);
  const m = ROOM.exec(url.pathname);
  if (!m) return plain("no such room", 404);
  // the demo and the phone's page are the site's own; nothing else joins a room
  if (request.headers.get("origin") !== url.origin) return plain("a room is for the site's own pages", 403);
  return m[1];
}

export default {
  async fetch(request, env, ctx) {
    const url = new URL(request.url);
    if (url.pathname.startsWith("/relay/")) {
      const name = room(request);
      return typeof name === "string" ? env.ROOMS.get(env.ROOMS.idFromName(name)).fetch(request) : name;
    }
    if (!url.pathname.startsWith("/studio/samples/")) return env.ASSETS.fetch(request);
    if (request.method !== "GET") return plain("only GET", 405, { allow: "GET" });
    if (url.pathname === "/studio/samples/search") {
      const q = words(url.searchParams.get("q"));
      if (!WORDS.test(q)) return plain("a search is a few words of letters and digits", 400);
      const key = new Request(`${url.origin}/studio/samples/search?q=${encodeURIComponent(q)}`);
      const hit = await kept(key);
      if (hit) return hit;
      // a search the edge has not kept asks Openverse: a visitor may ask for so many a minute
      const visitor = request.headers.get("cf-connecting-ip") ?? "someone";
      if (env?.SEARCHES && !(await env.SEARCHES.limit({ key: visitor })).success) {
        return plain("too many searches at once: try again in a minute", 429, { "retry-after": "60" });
      }
      return keep(key, ctx, () => search(q));
    }
    const at = source(url.pathname);
    if (!at) return plain("no such sample", 404);
    const key = new Request(`${url.origin}${url.pathname}`);
    return (await kept(key)) ?? keep(key, ctx, () => audio(at[0], at[1]));
  },
};

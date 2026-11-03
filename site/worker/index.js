// The site's one piece of code on the server: the studio demo's samples. A genre built on samples (trip-hop) asks for
// them as the demo makes a song, and the demo asks this site and no one else: this Worker searches Openverse for loops
// on Freesound under CC0 or CC BY and serves their previews, caching both at Cloudflare's edge. It passes on nothing
// about the visitor: Openverse and Freesound see this site ask, with the demo's own words. Everything else the site
// serves is static (`run_worker_first` in wrangler.toml sends only these paths here).
//
//   GET /studio/samples/search?q=dark%20piano%20loop   up to 20 loops, as the demo reads them
//   GET /studio/samples/audio/<folder>/<sound>_<user>.ogg   a preview, as Freesound serves it
//
// The searches are the demo's own (`crates/claudius-ui/src/mock/song.rs`), so that the Worker is no one's proxy.

export const SEARCHES = new Set([
  // trip-hop's phrase, break and texture
  "dark piano loop",
  "sad piano loop",
  "strings loop",
  "cello loop",
  "orchestral strings loop",
  "dark melody loop",
  "orchestral melody",
  "sad violin loop",
  "hip hop drum loop",
  "dusty drum loop",
  "slow drum break",
  "funk drum loop",
  "vinyl crackle",
  "record crackle",
  "vinyl noise",
]);

const OPENVERSE = "https://api.openverse.org/v1/audio/";
const PREVIEWS = "https://cdn.freesound.org/previews/";
const AGENT = "claudius-site/1 (https://claudius.devlato.com)";
const DAY = 86_400;
/// the longest preview served: a loop of half a minute is far less
const MOST = 8 * 1024 * 1024;
const PREVIEW = /^https:\/\/cdn\.freesound\.org\/previews\/(\d{1,6})\/(\d{1,10})_(\d{1,10})-(?:hq|lq)\.(?:mp3|ogg)$/;
const AUDIO = /^\/studio\/samples\/audio\/(\d{1,6})\/(\d{1,10})_(\d{1,10})\.ogg$/;

const plain = (body, status, headers = {}) =>
  new Response(body, {
    status,
    headers: { "content-type": "text/plain; charset=utf-8", "x-content-type-options": "nosniff", ...headers },
  });

/** a search's words as the demo writes them: lower case, single spaces */
export const words = (q) => (q || "").trim().toLowerCase().replace(/\s+/g, " ");

/** an Openverse result as the demo reads it, or null for one it cannot play */
export function trim(x) {
  const m = PREVIEW.exec(x?.url || "");
  if (!m || !["cc0", "by"].includes(x.license)) return null;
  return {
    id: String(x.id ?? ""),
    title: String(x.title ?? ""),
    creator: String(x.creator ?? ""),
    license: x.license,
    license_version: String(x.license_version ?? ""),
    license_url: String(x.license_url ?? ""),
    page: String(x.foreign_landing_url ?? ""),
    audio: `audio/${m[1]}/${m[2]}_${m[3]}.ogg`,
    seconds: Number(x.duration ?? 0) / 1000,
    tags: (x.tags ?? []).map((t) => String(t?.name ?? "")).filter(Boolean).slice(0, 24),
  };
}

async function search(q) {
  const up = new URL(OPENVERSE);
  up.search = new URLSearchParams({
    q,
    license: "cc0,by",
    source: "freesound",
    length: "shortest",
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

async function audio(folder, sound, user) {
  const r = await fetch(`${PREVIEWS}${folder}/${sound}_${user}-hq.ogg`, { headers: { "user-agent": AGENT } });
  if (!r.ok) return plain(`Freesound answered ${r.status}`, r.status === 404 ? 404 : 502);
  if (Number(r.headers.get("content-length") ?? 0) > MOST) return plain("too long for the demo", 413);
  return new Response(r.body, {
    headers: {
      "content-type": "audio/ogg",
      "cache-control": `public, max-age=${365 * DAY}, immutable`,
      "x-content-type-options": "nosniff",
    },
  });
}

/** what `make` answers, from the edge's cache when it has it (a Worker run elsewhere, as the tests run it, has none) */
async function cached(key, ctx, make) {
  const cache = typeof caches === "undefined" ? null : caches.default;
  const hit = cache && (await cache.match(key));
  if (hit) return hit;
  const res = await make();
  if (cache && res.ok) ctx?.waitUntil?.(cache.put(key, res.clone()));
  return res;
}

export default {
  async fetch(request, env, ctx) {
    const url = new URL(request.url);
    if (!url.pathname.startsWith("/studio/samples/")) return env.ASSETS.fetch(request);
    if (request.method !== "GET") return plain("only GET", 405, { allow: "GET" });
    if (url.pathname === "/studio/samples/search") {
      const q = words(url.searchParams.get("q"));
      if (!SEARCHES.has(q)) return plain("the demo searches for its own samples only", 400);
      const key = new Request(`${url.origin}/studio/samples/search?q=${encodeURIComponent(q)}`);
      return cached(key, ctx, () => search(q));
    }
    const m = AUDIO.exec(url.pathname);
    if (m) return cached(new Request(`${url.origin}${url.pathname}`), ctx, () => audio(m[1], m[2], m[3]));
    return plain("no such sample", 404);
  },
};

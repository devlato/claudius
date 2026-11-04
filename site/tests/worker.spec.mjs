// The site's Worker (worker/index.js), run as Cloudflare runs it, with Openverse and Freesound stood in for: it asks
// for the demo's own searches only, with its filters fixed, passes on the loops the demo can play, and serves their
// previews by path and nothing else.
import { test, expect } from "@playwright/test";
import worker from "../worker/index.js";

const ORIGIN = "https://claudius.devlato.com";
const assets = { fetch: async (r) => new Response(`asset ${new URL(r.url).pathname}`) };
const get = (path, init) => worker.fetch(new Request(ORIGIN + path, init), { ASSETS: assets }, { waitUntil() {} });

/** runs `fn` with the network answering from `answer(url)`; returns what was asked of it */
async function upstream(answer, fn) {
  const real = globalThis.fetch;
  const asked = [];
  globalThis.fetch = async (url, init) => {
    asked.push({ url: String(url), headers: new Headers(init?.headers) });
    return answer(String(url));
  };
  try {
    await fn();
  } finally {
    globalThis.fetch = real;
  }
  return asked;
}

const FOUND = {
  id: "5d1c",
  title: "Sad Hip Hop Piano Keys - 75bpm - Amin",
  creator: "nnaudio",
  license: "by",
  license_version: "4.0",
  license_url: "https://creativecommons.org/licenses/by/4.0/",
  foreign_landing_url: "https://freesound.org/people/nnaudio/sounds/123/",
  url: "https://cdn.freesound.org/previews/0/123_456-hq.mp3",
  duration: 25600,
  tags: [{ name: "piano" }, { name: "75bpm" }],
  filesize: 409600,
};

test.describe("the samples Worker", () => {
  // run as a Worker runs off the edge, with no cache: serve.mjs puts one in for the test server, and a spec that loads it
  // (the smoke test does) may have run in this process first, keeping the previews these tests serve
  test.beforeEach(() => {
    delete globalThis.caches;
  });

  test("searches Openverse for the demo's words with its filters, and passes on the loops it can play", async () => {
    let body;
    const asked = await upstream(
      () =>
        Response.json({
          results: [
            FOUND,
            { ...FOUND, id: "nc", license: "by-nc" },
            { ...FOUND, id: "commons", url: "https://upload.wikimedia.org/wikipedia/commons/a/ab/x.ogg" },
          ],
        }),
      async () => {
        const r = await get("/studio/samples/search?q=Dark%20%20Piano%20Loop");
        expect(r.status).toBe(200);
        expect(r.headers.get("cache-control")).toBe("public, max-age=86400");
        body = await r.json();
      },
    );
    expect(asked).toHaveLength(1);
    const up = new URL(asked[0].url);
    expect(up.origin + up.pathname).toBe("https://api.openverse.org/v1/audio/");
    expect(Object.fromEntries(up.searchParams)).toEqual({
      q: "dark piano loop",
      license: "cc0,by",
      source: "freesound",
      length: "shortest",
      page_size: "20",
      mature: "false",
    });
    expect(asked[0].headers.get("user-agent")).toMatch(/^claudius-site\//);
    expect(body.results).toEqual([
      {
        id: "5d1c",
        title: "Sad Hip Hop Piano Keys - 75bpm - Amin",
        creator: "nnaudio",
        license: "by",
        license_version: "4.0",
        license_url: "https://creativecommons.org/licenses/by/4.0/",
        page: "https://freesound.org/people/nnaudio/sounds/123/",
        audio: "audio/0/123_456.ogg",
        seconds: 25.6,
        tags: ["piano", "75bpm"],
      },
    ]);
  });

  test("refuses searches that are not the demo's, and asks no one", async () => {
    const asked = await upstream(
      () => Response.json({ results: [] }),
      async () => {
        for (const q of ["anything at all", "", "vinyl crackle loop", "x".repeat(300)]) {
          expect((await get(`/studio/samples/search?q=${encodeURIComponent(q)}`)).status, q).toBe(400);
        }
      },
    );
    expect(asked).toEqual([]);
  });

  test("says the library is busy when Openverse limits it", async () => {
    await upstream(
      () => new Response("slow down", { status: 429 }),
      async () => {
        const r = await get("/studio/samples/search?q=vinyl%20crackle");
        expect(r.status).toBe(503);
        expect(r.headers.get("retry-after")).toBe("60");
      },
    );
  });

  test("serves a Freesound preview by its path, and no other file", async () => {
    const asked = await upstream(
      () => new Response("OggS", { headers: { "content-type": "application/ogg", "content-length": "4" } }),
      async () => {
        const r = await get("/studio/samples/audio/0/123_456.ogg");
        expect(r.status).toBe(200);
        expect(r.headers.get("content-type")).toBe("audio/ogg");
        expect(r.headers.get("cache-control")).toBe("public, max-age=31536000, immutable");
        expect(await r.text()).toBe("OggS");
        for (const path of ["/studio/samples/audio/0/123.ogg", "/studio/samples/audio/x/123_456.ogg", "/studio/samples/list"]) {
          expect((await get(path)).status, path).toBe(404);
        }
      },
    );
    expect(asked.map((a) => a.url)).toEqual(["https://cdn.freesound.org/previews/0/123_456-hq.ogg"]);
  });

  test("turns down a preview too long for the demo", async () => {
    await upstream(
      () => new Response("x", { headers: { "content-length": String(64 * 1024 * 1024) } }),
      async () => expect((await get("/studio/samples/audio/0/123_456.ogg")).status).toBe(413),
    );
  });

  test("leaves every other path to the static assets, and takes only GET", async () => {
    expect(await (await get("/studio/")).text()).toBe("asset /studio/");
    expect((await get("/studio/samples/search?q=vinyl%20crackle", { method: "POST", body: "x" })).status).toBe(405);
  });
});

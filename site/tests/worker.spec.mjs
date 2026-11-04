// The site's Worker (worker/index.js), run as Cloudflare runs it, with Openverse and the libraries stood in for: it
// asks for a few words of letters and digits at a time, a visitor only so many a minute, with its filters fixed,
// passes on the recordings the demo can play, and serves them by path and nothing else.
import { test, expect } from "@playwright/test";
import worker from "../worker/index.js";

const ORIGIN = "https://claudius.devlato.com";
const assets = { fetch: async (r) => new Response(`asset ${new URL(r.url).pathname}`) };
const get = (path, init, env = {}) =>
  worker.fetch(new Request(ORIGIN + path, init), { ASSETS: assets, ...env }, { waitUntil() {} });

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
  provider: "freesound",
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
const SONG = {
  ...FOUND,
  id: "9a7e",
  provider: "jamendo",
  title: "Disco Fever",
  foreign_landing_url: "https://www.jamendo.com/track/582248",
  url: "https://prod-1.storage.jamendo.com/?trackid=582248&format=mp32",
  duration: 235000,
  filesize: undefined,
};
const RECORD = {
  ...FOUND,
  id: "c0de",
  provider: "wikimedia_audio",
  title: "Stringed Disco",
  foreign_landing_url: "https://commons.wikimedia.org/wiki/File:Stringed_Disco.oga",
  url: "https://upload.wikimedia.org/wikipedia/commons/0/0e/Stringed_Disco_%28MacLeod%2C_Kevin%29.oga",
  duration: 175000,
  filesize: 4604953,
};

test.describe("the samples Worker", () => {
  // run as a Worker runs off the edge, with no cache: serve.mjs puts one in for the test server, and a spec that loads it
  // (the smoke test does) may have run in this process first, keeping the previews these tests serve
  test.beforeEach(() => {
    delete globalThis.caches;
  });

  test("searches Openverse for the words with its filters, and passes on the recordings it can play", async () => {
    let body;
    const asked = await upstream(
      () =>
        Response.json({
          results: [
            FOUND,
            SONG,
            RECORD,
            { ...FOUND, id: "nc", license: "by-nc" },
            { ...RECORD, id: "huge", filesize: 80 * 1024 * 1024 },
            { ...RECORD, id: "flac", url: "https://upload.wikimedia.org/wikipedia/commons/a/ab/x.flac" },
            { ...FOUND, id: "elsewhere", url: "https://example.com/x.mp3" },
          ],
        }),
      async () => {
        const r = await get("/studio/samples/search?q=Disco%20%20Strings");
        expect(r.status).toBe(200);
        expect(r.headers.get("cache-control")).toBe("public, max-age=86400");
        body = await r.json();
      },
    );
    expect(asked).toHaveLength(1);
    const up = new URL(asked[0].url);
    expect(up.origin + up.pathname).toBe("https://api.openverse.org/v1/audio/");
    expect(Object.fromEntries(up.searchParams)).toEqual({
      q: "disco strings",
      license: "cc0,by",
      source: "freesound,jamendo,wikimedia_audio",
      page_size: "20",
      mature: "false",
    });
    expect(asked[0].headers.get("user-agent")).toMatch(/^claudius-site\//);
    const common = {
      creator: "nnaudio",
      license: "by",
      license_version: "4.0",
      license_url: "https://creativecommons.org/licenses/by/4.0/",
      tags: ["piano", "75bpm"],
    };
    expect(body.results).toEqual([
      {
        ...common,
        id: "freesound:123",
        provider: "freesound",
        title: "Sad Hip Hop Piano Keys - 75bpm - Amin",
        page: "https://freesound.org/people/nnaudio/sounds/123/",
        audio: "audio/0/123_456.ogg",
        ext: "ogg",
        seconds: 25.6,
      },
      {
        ...common,
        id: "jamendo:582248",
        provider: "jamendo",
        title: "Disco Fever",
        page: "https://www.jamendo.com/track/582248",
        audio: "audio/jamendo/582248.mp3",
        ext: "mp3",
        seconds: 235,
      },
      {
        ...common,
        id: "commons:Stringed_Disco_(MacLeod,_Kevin).oga",
        provider: "wikimedia_audio",
        title: "Stringed Disco",
        page: "https://commons.wikimedia.org/wiki/File:Stringed_Disco.oga",
        audio: "audio/commons/0/0e/Stringed_Disco_%28MacLeod%2C_Kevin%29.oga",
        ext: "oga",
        seconds: 175,
      },
    ]);
  });

  test("takes a few words of letters and digits, and asks no one for anything else", async () => {
    const asked = await upstream(
      () => Response.json({ results: [] }),
      async () => {
        for (const q of ["anything at all", "R&B", "drum 'n' bass", "70s soul", "música brasileira"]) {
          expect((await get(`/studio/samples/search?q=${encodeURIComponent(q)}`)).status, q).toBe(200);
        }
        for (const q of ["", "  ", "<script>", "a/b", "../etc", "x".repeat(61), "-dash first"]) {
          expect((await get(`/studio/samples/search?q=${encodeURIComponent(q)}`)).status, q).toBe(400);
        }
      },
    );
    expect(asked).toHaveLength(5);
  });

  test("lets a visitor ask Openverse only so often, but not for what the edge has kept", async () => {
    const kept = new Map();
    globalThis.caches = {
      default: { match: async (r) => kept.get(r.url)?.clone(), put: async (r, res) => void kept.set(r.url, res) },
    };
    let left = 1;
    const asked_by = [];
    const env = {
      SEARCHES: {
        limit: async ({ key }) => {
          asked_by.push(key);
          return { success: left-- > 0 };
        },
      },
    };
    const from = (ip) => ({ headers: { "cf-connecting-ip": ip } });
    await upstream(
      () => Response.json({ results: [FOUND] }),
      async () => {
        expect((await get("/studio/samples/search?q=disco", from("1.2.3.4"), env)).status).toBe(200);
        // kept at the edge: asked again, it costs nothing
        await new Promise((r) => setTimeout(r, 0));
        expect((await get("/studio/samples/search?q=disco", from("1.2.3.4"), env)).status).toBe(200);
        const r = await get("/studio/samples/search?q=funk", from("1.2.3.4"), env);
        expect(r.status).toBe(429);
        expect(r.headers.get("retry-after")).toBe("60");
      },
    );
    expect(asked_by).toEqual(["1.2.3.4", "1.2.3.4"]);
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

  test("serves each library's recordings by their paths, and no other file", async () => {
    const asked = await upstream(
      (url) => new Response("OggS", { headers: { "content-length": "4", "x-from": url } }),
      async () => {
        const r = await get("/studio/samples/audio/0/123_456.ogg");
        expect(r.status).toBe(200);
        expect(r.headers.get("content-type")).toBe("audio/ogg");
        expect(r.headers.get("cache-control")).toBe("public, max-age=31536000, immutable");
        expect(await r.text()).toBe("OggS");
        expect((await get("/studio/samples/audio/jamendo/582248.mp3")).headers.get("content-type")).toBe("audio/mpeg");
        const commons = "/studio/samples/audio/commons/0/0e/Stringed_Disco_%28MacLeod%2C_Kevin%29.oga";
        expect((await get(commons)).headers.get("content-type")).toBe("audio/ogg");
        for (const path of [
          "/studio/samples/audio/0/123.ogg",
          "/studio/samples/audio/x/123_456.ogg",
          "/studio/samples/audio/jamendo/x.mp3",
          "/studio/samples/audio/commons/0/0e/x.flac",
          "/studio/samples/audio/commons/0/0e/..%2F..%2Fx.ogg",
          "/studio/samples/audio/commons/zz/0e/x.ogg",
          "/studio/samples/list",
        ]) {
          expect((await get(path)).status, path).toBe(404);
        }
      },
    );
    expect(asked.map((a) => a.url)).toEqual([
      "https://cdn.freesound.org/previews/0/123_456-hq.ogg",
      "https://prod-1.storage.jamendo.com/?trackid=582248&format=mp32",
      "https://upload.wikimedia.org/wikipedia/commons/0/0e/Stringed_Disco_%28MacLeod%2C_Kevin%29.oga",
    ]);
  });

  test("turns down a recording too long for the demo, said or not", async () => {
    await upstream(
      () => new Response("x", { headers: { "content-length": String(64 * 1024 * 1024) } }),
      async () => expect((await get("/studio/samples/audio/0/123_456.ogg")).status).toBe(413),
    );
    await upstream(
      () => new Response(new Uint8Array(13 * 1024 * 1024)),
      async () => expect((await get("/studio/samples/audio/jamendo/582248.mp3")).status).toBe(413),
    );
  });

  test("leaves every other path to the static assets, and takes only GET", async () => {
    expect(await (await get("/studio/")).text()).toBe("asset /studio/");
    expect((await get("/studio/samples/search?q=vinyl%20crackle", { method: "POST", body: "x" })).status).toBe(405);
  });
});

// The download button, in the browser studio: the demo hands over its render as a FLAC file, in the format its
// export menu asks for, or the song itself as a project that claudius opens, and a claudius the song as an MP3,
// saving the song's folder as well. It needs what claudius.mjs says:
//
//   cargo build --bin claudius && cargo xtask web --plan
//   npx playwright test tests/download.spec.mjs      (in site/)
import { test, expect, chromium } from "@playwright/test";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { claudius, heard, ready, why } from "./claudius.mjs";

async function browser() {
  const b = await chromium.launch({ args: ["--autoplay-policy=no-user-gesture-required"] });
  return { page: await b.newPage({ acceptDownloads: true }), close: () => b.close() };
}

test.describe("the download button", () => {
  test.skip(!ready, why);

  test("the demo hands over its song as a FLAC file", async () => {
    test.setTimeout(180_000);
    const c = await claudius();
    const { page, close } = await browser();
    try {
      const toasts = heard(page, "toast");
      const download = page.waitForEvent("download", { timeout: 150_000 });
      await page.goto(`${c.base}/?mock&plan=${encodeURIComponent("wait:2;download;wait:120")}`);
      const file = await download;
      expect(file.suggestedFilename()).toMatch(/^[a-z0-9-]+\.flac$/);
      const flac = readFileSync(await file.path());
      expect(flac.subarray(0, 4).toString()).toBe("fLaC");
      // a minute of 16-bit stereo at 44.1 kHz is 10.6 MB as a WAV, and lossless FLAC is most of that
      expect(flac.length, "the whole song, losslessly").toBeGreaterThan(3_000_000);
      await expect.poll(() => toasts.join("\n")).toMatch(/downloading [a-z0-9-]+\.flac/);
    } finally {
      await close();
      c.stop();
    }
  });

  test("the demo hands over its song in the format asked for", async () => {
    test.setTimeout(180_000);
    const c = await claudius();
    const { page, close } = await browser();
    try {
      const download = page.waitForEvent("download", { timeout: 150_000 });
      await page.goto(`${c.base}/?mock&plan=${encodeURIComponent("wait:2;download:mp3;wait:120")}`);
      const file = await download;
      expect(file.suggestedFilename()).toMatch(/^[a-z0-9-]+\.mp3$/);
      const mp3 = readFileSync(await file.path());
      const tagged = mp3.subarray(0, 3).toString() === "ID3";
      const framed = mp3[0] === 0xff && (mp3[1] & 0xe0) === 0xe0;
      expect(tagged || framed, "an ID3 tag or an MPEG frame first").toBe(true);
      expect(mp3.length).toBeGreaterThan(500_000);
    } finally {
      await close();
      c.stop();
    }
  });

  test("the demo hands over its song as a project that claudius opens", async () => {
    test.setTimeout(180_000);
    const c = await claudius();
    const { page, close } = await browser();
    try {
      const download = page.waitForEvent("download", { timeout: 150_000 });
      await page.goto(`${c.base}/?mock&plan=${encodeURIComponent("wait:2;download:project;wait:120")}`);
      const file = await download;
      const name = file.suggestedFilename();
      expect(name).toMatch(/^[a-z0-9-]+\.zip$/);
      const zip = readFileSync(await file.path());
      expect(zip.subarray(0, 4).toString("latin1")).toBe("PK\x03\x04");
      for (const inside of ["blueprint.json", "song.rhai"]) expect(zip.includes(inside), inside).toBe(true);
      // the zip as the studio sends a chosen one to claudius, which unpacks it into a song folder and opens it
      const opened = await fetch(`${c.base}/api/project?name=${encodeURIComponent(name)}`, {
        method: "POST",
        headers: { "content-type": "application/zip" },
        body: zip,
      });
      const reply = await opened.json();
      expect(reply.ok, reply.message).toBe(true);
      const state = await (await fetch(`${c.base}/api/state`)).json();
      const slug = state.song.title.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
      expect(`${slug}.zip`, "the demo's song").toBe(name);
      const folder = join(c.home, "Music", "claudius", name.replace(/\.zip$/, ""));
      expect(existsSync(join(folder, "blueprint.json")), folder).toBe(true);
    } finally {
      await close();
      c.stop();
    }
  });

  test("a claudius saves the song's folder and hands over its MP3", async () => {
    test.setTimeout(180_000);
    const c = await claudius();
    const { page, close } = await browser();
    try {
      const download = page.waitForEvent("download", { timeout: 150_000 });
      await page.goto(`${c.base}/?plan=${encodeURIComponent("wait:3;download;wait:120")}`);
      const file = await download;
      expect(file.suggestedFilename()).toMatch(/^[a-z0-9-]+\.mp3$/);
      const mp3 = readFileSync(await file.path());
      const tagged = mp3.subarray(0, 3).toString() === "ID3";
      const framed = mp3[0] === 0xff && (mp3[1] & 0xe0) === 0xe0;
      expect(tagged || framed, "an ID3 tag or an MPEG frame first").toBe(true);
      expect(mp3.length).toBeGreaterThan(500_000);
      const songs = join(c.home, "Music", "claudius");
      await expect.poll(() => (existsSync(songs) ? readdirSync(songs).length : 0), { timeout: 60_000 }).toBe(1);
      expect(existsSync(join(songs, readdirSync(songs)[0], "blueprint.json"))).toBe(true);
    } finally {
      await close();
      c.stop();
    }
  });
});

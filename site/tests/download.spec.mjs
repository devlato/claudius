// The download button, in the browser studio: the demo hands over its render as a WAV file, and a claudius the song
// as an MP3, saving the song's folder as well. It needs what claudius.mjs says:
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

  test("the demo hands over its song as a WAV file", async () => {
    test.setTimeout(180_000);
    const c = await claudius();
    const { page, close } = await browser();
    try {
      const toasts = heard(page, "toast");
      const download = page.waitForEvent("download", { timeout: 150_000 });
      await page.goto(`${c.base}/?mock&plan=${encodeURIComponent("wait:2;download;wait:120")}`);
      const file = await download;
      expect(file.suggestedFilename()).toMatch(/^[a-z0-9-]+\.wav$/);
      const wav = readFileSync(await file.path());
      expect([wav.subarray(0, 4).toString(), wav.subarray(8, 12).toString()]).toEqual(["RIFF", "WAVE"]);
      expect((wav.length - 44) / (48_000 * 2 * 2), "seconds of 48 kHz stereo: the whole song").toBeGreaterThan(60);
      await expect.poll(() => toasts.join("\n")).toMatch(/downloading [a-z0-9-]+\.wav/);
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

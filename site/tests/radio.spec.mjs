// The radio (/radio/): its page under its own policy, and the station on air, a song mixed into the next. It needs the
// assembled site (cargo xtask site), whose /studio/ holds the engine the station runs.
import { test, expect, chromium } from "@playwright/test";
import { siteRoot } from "./serve.mjs";

const site = siteRoot();

/** A recording as a library might offer one, as a WAV file: sixteen bars at 90 BPM, A minor and F major four bars each
 *  over a kick on every beat (mono, 16-bit, 22 050 Hz). */
function chords() {
  const rate = 22_050;
  const beat = 60 / 90;
  const n = Math.round(64 * beat * rate);
  const wav = Buffer.alloc(44 + 2 * n);
  wav.write("RIFF", 0);
  wav.writeUInt32LE(36 + 2 * n, 4);
  wav.write("WAVEfmt ", 8);
  wav.writeUInt32LE(16, 16);
  wav.writeUInt16LE(1, 20);
  wav.writeUInt16LE(1, 22);
  wav.writeUInt32LE(rate, 24);
  wav.writeUInt32LE(2 * rate, 28);
  wav.writeUInt16LE(2, 32);
  wav.writeUInt16LE(16, 34);
  wav.write("data", 36);
  wav.writeUInt32LE(2 * n, 40);
  for (let i = 0; i < n; i++) {
    const t = i / rate;
    const minor = Math.floor(t / (16 * beat)) % 2 === 0;
    const notes = minor ? [220, 261.63, 329.63] : [174.61, 220, 261.63];
    const held = notes.reduce((sum, f) => sum + Math.sin(2 * Math.PI * f * t), 0) * 0.1;
    const tb = t % beat;
    const kick = Math.sin(2 * Math.PI * 55 * tb) * Math.exp(-tb / 0.08) * 0.5;
    wav.writeInt16LE(Math.round(Math.max(-1, Math.min(1, held + kick)) * 32_000), 44 + 2 * i);
  }
  return wav;
}

test.describe("the radio", () => {
  test.skip(!site.assembled, "needs target/site from cargo xtask site");

  test("loads under its own policy and goes on air, then mixes the next song in", async ({ baseURL }) => {
    test.setTimeout(300_000);
    const b = await chromium.launch({ args: ["--autoplay-policy=no-user-gesture-required"] });
    try {
      const page = await b.newPage({ baseURL });
      const problems = [];
      page.on("console", (m) => {
        if (m.type() === "error" || /Content Security Policy|Refused to|Trusted Type/i.test(m.text())) problems.push(m.text());
      });
      page.on("pageerror", (e) => problems.push(e.message));
      await page.goto("/radio/?soon");
      await expect(page.locator("h1")).toHaveText("Infinite Radio");
      await page.locator("#tune").click();
      // the first song: written, rendered and playing
      await expect(page.locator("#tune")).toHaveText("Pause", { timeout: 120_000 });
      const first = await page.locator("#now-title").textContent();
      expect(first).not.toBe("Not tuned in");
      await expect(page.locator("#now-meta")).toHaveText(/ BPM · [A-G]/);
      const clock = () => page.locator("#now-time").textContent();
      const t0 = await clock();
      await expect.poll(clock, { timeout: 10_000 }).not.toBe(t0);
      // the next one, made while the first plays and mixed in on a bar line (the first started near its end)
      await expect(page.locator("#played li")).toHaveCount(1, { timeout: 240_000 });
      await expect(page.locator("#played li").first()).toContainText(first);
      await expect(page.locator("#now-title")).not.toHaveText(first);
      expect(problems).toEqual([]);
    } finally {
      await b.close();
    }
  });

  test("samples a song as its genre does: the recordings found and cut, credited on air and handed to the studio", async ({ baseURL }) => {
    test.setTimeout(300_000);
    const b = await chromium.launch({ args: ["--autoplay-policy=no-user-gesture-required"] });
    try {
      const page = await b.newPage({ baseURL });
      const problems = [];
      page.on("pageerror", (e) => problems.push(e.message));
      // the site's search and its recordings, as its Worker answers them: for every search, one recording of chords
      // over a kick, as a library might offer one
      const record = (id) => ({
        id: `freesound:${id}`,
        provider: "freesound",
        title: `Dusty Chords ${id}`,
        creator: "someone",
        license: "by",
        license_version: "4.0",
        license_url: "https://creativecommons.org/licenses/by/4.0/",
        page: `https://freesound.org/s/${id}/`,
        audio: `audio/1/${id}_1.wav`,
        ext: "wav",
        seconds: 20,
        tags: [],
      });
      let searches = 0;
      await page.route("**/studio/samples/search*", (r) => {
        searches += 1;
        return r.fulfill({ contentType: "application/json", body: JSON.stringify({ results: [record(100 + searches)] }) });
      });
      const wav = chords();
      await page.route("**/studio/samples/audio/**", (r) => r.fulfill({ contentType: "audio/wav", body: wav }));
      await page.goto("/radio/?genres=trip_hop");
      await page.locator("#tune").click();
      await expect(page.locator("#tune")).toHaveText("Pause", { timeout: 180_000 });
      // the song on air credits what it samples
      await expect(page.locator("#now-samples")).toHaveText(/^Samples "Dusty Chords \d+" by someone, CC BY 4\.0/);
      expect(searches).toBeGreaterThan(0);
      // the studio is handed the song with the recordings it sampled, to cut them again
      const q = new URL(await page.locator("#open").getAttribute("href"), baseURL).searchParams;
      expect(q.get("sampled")).toBe("1");
      const handed = JSON.parse(q.get("recordings"));
      expect(handed.length).toBeGreaterThan(0);
      expect(handed.map((h) => h.found.title)).toEqual(expect.arrayContaining([expect.stringMatching(/^Dusty Chords \d+$/)]));
      expect(problems).toEqual([]);
    } finally {
      await b.close();
    }
  });

  test("tunes in after the list of genres could not be had", async ({ baseURL }) => {
    test.setTimeout(300_000);
    const b = await chromium.launch({ args: ["--autoplay-policy=no-user-gesture-required"] });
    try {
      const page = await b.newPage({ baseURL });
      // the engine's first start fails, as on a dropped connection; the next is let through
      let asked = 0;
      await page.route("**/studio/claudius-engine_loader.js", (r) => (asked++ === 0 ? r.abort() : r.continue()));
      await page.goto("/radio/?soon");
      await page.locator("#genres summary").click();
      await expect(page.locator("#status")).toContainText("could not be had", { timeout: 60_000 });
      // Tune in starts an engine of its own rather than waiting on the one that failed
      await page.locator("#tune").click();
      await expect(page.locator("#tune")).toHaveText("Pause", { timeout: 120_000 });
      expect(asked).toBeGreaterThan(1);
    } finally {
      await b.close();
    }
  });

  test("keeps to the genres chosen, hands the song on air over as a file, opens it in the studio and skips", async ({ baseURL }) => {
    test.setTimeout(420_000);
    const b = await chromium.launch({ args: ["--autoplay-policy=no-user-gesture-required"] });
    try {
      const page = await b.newPage({ baseURL, acceptDownloads: true });
      const problems = [];
      page.on("pageerror", (e) => problems.push(e.message));
      await page.goto("/radio/?genres=deep_house,house");
      await expect(page.locator("#genres-said")).toHaveText("deep house, house");
      await page.locator("#tune").click();
      await expect(page.locator("#tune")).toHaveText("Pause", { timeout: 120_000 });
      await expect(page.locator("#now-meta")).toHaveText(/^(deep house|house) · /);
      // the studio writes the same song again from what the station asked for
      const open = await page.locator("#open").getAttribute("href");
      expect(open).toMatch(/^\/studio\/\?mock&genre=(deep_house|house)&seed=\d+&bpm=[\d.]+&sampled=0/);
      // the song on air, as an MP3 the engine makes
      const [file] = await Promise.all([page.waitForEvent("download", { timeout: 240_000 }), page.locator("#download").click()]);
      expect(file.suggestedFilename()).toMatch(/\.mp3$/);
      const saved = await file.path();
      expect(saved).toBeTruthy();
      await expect(page.locator("#status")).toContainText("is in your downloads");
      // once the next song is written, Skip brings it in on the next beat, and it is of the genres chosen too
      const first = await page.locator("#now-title").textContent();
      await expect(page.locator("#next")).toContainText(/(mixes|comes) in .* from now/, { timeout: 240_000 });
      await page.locator("#skip").click();
      await expect(page.locator("#now-title")).not.toHaveText(first, { timeout: 5_000 });
      await expect(page.locator("#played li").first()).toContainText(first);
      await expect(page.locator("#now-meta")).toHaveText(/^(deep house|house) · /);
      expect(problems).toEqual([]);
    } finally {
      await b.close();
    }
  });
});

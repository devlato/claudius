// Adding a recording of one's own, end to end: a song dropped on the studio, or chosen in the Add a recording window,
// goes into the song whole as a track, in the demo and with a claudius. It needs what claudius.mjs says:
//
//   cargo build --bin claudius && cargo xtask web --plan
//   npx playwright test tests/import.spec.mjs      (in site/)
import { test, expect, chromium } from "@playwright/test";
import { existsSync, mkdtempSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { claudius, heard, ready, repo, tone, why } from "./claudius.mjs";

const song = join(mkdtempSync(join(tmpdir(), "claudius-import-")), "my song.wav");
tone(song, 6);

/** A band, as a mono 16-bit WAV at 24 kHz: 120 BPM, sixteen bars of a kick on every beat and a hat on every off-beat,
 * alone for four bars, then a bass on the roots and the chords themselves, A minor, F, C and G a bar each, three
 * times round. No voice. */
function band(path, { tonal = true } = {}) {
  const rate = 24_000;
  const n = 16 * 2 * rate;
  const x = new Float32Array(n);
  let seed = 5;
  const noise = () => ((seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648 - 0.5);
  for (let beat = 0; beat < 64; beat++) {
    const at = (beat * rate) / 2;
    for (let i = 0; i < 0.15 * rate; i++) x[at + i] += Math.sin((2 * Math.PI * 60 * i) / rate) * 0.6 * Math.exp(-i / (0.05 * rate));
    for (let i = 0; i < rate / 30; i++) x[at + rate / 4 + i] += noise() * 0.3 * (1 - i / (rate / 30));
  }
  const chords = [[220, 261.63, 329.63], [174.61, 220, 261.63], [261.63, 329.63, 392], [196, 246.94, 293.66]];
  const roots = [55, 43.65, 65.41, 49];
  for (let bar = 4; tonal && bar < 16; bar++) {
    const c = (bar - 4) % 4;
    for (let i = 0; i < 2 * rate; i++) {
      const t = (bar * 2 * rate + i) / rate;
      const up = Math.floor(i / (rate / 2)) % 2 === 1 ? 2 : 1;
      x[bar * 2 * rate + i] += Math.sin(2 * Math.PI * roots[c] * up * t) * 0.3 + chords[c].reduce((s, f) => s + Math.sin(2 * Math.PI * f * t) * 0.09, 0);
    }
  }
  const b = Buffer.alloc(44 + n * 2);
  b.write("RIFF", 0);
  b.writeUInt32LE(36 + n * 2, 4);
  b.write("WAVEfmt ", 8);
  b.writeUInt32LE(16, 16);
  b.writeUInt16LE(1, 20);
  b.writeUInt16LE(1, 22);
  b.writeUInt32LE(rate, 24);
  b.writeUInt32LE(rate * 2, 28);
  b.writeUInt16LE(2, 32);
  b.writeUInt16LE(16, 34);
  b.write("data", 36);
  b.writeUInt32LE(n * 2, 40);
  for (let i = 0; i < n; i++) b.writeInt16LE(Math.round(Math.max(-1, Math.min(1, x[i])) * 32767), 44 + i * 2);
  writeFileSync(path, b);
}

/** the takes a claudius kept with its songs */
const takes = (home) => {
  const dir = join(home, "Music", "claudius", "takes");
  return existsSync(dir) ? readdirSync(dir).filter((f) => f.endsWith(".flac")) : [];
};

/** drops the file at `path` on the studio, as a file dragged from the desktop onto the page lands */
async function drop(page, path) {
  const bytes = [...readFileSync(path)];
  await page.evaluate(([bytes, name]) => {
    const files = new DataTransfer();
    files.items.add(new File([new Uint8Array(bytes)], name, { type: "audio/wav" }));
    const canvas = document.querySelector("canvas");
    for (const type of ["dragenter", "dragover", "drop"]) {
      canvas.dispatchEvent(new DragEvent(type, { dataTransfer: files, bubbles: true, cancelable: true }));
    }
  }, [bytes, "my song.wav"]);
}

test.describe("adding a recording", () => {
  test.skip(!ready, why);

  test("in the demo, a song dropped on the studio goes in whole as a track, kept in the page", async () => {
    test.setTimeout(120_000);
    const c = await claudius();
    const b = await chromium.launch();
    try {
      const page = await b.newPage();
      const said = heard(page, "import");
      // the window adds what it holds as soon as it holds it
      await page.goto(`${c.base}/?mock&plan=${encodeURIComponent("wait:3;import:whole;wait:60")}`);
      await page.waitForTimeout(8_000);
      await drop(page, song);
      await expect.poll(() => said.join("\n"), { timeout: 60_000 }).toMatch(/^take1: a take of [\d.]+ bars from the start of /m);
      expect(takes(c.home), "the claudius serving the page keeps nothing").toHaveLength(0);
    } finally {
      await b.close();
      c.stop();
    }
  });

  test("in the demo, a recording a free library offers is found, used and goes in whole as a track", async () => {
    test.setTimeout(120_000);
    const c = await claudius();
    const b = await chromium.launch();
    try {
      const page = await b.newPage();
      const listed = heard(page, "library");
      const said = heard(page, "import");
      // claudius' site, which the demo searches the libraries through, as its Worker answers
      const rain = {
        id: "freesound:629052", provider: "freesound", title: "Rain on a window", creator: "Someone",
        license: "by", license_version: "4.0", license_url: "https://creativecommons.org/licenses/by/4.0/",
        page: "https://freesound.org/s/629052/", audio: "audio/629/629052_1.ogg", ext: "ogg", seconds: 6, tags: [],
      };
      await page.route("**/samples/search?*", (r) => r.fulfill({ json: { results: [rain] } }));
      await page.route("**/samples/audio/**", (r) => r.fulfill({ body: readFileSync(song), contentType: "audio/wav" }));
      const plan = "wait:3;import:search:rain;wait:5;import:use:0;wait:5;import:whole;wait:40";
      await page.goto(`${c.base}/?mock&plan=${encodeURIComponent(plan)}`);
      await expect.poll(() => listed.join("\n"), { timeout: 30_000 }).toMatch(/^Rain on a window$/m);
      await expect.poll(() => said.join("\n"), { timeout: 60_000 }).toMatch(/^take1: a take of [\d.]+ bars from the start of /m);
    } finally {
      await b.close();
      c.stop();
    }
  });

  test("with a claudius, a recording a search found is added by its id, which claudius downloads", async () => {
    test.setTimeout(120_000);
    const c = await claudius();
    const b = await chromium.launch();
    try {
      const page = await b.newPage();
      const listed = heard(page, "library");
      const said = heard(page, "import");
      // the search, as claudius answers it; this claudius has searched nothing itself, so it cannot download it
      const rain = {
        id: "freesound:629052", title: "Rain on a window", creator: "Someone", licence: "CC BY 4.0", seconds: 6,
        page: "https://freesound.org/s/629052/", audio: "/api/library/audio?id=freesound%3A629052",
        credit: '"Rain on a window" by Someone, CC BY 4.0',
      };
      await page.route("**/api/library/search?*", (r) => r.fulfill({ json: [rain] }));
      const plan = "wait:3;import:search:rain;wait:4;import:use:0;wait:1;import:whole;wait:30";
      await page.goto(`${c.base}/?plan=${encodeURIComponent(plan)}`);
      await expect.poll(() => listed.join("\n"), { timeout: 30_000 }).toMatch(/^Rain on a window$/m);
      await expect.poll(() => said.join("\n"), { timeout: 30_000 }).toMatch(/no recording 'freesound:629052' among the searches/);
    } finally {
      await b.close();
      c.stop();
    }
  });

  test("in the demo, the Sample window offers pieces of a recording dropped, and one kept treated plays in the song", async () => {
    test.setTimeout(150_000);
    const c = await claudius();
    const b = await chromium.launch();
    try {
      const page = await b.newPage();
      const offered = heard(page, "pieces");
      const toasts = heard(page, "toast");
      const keep = JSON.stringify({ cmd: "keep_pieces", keep: [{ id: 1, treated: true }] });
      await page.goto(`${c.base}/?mock&plan=${encodeURIComponent(`wait:3;import:sample;wait:45;cmd:${keep};wait:30`)}`);
      await page.waitForTimeout(8_000);
      await drop(page, song);
      await expect.poll(() => offered.join("\n"), { timeout: 45_000 }).toMatch(/^take 0: 1\. bars? [\d-]+, 0:\d\d \(dusty/m);
      await expect.poll(() => toasts.join("\n"), { timeout: 60_000 }).toMatch(/^vox1:/m);
    } finally {
      await b.close();
      c.stop();
    }
  });

  test("in the demo, under the site's policy, the Sample window's pieces play to be heard, as cut and treated", async () => {
    test.setTimeout(150_000);
    const c = await claudius();
    // a page that starts audio without a person's click, as a plan does
    const b = await chromium.launch({ args: ["--autoplay-policy=no-user-gesture-required"] });
    try {
      const page = await b.newPage();
      // the policy the site serves the studio under (the upgrade to https left out, for a claudius on http)
      const rules = readFileSync(join(repo, "site", "_headers"), "utf8");
      const policy = /^\/studio\/\*\n(?: .*\n)*?  Content-Security-Policy: (.+)$/m.exec(rules)[1].replace(/; upgrade-insecure-requests/, "");
      await page.route((url) => url.pathname === "/", async (route) => {
        const response = await route.fetch();
        await route.fulfill({ response, headers: { ...response.headers(), "content-security-policy": policy } });
      });
      const offered = heard(page, "pieces");
      const hearing = heard(page, "hearing");
      const plan = "wait:3;import:sample;wait:50;import:hear:1;wait:8;import:hear:1:treated;wait:20";
      await page.goto(`${c.base}/?mock&plan=${encodeURIComponent(plan)}`);
      await page.waitForTimeout(8_000);
      await drop(page, song);
      await expect.poll(() => offered.join("\n"), { timeout: 45_000 }).toMatch(/^take 0: 1\. bars? [\d-]+/m);
      await expect.poll(() => hearing.join("\n"), { timeout: 60_000 }).toMatch(/^1 as cut plays$/m);
      await expect.poll(() => hearing.join("\n"), { timeout: 30_000 }).toMatch(/^1 treated plays$/m);
      expect(hearing.join("\n")).not.toMatch(/did not play/);
    } finally {
      await b.close();
      c.stop();
    }
  });

  test("in the demo, Pieces of it offers a recording's drum break and a loop of its hook, each by its kind", async () => {
    test.setTimeout(150_000);
    const c = await claudius();
    const b = await chromium.launch();
    try {
      const page = await b.newPage();
      const offered = heard(page, "pieces");
      await page.goto(`${c.base}/?mock&plan=${encodeURIComponent("wait:3;import:pieces;wait:80")}`);
      await page.waitForTimeout(8_000);
      const file = join(mkdtempSync(join(tmpdir(), "claudius-import-")), "band.wav");
      band(file);
      await drop(page, file);
      await expect.poll(() => offered.join("\n"), { timeout: 60_000 }).toMatch(/^take 0: .*\d\. break: bars? [\d-]+/m);
      expect(offered.join("\n")).toMatch(/\d\. hook: bars? [\d-]+/);
    } finally {
      await b.close();
      c.stop();
    }
  });

  test("in the demo, Pieces of it of drums alone offers their break and says why there is no phrase", async () => {
    test.setTimeout(150_000);
    const c = await claudius();
    const b = await chromium.launch();
    try {
      const page = await b.newPage();
      const offered = heard(page, "pieces");
      await page.goto(`${c.base}/?mock&plan=${encodeURIComponent("wait:3;import:pieces;wait:80")}`);
      await page.waitForTimeout(8_000);
      const file = join(mkdtempSync(join(tmpdir(), "claudius-import-")), "drums.wav");
      band(file, { tonal: false });
      await drop(page, file);
      await expect.poll(() => offered.join("\n"), { timeout: 60_000 }).toMatch(/^take 0: 1\. break: bars? [\d-]+.* \| no phrase: \w/m);
    } finally {
      await b.close();
      c.stop();
    }
  });

  test("with a claudius, the Sample window offers pieces of a recording, and one kept treated plays in the song", async () => {
    test.setTimeout(150_000);
    const c = await claudius();
    const b = await chromium.launch();
    try {
      const page = await b.newPage();
      const offered = heard(page, "pieces");
      const keep = JSON.stringify({ cmd: "keep_pieces", keep: [{ id: 1, treated: true }] });
      const plan = `wait:3;pick:recording;import:sample;wait:40;cmd:${keep};wait:30`;
      await page.goto(`${c.base}/?plan=${encodeURIComponent(plan)}`);
      const input = page.locator('input[data-claudius-pick="recording"]');
      await input.waitFor({ state: "attached", timeout: 30_000 });
      await input.setInputFiles(song);
      await expect.poll(() => offered.join("\n"), { timeout: 40_000 }).toMatch(/^take 0: 1\. bars? [\d-]+, 0:\d\d \(dusty/m);
      await expect
        .poll(async () => (await (await c.api("/api/state")).json()).song.tracks.map((t) => t.name), { timeout: 60_000 })
        .toContain("vox1");
      expect((await (await c.api("/api/state")).json()).pieces ?? null, "kept, the offer goes").toBeNull();
    } finally {
      await b.close();
      c.stop();
    }
  });

  test("with a claudius, a song chosen goes in whole as a track, kept with the songs", async () => {
    test.setTimeout(120_000);
    const c = await claudius();
    const b = await chromium.launch();
    try {
      const page = await b.newPage();
      const said = heard(page, "import");
      await page.goto(`${c.base}/?plan=${encodeURIComponent("wait:3;pick:recording;import:whole;wait:60")}`);
      const input = page.locator('input[data-claudius-pick="recording"]');
      await input.waitFor({ state: "attached", timeout: 30_000 });
      await input.setInputFiles(song);
      await expect.poll(() => said.join("\n"), { timeout: 60_000 }).toMatch(/^take1: a take of [\d.]+ bars from the start of /m);
      const state = await (await c.api("/api/state")).json();
      const take = state.song.tracks.find((t) => t.name === "take1");
      expect(take?.kind).toBe("take");
      expect(take.seconds).toBeGreaterThan(5.5);
      expect(takes(c.home)).toHaveLength(1);
    } finally {
      await b.close();
      c.stop();
    }
  });
});

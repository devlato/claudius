// Adding a recording of one's own, end to end: a song dropped on the studio, or chosen in the Add a recording window,
// goes into the song whole as a track, in the demo and with a claudius. It needs what claudius.mjs says:
//
//   cargo build --bin claudius && cargo xtask web --plan
//   npx playwright test tests/import.spec.mjs      (in site/)
import { test, expect, chromium } from "@playwright/test";
import { existsSync, mkdtempSync, readdirSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { claudius, heard, ready, repo, tone, why } from "./claudius.mjs";

const song = join(mkdtempSync(join(tmpdir(), "claudius-import-")), "my song.wav");
tone(song, 6);

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

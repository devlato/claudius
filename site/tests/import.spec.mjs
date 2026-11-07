// Adding a recording of one's own, end to end: a song dropped on the studio, or chosen in the Add a recording window,
// goes into the song whole as a track, in the demo and with a claudius. It needs what claudius.mjs says:
//
//   cargo build --bin claudius && cargo xtask web --plan
//   npx playwright test tests/import.spec.mjs      (in site/)
import { test, expect, chromium } from "@playwright/test";
import { existsSync, mkdtempSync, readdirSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { claudius, heard, ready, tone, why } from "./claudius.mjs";

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

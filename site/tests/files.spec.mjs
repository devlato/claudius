// Files chosen on this computer, in the browser studio: the page opens the browser's own dialog (its file input, which
// the test fills as a person choosing a file would), reads the file into memory and sends claudius what it holds. It
// needs what claudius.mjs says:
//
//   cargo build --bin claudius && cargo xtask web --plan
//   npx playwright test tests/files.spec.mjs      (in site/)
import { test, expect, chromium } from "@playwright/test";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { deflateSync } from "node:zlib";
import { claudius, heard, ready, tone, why } from "./claudius.mjs";

const scratch = mkdtempSync(join(tmpdir(), "claudius-files-"));

/** a PNG `side` pixels a side, all one grey */
function png(path, side) {
  const crc = (bytes) => {
    let c = ~0;
    for (const b of bytes) {
      c ^= b;
      for (let k = 0; k < 8; k++) c = c & 1 ? (c >>> 1) ^ 0xedb88320 : c >>> 1;
    }
    return ~c >>> 0;
  };
  const chunk = (type, data) => {
    const body = Buffer.concat([Buffer.from(type), data]);
    const head = Buffer.alloc(4);
    head.writeUInt32BE(data.length);
    const tail = Buffer.alloc(4);
    tail.writeUInt32BE(crc(body));
    return Buffer.concat([head, body, tail]);
  };
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(side, 0);
  ihdr.writeUInt32BE(side, 4);
  ihdr[8] = 8; // 8 bits a channel
  ihdr[9] = 2; // RGB
  const row = Buffer.concat([Buffer.from([0]), Buffer.alloc(side * 3, 0x80)]);
  const pixels = deflateSync(Buffer.concat(Array(side).fill(row)));
  const signature = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
  writeFileSync(path, Buffer.concat([signature, chunk("IHDR", ihdr), chunk("IDAT", pixels), chunk("IEND", Buffer.alloc(0))]));
}

const recording = join(scratch, "a song (live).wav");
tone(recording, 8);
const cover = join(scratch, "cover.png");
png(cover, 3000);

async function browser() {
  const b = await chromium.launch({ args: ["--autoplay-policy=no-user-gesture-required"] });
  return { page: await b.newPage(), close: () => b.close() };
}

/** chooses `file` in the dialog the page opens for `what` ("song", "audio" or "cover"), as a person would */
async function choose(page, what, file) {
  const input = page.locator(`input[data-claudius-pick="${what}"]`);
  await input.waitFor({ state: "attached", timeout: 30_000 });
  await input.setInputFiles(file);
}

const state = async (c) => (await c.api("/api/state")).json();

test.describe("files chosen in the browser", () => {
  test.skip(!ready, why);

  test("a recording chosen is listened to, then made a song of", async () => {
    test.setTimeout(180_000);
    const c = await claudius();
    const { page, close } = await browser();
    try {
      const analysis = heard(page, "analysis");
      // the New song dialog: Choose a file, Analyze, then Remake (as the open song's genre)
      const plan = "wait:3;newsong;wait:1;pick:audio;wait:4;listen:analyze;wait:10;listen:remake;wait:10";
      await page.goto(`${c.base}/?plan=${encodeURIComponent(plan)}`);
      await choose(page, "audio", recording);
      await expect.poll(() => analysis.join("\n"), { timeout: 60_000 }).toMatch(/\d+\.\d BPM \(pulse clarity/);
      await expect.poll(() => analysis.join("\n"), { timeout: 60_000 }).toMatch(/remade a song \(live\)\.wav as house/);
      expect((await state(c)).log.join("\n"), "claudius named the file as it was chosen").toMatch(
        /analyzed a song \(live\)\.wav/,
      );
    } finally {
      await close();
      c.stop();
    }
  });

  test("what claudius hears in a recording shows in its window, which plays the recording claudius kept", async () => {
    test.setTimeout(180_000);
    const c = await claudius();
    const { page, close } = await browser();
    try {
      const hears = heard(page, "hears");
      const plan = "wait:3;newsong;wait:1;pick:audio;wait:4;listen:analyze;wait:12;hears:play;wait:4;hears:pause;wait:2";
      await page.goto(`${c.base}/?plan=${encodeURIComponent(plan)}`);
      await choose(page, "audio", recording);
      await expect.poll(() => hears.join("\n"), { timeout: 60_000 }).toMatch(/^\d+ BPM in \S+ \S+, \d+ bars, paused on recording at 0\.0 s$/m);
      // the window's play button plays the recording from claudius, and its pause stops it where it got to
      const played = () => hears.map((h) => Number(/playing recording at (\d+\.\d) s/.exec(h)?.[1] ?? 0));
      await expect.poll(() => Math.max(0, ...played()), { timeout: 60_000 }).toBeGreaterThan(1.5);
      await expect.poll(() => hears.at(-1), { timeout: 30_000 }).toMatch(/paused on recording at [1-9]\.\d s$/);
    } finally {
      await close();
      c.stop();
    }
  });

  test("a song chosen opens, and a file that is no song says so", async () => {
    test.setTimeout(180_000);
    const c = await claudius();
    // the song to open, saved; then another one
    const folder = join(c.home, "Music", "claudius", "first");
    expect((await (await c.api("/api/command", { cmd: "save", path: folder })).json()).ok).toBe(true);
    const title = (await state(c)).song.title;
    expect((await (await c.api("/api/command", { cmd: "new_song", genre: "techno", seed: 11 })).json()).ok).toBe(true);
    expect((await state(c)).song.title).not.toBe(title);
    const { page, close } = await browser();
    try {
      const toasts = heard(page, "toast");
      await page.goto(`${c.base}/?plan=${encodeURIComponent("wait:3;pick:song;wait:6;pick:song;wait:6")}`);
      await choose(page, "song", join(folder, "blueprint.json"));
      await expect.poll(async () => (await state(c)).song.title, { timeout: 30_000 }).toBe(title);
      await choose(page, "song", cover);
      await expect.poll(() => toasts.join("\n"), { timeout: 30_000 }).toMatch(
        /cover\.png is not a song: choose a blueprint\.json or a claudius script/,
      );
    } finally {
      await close();
      c.stop();
    }
  });

  test("a picture chosen becomes the release's cover", async () => {
    test.setTimeout(120_000);
    const c = await claudius();
    const { page, close } = await browser();
    try {
      await page.goto(`${c.base}/?plan=${encodeURIComponent("wait:3;publish;wait:2;pick:cover;wait:6")}`);
      await choose(page, "cover", cover);
      await expect.poll(async () => (await state(c)).release?.artwork ?? "", { timeout: 30_000 }).toMatch(/3000 × 3000 PNG/);
    } finally {
      await close();
      c.stop();
    }
  });
});

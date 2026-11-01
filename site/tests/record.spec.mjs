// Recording in the browser studio, end to end: a claudius serves the studio, the browser's microphone is a file
// Chrome plays as one, and a scripted plan (`?plan=`, in a studio built with its `plan` feature only) presses the
// Record window's buttons. It needs a claudius and that studio:
//
//   cargo build --bin claudius && cargo xtask web --plan
//   npx playwright test tests/record.spec.mjs      (in site/)
//
// CLAUDIUS_BIN and CLAUDIUS_PLAN_STUDIO name others. Each test's claudius has a home of its own, which goes after.
import { test, expect, chromium } from "@playwright/test";
import { spawn } from "node:child_process";
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");
const bin = process.env.CLAUDIUS_BIN || join(repo, "target", "debug", "claudius");
const studio = process.env.CLAUDIUS_PLAN_STUDIO || join(repo, "target", "studio-plan");
const ready = existsSync(bin) && existsSync(join(studio, "index.html")) && existsSync(join(studio, "record-worklet.js"));
const why = `needs ${bin} and the studio built with its plan feature in ${studio} (see the top of this file)`;

/** a mono 16-bit WAV of a 440 Hz tone, at a quarter of full scale: what the fake microphone hears */
function tone(path, secs = 10, rate = 48_000) {
  const n = secs * rate;
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
  for (let i = 0; i < n; i++) b.writeInt16LE(Math.round(0.25 * 32767 * Math.sin((2 * Math.PI * 440 * i) / rate)), 44 + i * 2);
  writeFileSync(path, b);
}

/** a port nothing listens on */
const freePort = () =>
  new Promise((ok) => {
    const s = createServer();
    s.listen(0, "127.0.0.1", () => {
      const { port } = s.address();
      s.close(() => ok(port));
    });
  });

/** a claudius of its own, serving the studio, with a song rendered */
async function claudius() {
  const home = mkdtempSync(join(tmpdir(), "claudius-record-"));
  const port = await freePort();
  const env = { ...process.env, HOME: home, CLAUDIUS_HOME: join(home, "cache") };
  const args = ["serve", "--port", String(port), "--web", studio, "--no-ears", "--no-library"];
  const child = spawn(bin, args, { env, stdio: ["ignore", "ignore", "pipe"] });
  let log = "";
  child.stderr.on("data", (d) => (log += d));
  const base = `http://127.0.0.1:${port}`;
  const api = (path, body) =>
    fetch(base + path, body && { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(body) });
  for (let i = 0; ; i++) {
    if (await api("/api/hello").then((r) => r.ok, () => false)) break;
    if (i > 100) throw new Error(`claudius did not start:\n${log}`);
    await new Promise((r) => setTimeout(r, 200));
  }
  await api("/api/command", { cmd: "new_song", genre: "house", seed: 3 });
  await expect.poll(async () => (await (await api("/api/state")).json()).render?.url ?? "", { timeout: 120_000 }).toMatch(/\.mp3$/);
  const stop = () => {
    child.kill();
    rmSync(home, { recursive: true, force: true });
  };
  return { base, home, api, stop };
}

/** what the Record window says, as the plan reports it */
function said(page) {
  const lines = [];
  page.on("console", (m) => {
    const t = m.text();
    if (t.startsWith("record: ")) lines.push(t.slice(8));
  });
  return lines;
}

/** the take files in a claudius' home: where a song that is not saved yet keeps its takes */
const takes = (home) => {
  const dir = join(home, "Music", "claudius", "takes");
  return existsSync(dir) ? readdirSync(dir).filter((f) => f.endsWith(".flac")).map((f) => join(dir, f)) : [];
};

/** a FLAC file's rate and length in seconds, from its STREAMINFO */
function flacInfo(path) {
  const b = readFileSync(path);
  expect(b.subarray(0, 4).toString()).toBe("fLaC");
  const s = b.subarray(8, 8 + 34);
  const rate = (s[10] << 12) | (s[11] << 4) | (s[12] >> 4);
  const frames = (s[13] & 0x0f) * 2 ** 32 + s.readUInt32BE(14);
  return { rate, secs: frames / rate, bytes: b.length };
}

const microphone = join(mkdtempSync(join(tmpdir(), "claudius-microphone-")), "tone.wav");
tone(microphone);

/** a browser whose microphone is the tone, and which lets `origin` use it; or, `allowed` false, one blocked for it */
async function browser(origin, allowed = true) {
  const args = ["--autoplay-policy=no-user-gesture-required", "--use-fake-device-for-media-stream"];
  // the flag that answers the browser's question says yes to a site that is blocked too
  if (allowed) args.push("--use-fake-ui-for-media-stream", `--use-file-for-fake-audio-capture=${microphone}`);
  // a headless shell cannot ask at all (NotSupportedError): the whole browser says no as a person does
  const b = await chromium.launch(allowed ? { args } : { args, channel: "chromium" });
  const page = await b.newPage({ permissions: allowed ? ["microphone"] : [] });
  if (!allowed) {
    // as when the person chose Block for the site
    const cdp = await page.context().newCDPSession(page);
    await cdp.send("Browser.setPermission", { permission: { name: "microphone" }, setting: "denied", origin });
  }
  return { page, close: () => b.close() };
}

test.describe("recording in the browser", () => {
  test.skip(!ready, why);

  test("a take goes to claudius, plays in the song, and holds what the microphone heard", async () => {
    test.setTimeout(180_000);
    const c = await claudius();
    const { page, close } = await browser(c.base);
    try {
      const lines = said(page);
      const plan = "wait:3;record;wait:2;rec:start;wait:7;rec:stop;wait:5";
      await page.goto(`${c.base}/?plan=${encodeURIComponent(plan)}`);
      await expect.poll(() => lines.join("\n"), { timeout: 60_000 }).toMatch(/take1: a take of [\d.]+ bars from the start of intro/);
      expect(lines.join("\n")).not.toMatch(/dropped audio|went silent/);
      const state = await (await c.api("/api/state")).json();
      const take = state.song.tracks.find((t) => t.name === "take1");
      expect(take, "the take is a track of the song").toBeTruthy();
      expect(take.kind).toBe("take");
      // about seven seconds of the plan, less the count-in and the browser's start: four seconds or so of the tone,
      // which squeezes into far more than silence would
      const files = takes(c.home);
      expect(files).toHaveLength(1);
      const info = flacInfo(files[0]);
      expect(info.rate).toBe(48_000);
      expect(info.secs).toBeGreaterThan(2.5);
      expect(info.secs).toBeLessThan(7.5);
      expect(info.bytes / info.secs, "a tone, not silence").toBeGreaterThan(20_000);
    } finally {
      await close();
      c.stop();
    }
  });

  test("measuring with an input that cannot hear the clicks says so", async () => {
    test.setTimeout(120_000);
    const c = await claudius();
    const { page, close } = await browser(c.base);
    try {
      const lines = said(page);
      await page.goto(`${c.base}/?plan=${encodeURIComponent("wait:3;record;wait:2;rec:measure;wait:8")}`);
      await expect.poll(() => lines.join("\n"), { timeout: 45_000 }).toMatch(/claudius did not hear its clicks/);
    } finally {
      await close();
      c.stop();
    }
  });

  test("a page that may not use the microphone is told what to do", async () => {
    test.setTimeout(120_000);
    const c = await claudius();
    const { page, close } = await browser(c.base, false);
    try {
      const lines = said(page);
      await page.goto(`${c.base}/?plan=${encodeURIComponent("wait:3;record;wait:5")}`);
      await expect.poll(() => lines.join("\n"), { timeout: 30_000 }).toMatch(/not allowed to use the microphone.*address bar/);
    } finally {
      await close();
      c.stop();
    }
  });
});

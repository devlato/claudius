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

/** what the Record window says, as the plan reports it, and when (ms since 1970) it said each */
function said(page) {
  const lines = [];
  lines.when = {};
  page.on("console", (m) => {
    const t = m.text();
    if (t.startsWith("record: ")) {
      lines.push(t.slice(8));
      lines.when[t.slice(8)] ??= Date.now();
    }
  });
  return lines;
}

/** what the Record window offers for MIDI keyboards, as the plan reports it */
function offered(page) {
  const lines = [];
  page.on("console", (m) => {
    const t = m.text();
    if (t.startsWith("midi: ")) lines.push(t.slice(6));
  });
  return lines;
}

/**
 * Web MIDI as a browser gives it, before the page's own scripts: `state` is what the browser says of the page
 * ("granted", "prompt" (it asks, and the person says yes) or "denied"), or "absent" for a browser without it. Its one
 * keyboard plays middle C every half second, held a fifth of a second, to whatever listens, its messages stamped where
 * they arrived, or 0 when `stamped` is false, as Chromium stamps a real keyboard's.
 */
function fakeMidi({ state, stamped = true, slow = 0 }) {
  if (state === "absent") {
    delete Navigator.prototype.requestMIDIAccess;
    return;
  }
  const input = { id: "fake", name: "Fake Keyboard", type: "input", state: "connected", connection: "open", onmidimessage: null };
  const access = { inputs: new Map([["fake", input]]), outputs: new Map(), sysexEnabled: false, onstatechange: null };
  // a browser that takes `slow` ms to give access, as one that looks into its devices first
  const later = (f) => new Promise((ok, no) => setTimeout(() => f(ok, no), slow));
  Navigator.prototype.requestMIDIAccess = () =>
    later((ok, no) => (state === "denied" ? no(new DOMException("blocked", "NotAllowedError")) : ok(access)));
  const query = navigator.permissions.query.bind(navigator.permissions);
  navigator.permissions.query = (d) => (d && d.name === "midi" ? Promise.resolve({ state }) : query(d));
  const send = (bytes, at) =>
    input.onmidimessage && input.onmidimessage({ data: new Uint8Array(bytes), timeStamp: stamped ? at : 0 });
  // a keyboard's messages are stamped when they arrive, however late the page gets round to them: a timer that fires
  // late, as on a page busy drawing, does not move the notes (and no message comes before its time)
  const t0 = performance.now();
  let next = 0;
  const due = [];
  setInterval(() => {
    const now = performance.now();
    for (; t0 + next * 500 <= now; next++) due.push([t0 + next * 500, [0x90, 60, 100]], [t0 + next * 500 + 200, [0x80, 60, 0]]);
    due.sort((a, b) => a[0] - b[0]);
    while (due.length && due[0][0] <= now) {
      const [at, bytes] = due.shift();
      send(bytes, at);
    }
  }, 20);
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
      // the recorder hands over the last of what it heard when it is told to stop, long before the window gives up
      // waiting for it
      const asked = lines.when["finishing ..."];
      const sent = lines.find((l) => l.startsWith("sending "));
      expect(asked, "the take waited for the last of what was heard").toBeTruthy();
      expect(lines.when[sent] - asked).toBeLessThan(800);
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

  test("notes played on a MIDI keyboard become a part, a note on every beat", async () => {
    test.setTimeout(180_000);
    const c = await claudius();
    const { page, close } = await browser(c.base);
    try {
      // the keyboard is chosen as the window opens, 1.5 s before the browser has listed it: it must stay chosen
      await page.addInitScript(fakeMidi, { state: "granted", slow: 4_500 });
      const lines = said(page);
      const plan = "wait:3;record;rec:midi:Fake Keyboard;rec:grid:0;wait:3;rec:start;wait:7;rec:stop;wait:4";
      await page.goto(`${c.base}/?plan=${encodeURIComponent(plan)}`);
      await expect.poll(() => lines.join("\n"), { timeout: 60_000 }).toMatch(/\d+ notes from the start of intro, C4 to C4, played by played/);
      // about four and a half seconds after the count-in, two notes a second
      const n = Number(lines.join("\n").match(/(\d+) notes from the start of intro/)[1]);
      expect(n).toBeGreaterThanOrEqual(6);
      expect(n).toBeLessThanOrEqual(12);
      const state = await (await c.api("/api/state")).json();
      expect(state.song.tracks.find((t) => t.name === "played"), "the notes are a part of the song").toBeTruthy();
      // as played: a key every half second, held a fifth of one, is a note a beat at 120 BPM, held 0.4 of a beat
      const folder = join(c.home, "Music", "claudius", "played");
      expect((await (await c.api("/api/command", { cmd: "save", path: folder })).json()).ok).toBe(true);
      const bp = JSON.parse(readFileSync(join(folder, "blueprint.json"), "utf8"));
      const part = bp.parts.find((p) => p.name === "played");
      const intro = bp.form.find((f) => f.name === "intro");
      const all = part.notes[intro.prog];
      expect(all.length).toBe(n);
      // a key held over the downbeat starts with the take, and is shorter
      const notes = all[0].t === 0 && all[0].d < 0.34 ? all.slice(1) : all;
      expect(notes.length).toBeGreaterThanOrEqual(5);
      for (const [a, b] of notes.slice(1).map((x, i) => [notes[i], x])) {
        expect(Math.abs(b.t - a.t - 1), `a beat apart: ${a.t} then ${b.t}`).toBeLessThan(0.06);
      }
      for (const note of notes) expect(Math.abs(note.d - 0.4), `held 0.4 of a beat: ${note.d}`).toBeLessThan(0.06);
    } finally {
      await close();
      c.stop();
    }
  });

  test("a keyboard the page has to ask for, stamped as Chromium stamps a real one, still makes a part", async () => {
    test.setTimeout(180_000);
    const c = await claudius();
    const { page, close } = await browser(c.base);
    try {
      await page.addInitScript(fakeMidi, { state: "prompt", stamped: false });
      const lines = said(page);
      const offers = offered(page);
      const plan =
        "wait:3;record;wait:1;rec:allow-midi;wait:1;rec:midi:Fake Keyboard;wait:1;rec:start;wait:7;rec:stop;wait:4";
      await page.goto(`${c.base}/?plan=${encodeURIComponent(plan)}`);
      await expect.poll(() => lines.join("\n"), { timeout: 60_000 }).toMatch(/\d+ notes from the start of intro/);
      expect(offers.join("\n"), "the window offered to ask first").toMatch(/let the page use them/);
      const n = Number(lines.join("\n").match(/(\d+) notes from the start of intro/)[1]);
      expect(n).toBeGreaterThanOrEqual(6);
      expect(n).toBeLessThanOrEqual(12);
    } finally {
      await close();
      c.stop();
    }
  });

  test("a browser without Web MIDI, or a page blocked from it, says so", async () => {
    test.setTimeout(120_000);
    for (const [state, words] of [
      ["absent", /this browser has none \(Chrome, Edge and Firefox have\)/],
      ["denied", /blocked for this page: allow them in the site settings/],
    ]) {
      const c = await claudius();
      const { page, close } = await browser(c.base);
      try {
        await page.addInitScript(fakeMidi, { state });
        const lines = offered(page);
        await page.goto(`${c.base}/?plan=${encodeURIComponent("wait:3;record;wait:3")}`);
        await expect.poll(() => lines.join("\n"), { timeout: 30_000 }).toMatch(words);
      } finally {
        await close();
        c.stop();
      }
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

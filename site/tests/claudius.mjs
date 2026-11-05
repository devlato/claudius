// What the studio's end-to-end tests share: a claudius of their own serving the web studio built with its `plan`
// feature (`cargo xtask web --plan`), and what its scripted plans report. It needs that claudius and that studio:
//
//   cargo build --bin claudius && cargo xtask web --plan
//
// CLAUDIUS_BIN and CLAUDIUS_PLAN_STUDIO name others. Each claudius has a home of its own, which goes after.
import { expect } from "@playwright/test";
import { spawn } from "node:child_process";
import { existsSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const repo = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");
export const bin = process.env.CLAUDIUS_BIN || join(repo, "target", "debug", "claudius");
export const studio = process.env.CLAUDIUS_PLAN_STUDIO || join(repo, "target", "studio-plan");
export const ready = existsSync(bin) && existsSync(join(studio, "index.html")) && existsSync(join(studio, "record-worklet.js"));
export const why = `needs ${bin} and the studio built with its plan feature in ${studio} (see the top of this file)`;

/** a mono 16-bit WAV of a 440 Hz tone, at a quarter of full scale: what the fake microphone hears */
export function tone(path, secs = 10, rate = 48_000) {
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
export const freePort = () =>
  new Promise((ok) => {
    const s = createServer();
    s.listen(0, "127.0.0.1", () => {
      const { port } = s.address();
      s.close(() => ok(port));
    });
  });

/** a claudius of its own, serving the studio, with a song rendered */
export async function claudius() {
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

/** what a scripted plan reports of `thing` ("record", "midi", "analysis", "toast"), and when (ms since 1970) each */
export function heard(page, thing) {
  const lines = [];
  lines.when = {};
  page.on("console", (m) => {
    const t = m.text();
    if (t.startsWith(`${thing}: `)) {
      const line = t.slice(thing.length + 2);
      lines.push(line);
      lines.when[line] ??= Date.now();
    }
  });
  return lines;
}

/**
 * On the phone's page: a take started on the bar after next, two bars of a beat struck on its pads in time with the
 * song (the first lane on every other beat, the second on the others), and the take sent. The pads are struck by touch,
 * or with `midi`, by a drum pad's General MIDI notes (`window.sendMidi`, a controller a test stands in for). Returns what
 * the page says.
 */
export function playTwoBars(midi) {
  const wait = (ms) => new Promise((r) => setTimeout(r, Math.max(0, ms)));
  const [low, high] = song.lanes;
  const NOTES = { kick: 36, snare: 38, clap: 39, hat: 42, ohat: 46, tom: 45, crash: 49, ride: 51, shaker: 70, perc: 37 };
  const strike = (lane) => {
    if (midi) return window.sendMidi(0x99, NOTES[song.roles[song.lanes.indexOf(lane)]], 120);
    // the pads are drawn: struck where the page says the pad is
    const r = padRect(lane);
    const at = { bubbles: true, clientX: r.left + 10, clientY: r.top + 10 };
    document.getElementById("padgrid").dispatchEvent(new PointerEvent("pointerdown", at));
  };
  const now = () => songAt(performance.now() + latency());
  const beat = 60 / song.bpm;
  return (async () => {
    const t0 = (Math.ceil(now() / (4 * beat)) + 1) * 4 * beat;
    await wait((t0 - 0.2 - now()) * 1000);
    record();
    for (let b = 0; b < 8; b++) {
      await wait((t0 + b * beat - now()) * 1000);
      strike(b % 2 ? high : low);
    }
    await wait((t0 + 8 * beat + 0.05 - now()) * 1000);
    record();
    const said = () => document.getElementById("said").textContent;
    for (let i = 0; i < 100 && !/played|nothing|cannot|no /.test(said()); i++) await wait(100);
    return { said: said(), low, high };
  })();
}

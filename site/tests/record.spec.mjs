// Recording in the browser studio, end to end: a claudius serves the studio, the browser's microphone is a file
// Chrome plays as one, and a scripted plan (`?plan=`, in a studio built with its `plan` feature only) presses the
// Record window's buttons. It needs a claudius and that studio:
//
//   cargo build --bin claudius && cargo xtask web --plan
//   npx playwright test tests/record.spec.mjs      (in site/)
//
// CLAUDIUS_BIN and CLAUDIUS_PLAN_STUDIO name others. Each test's claudius has a home of its own, which goes after.
import { test, expect, chromium } from "@playwright/test";
import { existsSync, mkdtempSync, readdirSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { claudius, heard, ready, tone, why } from "./claudius.mjs";

/** what the Record window says, as the plan reports it, and when */
const said = (page) => heard(page, "record");

/** what the Record window offers for MIDI keyboards, as the plan reports it */
const offered = (page) => heard(page, "midi");

/**
 * Web MIDI as a browser gives it, before the page's own scripts: `state` is what the browser says of the page
 * ("granted", "prompt" (it asks, and the person says yes) or "denied"), or "absent" for a browser without it. Its one
 * keyboard plays `pitch` (middle C) every half second, held a fifth of a second, to whatever listens, its messages
 * stamped where they arrived, or 0 when `stamped` is false, as Chromium stamps a real keyboard's.
 */
function fakeMidi({ state, stamped = true, slow = 0, pitch = 60 }) {
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
    for (; t0 + next * 500 <= now; next++) {
      due.push([t0 + next * 500, [0x90, pitch, 100]], [t0 + next * 500 + 200, [0x80, pitch, 0]]);
    }
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

  test("a take kept goes to claudius, plays in the song, and holds what the microphone heard", async () => {
    test.setTimeout(180_000);
    const c = await claudius();
    const { page, close } = await browser(c.base);
    try {
      const lines = said(page);
      const plan = "wait:3;record;wait:2;rec:start;wait:7;rec:stop;wait:2;rec:keep;wait:5";
      await page.goto(`${c.base}/?plan=${encodeURIComponent(plan)}`);
      await expect.poll(() => lines.join("\n"), { timeout: 60_000 }).toMatch(/take1: a take of [\d.]+ bars from the start of intro/);
      expect(lines.join("\n")).not.toMatch(/dropped audio|went silent|assistant/);
      // the recorder hands over the last of what it heard when it is told to stop, long before the window gives up
      // waiting for it; the take waits to be kept, and goes once it is
      const asked = lines.when["finishing ..."];
      const held = lines.find((l) => / s recorded: play it back, record it again, or keep it$/.test(l));
      expect(asked, "the take waited for the last of what was heard").toBeTruthy();
      expect(held, "the take waited to be kept").toBeTruthy();
      expect(lines.when[held] - asked).toBeLessThan(800);
      expect(lines.indexOf(lines.find((l) => l.startsWith("sending ")))).toBeGreaterThan(lines.indexOf(held));
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

  test("a take an assistant asks for opens the window for it, plays back in the song, and goes to the assistant", async () => {
    test.setTimeout(180_000);
    const c = await claudius();
    const { page, close } = await browser(c.base);
    try {
      const ask = { cmd: "request_take", from: "intro", role: "vocal", name: "hook", note: "the hook, softly" };
      const asked = await (await c.api("/api/command", ask)).json();
      expect(asked.ok, asked.message).toBe(true);
      const lines = said(page);
      // no `record` step: the link the assistant gives opens the window, set up for the take it asks for
      const plan = "wait:4;rec:start;wait:7;rec:stop;wait:2;rec:play;wait:2;rec:keep;wait:5";
      await page.goto(`${c.base}/?record=1&plan=${encodeURIComponent(plan)}`);
      await expect.poll(() => lines.join("\n"), { timeout: 60_000 }).toMatch(/^hook: a take of [\d.]+ bars from the start of intro.* \(the assistant has it now\)$/m);
      const all = lines.join("\n");
      expect(all).toMatch(/ s recorded: play it back, record it again, or send it$/m);
      expect(all, "the take played back in the song").toMatch(/^playing it back: 0:0[1-9] of 0:\d\d$/m);
      const state = await (await c.api("/api/state")).json();
      expect(state.take_request).toMatchObject({ id: 1, status: "taken", from: "intro", note: "the hook, softly" });
      expect(state.take_request.answer).toMatch(/^hook: a take of/);
      expect(state.song.tracks.find((t) => t.name === "hook")?.kind).toBe("take");
    } finally {
      await close();
      c.stop();
    }
  });

  test("a take an assistant asks for, turned down, says so to the assistant", async () => {
    test.setTimeout(120_000);
    const c = await claudius();
    const { page, close } = await browser(c.base);
    try {
      expect((await (await c.api("/api/command", { cmd: "request_take", from: "intro" })).json()).ok).toBe(true);
      const lines = said(page);
      await page.goto(`${c.base}/?record=1&plan=${encodeURIComponent("wait:4;rec:not-now;wait:3")}`);
      const status = async () => (await (await c.api("/api/state")).json()).take_request?.status;
      await expect.poll(status, { timeout: 30_000 }).toBe("cancelled");
      expect(lines.join("\n")).toMatch(/the assistant hears that you will not record it now/);
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

  test("drum pads on MIDI play the song's lanes, by the General MIDI drum map", async () => {
    test.setTimeout(180_000);
    const c = await claudius();
    const { page, close } = await browser(c.base);
    try {
      // a pad struck every half second, the General MIDI kick: a kick on every beat at 120 BPM
      await page.addInitScript(fakeMidi, { state: "granted", pitch: 36 });
      const lines = said(page);
      const plan = "wait:3;record;rec:midi:Fake Keyboard;rec:part:drums;wait:3;rec:start;wait:7;rec:stop;wait:4";
      await page.goto(`${c.base}/?plan=${encodeURIComponent(plan)}`);
      await expect.poll(() => lines.join("\n"), { timeout: 60_000 }).toMatch(/drums played from intro, \d bars? as their main patterns: \d+ steps into kick/);
      const state = await (await c.api("/api/state")).json();
      const kick = state.song.drums.find((d) => d.lane === "kick").variants.find(([n]) => n === "main")[1];
      const steps = kick.slice(0, 16).map((v, i) => (v > 0 ? i : -1)).filter((i) => i >= 0);
      expect(steps.length, `a kick on each beat of the first bar: ${steps}`).toBe(4);
      expect(steps.every((s, i) => i === 0 || s - steps[i - 1] === 4), `a beat apart: ${steps}`).toBe(true);
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

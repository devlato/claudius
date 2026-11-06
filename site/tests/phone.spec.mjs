// Playing from a phone, end to end: a claudius serves the studio and the phone's page, the studio (a scripted plan)
// opens Play from a phone and plays the song, and a phone-sized page follows it, plays two bars on its pads in time and
// sends them, which become the song's drums. It needs what claudius.mjs says:
//
//   cargo build --bin claudius && cargo xtask web --plan
//   npx playwright test tests/phone.spec.mjs      (in site/)
import { test, expect, chromium, devices } from "@playwright/test";
import { join } from "node:path";
import { claudius, playTwoBars, ready, tone, why } from "./claudius.mjs";

/** a MIDI controller, before the page's own scripts: `window.sendMidi(...bytes)` plays a message on it */
function fakeController() {
  const input = { id: "pads", name: "Fake Pads", type: "input", state: "connected", connection: "open", onmidimessage: null };
  const access = { inputs: new Map([["pads", input]]), outputs: new Map(), sysexEnabled: false, onstatechange: null };
  Navigator.prototype.requestMIDIAccess = async () => access;
  window.sendMidi = (...bytes) => input.onmidimessage?.({ data: new Uint8Array(bytes), timeStamp: performance.now() });
}

test.describe("playing from a phone", () => {
  test.skip(!ready, why);

  test("pads played along with the studio become the song's drums", async () => {
    test.setTimeout(240_000);
    const c = await claudius();
    const browser = await chromium.launch({ args: ["--autoplay-policy=no-user-gesture-required"] });
    try {
      const studio = await browser.newPage();
      await studio.goto(`${c.base}/?plan=${encodeURIComponent("wait:2;phone;wait:2;play;wait:200")}`);
      const state = async () => (await c.api("/api/state")).json();
      await expect.poll(async () => (await state()).phone?.url ?? "", { timeout: 60_000 }).toMatch(/\/\?key=[0-9a-f]{32}$/);
      const { url } = (await state()).phone;
      // the key opens the page, and only the key
      const shut = await browser.newPage();
      expect((await shut.goto(url.replace(/key=.*/, "key=0"))).status()).toBe(403);
      await shut.close();
      const phone = await browser.newPage({ ...devices["iPhone 13"] });
      await phone.goto(url);
      await phone.click("#go");
      await phone.waitForFunction(() => pads.length > 0);
      // taps are taps: the page gives no double tap a zoom
      expect(await phone.evaluate(() => getComputedStyle(document.documentElement).touchAction)).toBe("manipulation");
      await expect(phone.locator("#where"), "it follows the studio as it plays").toContainText(/, bar \d+/, { timeout: 60_000 });
      await expect.poll(async () => (await state()).phone.phones, { message: "the studio sees the phone" }).toBe(1);
      await phone.waitForFunction(() => hits.size === song.lanes.length, null, { timeout: 60_000 });
      const { said, low, high } = await phone.evaluate(playTwoBars);
      expect(said).toMatch(/^drums played from \S+, 2 bars as their main patterns: /);
      expect(said, "struck the one time the take passed each step").not.toContain("by chance");
      const song = (await state()).song;
      const steps = (lane) => {
        const v = song.drums.find((d) => d.lane === lane).variants.find(([n]) => n === "main")[1];
        return v.slice(0, 32).map((x, i) => (x > 0 ? i : -1)).filter((i) => i >= 0);
      };
      expect(steps(low), `${low} on every other beat`).toEqual([0, 8, 16, 24]);
      expect(steps(high), `${high} on the others`).toEqual([4, 12, 20, 28]);
    } finally {
      await browser.close();
      c.stop();
    }
  });

  test("the phone plays and stops the song in the studio, and a take started there starts it", async () => {
    test.setTimeout(240_000);
    const c = await claudius();
    const browser = await chromium.launch({ args: ["--autoplay-policy=no-user-gesture-required"] });
    try {
      // the studio serves the phone's page and does not play
      const studio = await browser.newPage();
      await studio.goto(`${c.base}/?plan=${encodeURIComponent("wait:2;phone;wait:220")}`);
      const state = async () => (await c.api("/api/state")).json();
      await expect.poll(async () => (await state()).phone?.url ?? "", { timeout: 60_000 }).toMatch(/\/\?key=/);
      await expect.poll(async () => (await state()).render?.version ?? 0, { timeout: 120_000 }).toBeGreaterThan(0);
      const phone = await browser.newPage({ ...devices["iPhone 13"] });
      await phone.goto((await state()).phone.url);
      await phone.click("#go");
      await phone.waitForFunction(() => hits.size === song.lanes.length, null, { timeout: 60_000 });
      await expect(phone.locator("#where")).toContainText("stopped");
      await phone.click("#play");
      await expect(phone.locator("#where"), "the studio plays it").toContainText(/, bar \d+/, { timeout: 30_000 });
      await phone.click("#play");
      await expect(phone.locator("#where"), "and stops it").toContainText("stopped", { timeout: 30_000 });
      // ● with the song stopped starts it a bar before its section, and records from the section's first beat
      await phone.click("#rec");
      await expect(phone.locator("#said")).toHaveText(/^recording: /, { timeout: 30_000 });
      const said = await phone.evaluate(async () => {
        const wait = (ms) => new Promise((r) => setTimeout(r, Math.max(0, ms)));
        const beat = 60 / song.bpm;
        const from = take.from;
        for (let b = 0; b < 4; b++) {
          await wait((from + b * beat - songAt(performance.now() + latency())) * 1000);
          hit(song.lanes[0], 1, performance.now());
        }
        await wait((from + 4 * beat + 0.05 - songAt(performance.now() + latency())) * 1000);
        record();
        const text = () => document.getElementById("said").textContent;
        for (let i = 0; i < 100 && !/played|nothing|cannot|no /.test(text()); i++) await wait(100);
        return text();
      });
      expect(said).toMatch(/^drums played from \S+/);
      // a section's own start finds that section at any tempo, though its seconds come back a hair short of it
      const found = await phone.evaluate(() => {
        const keep = song.bpm;
        try {
          return [91.5, 97, 87.3, 123.47].every((bpm) => {
            song.bpm = bpm;
            return song.sections.every((s) => sectionAt((s.start_bar * 240) / bpm) === s);
          });
        } finally {
          song.bpm = keep;
        }
      });
      expect(found, "each section found from its own start").toBe(true);
    } finally {
      await browser.close();
      c.stop();
    }
  });

  test("the turntable's record spins in time, and a finger brakes it, grabs it and swipes it as a DJ's does", async () => {
    test.setTimeout(240_000);
    const c = await claudius();
    const browser = await chromium.launch({ args: ["--autoplay-policy=no-user-gesture-required"] });
    try {
      const studio = await browser.newPage();
      await studio.goto(`${c.base}/?plan=${encodeURIComponent("wait:2;phone;wait:220")}`);
      const state = async () => (await c.api("/api/state")).json();
      await expect.poll(async () => (await state()).phone?.url ?? "", { timeout: 60_000 }).toMatch(/\/\?key=/);
      const phone = await browser.newPage({ ...devices["iPhone 13"] });
      await phone.goto((await state()).phone.url);
      await phone.click("#go");
      await phone.click("nav button[data-mode=deck]");
      await phone.waitForFunction(() => deck.buf !== null, null, { timeout: 60_000 });
      // its records: samples, voices, parts, beats and the whole mix, never a drum lane alone
      const offered = await phone.evaluate(() => [...$("track").options].map((o) => o.value));
      const lanes = await phone.evaluate(() => song.lanes);
      expect(offered.filter((r) => lanes.includes(r)), "no lone drum").toEqual([]);
      expect(offered).toContain("*");
      const speed = (wait) => phone.evaluate((ms) => new Promise((r) => setTimeout(() => r(deck.vel), ms)), wait);
      // with the motor on, in time with the song; the pitch fader moves it, and "in time" brings it back
      await phone.click("#motor");
      expect(await speed(400)).toBeCloseTo(1, 1);
      await phone.evaluate(() => ($("pitch").value = "8"));
      expect(await speed(400)).toBeCloseTo(1.08, 2);
      await expect(phone.locator("#speed")).toContainText("+8.0%");
      await phone.click("#sync");
      expect(await speed(400)).toBeCloseTo(1, 1);
      // a finger on the record, at its edge: touched, moved round it, let go
      const touch = (type, { turn = 0, size = 0, at = 0.4 } = {}) =>
        phone.evaluate(
          ([type, turn, size, at]) => {
            const c = $("platter");
            const r = c.getBoundingClientRect();
            const rad = Math.min(r.width, r.height) * at;
            const a = -Math.PI / 2 + turn;
            const init = { pointerId: 7, pointerType: "touch", isPrimary: true, bubbles: true, pressure: 0.5 };
            const xy = { clientX: r.left + r.width / 2 + rad * Math.cos(a), clientY: r.top + r.height / 2 + rad * Math.sin(a) };
            c.dispatchEvent(new PointerEvent(type, { ...init, ...xy, width: size || 1, height: size || 1 }));
          },
          [type, turn, size, at],
        );
      // a screen that tells nothing of the touch: a resting finger slows the record, then stops it
      await touch("pointerdown");
      const slowing = await speed(150);
      expect(slowing).toBeLessThan(0.9);
      expect(slowing).toBeGreaterThan(0.05);
      expect(await speed(600)).toBeLessThan(0.05);
      await touch("pointerup");
      expect(await speed(300)).toBeCloseTo(1, 1);
      // a firmer press shifts the fingertip's middle a pixel or two, which near the label is more of a turn than at the
      // edge: the finger still rests on the record and brakes it, rather than grabbing it
      await touch("pointerdown", { at: 0.12 });
      for (let i = 1; i <= 4; i++) await touch("pointermove", { at: 0.12, turn: i * 0.02 });
      expect(await phone.evaluate(() => hold.grab), "a press is no grab").toBe(false);
      await touch("pointerup", { at: 0.12 });
      expect(await speed(300)).toBeCloseTo(1, 1);
      // one that says how large: a light touch drags a little, a firm one stops it
      await touch("pointerdown", { size: 40 });
      await touch("pointerup", { size: 40 });
      await touch("pointerdown", { size: 12 });
      const light = await speed(400);
      expect(light).toBeGreaterThan(0.6);
      expect(light).toBeLessThan(0.9);
      await touch("pointerup", { size: 12 });
      await touch("pointerdown", { size: 40 });
      expect(await speed(400)).toBeLessThan(0.1);
      await touch("pointerup", { size: 40 });
      expect(await speed(300)).toBeCloseTo(1, 1);
      // a swipe: the finger grabs, carries the record round fast, and lets go; it runs on faster, then back in time
      await touch("pointerdown");
      for (let i = 1; i <= 8; i++) await touch("pointermove", { turn: i * 0.25 });
      await touch("pointerup", { turn: 2 });
      const flung = await speed(20);
      expect(flung, "a swipe spins it faster").toBeGreaterThan(1.3);
      expect(flung, "as fast as a finger turns one").toBeLessThanOrEqual(4);
      expect(await speed(900)).toBeCloseTo(1, 1);
      // a swipe backwards spins it back a moment, as the slipmat slows it, and then the platter carries it on in time
      await touch("pointerdown", { turn: 2 });
      for (let i = 1; i <= 8; i++) await touch("pointermove", { turn: 2 - i * 0.25 });
      await touch("pointerup");
      expect(await speed(150), "a spin back").toBeLessThan(-0.5);
      expect(await speed(1500)).toBeCloseTo(1, 1);
      // the motor off, the brake stops the platter, and the record with it, in about a third of a second
      await phone.click("#motor");
      expect(await speed(150)).toBeGreaterThan(0.2);
      expect(await speed(400)).toBe(0);
    } finally {
      await browser.close();
      c.stop();
    }
  });

  test("a sample from the library goes on the turntable, at the speed that plays it in time with the song", async () => {
    test.setTimeout(240_000);
    const c = await claudius({ samples: [{ name: "break", role: "break", bpm: 100, beats: 8, title: "Funky Break" }] });
    const browser = await chromium.launch({ args: ["--autoplay-policy=no-user-gesture-required"] });
    try {
      const studio = await browser.newPage();
      await studio.goto(`${c.base}/?plan=${encodeURIComponent("wait:2;phone;wait:220")}`);
      const state = async () => (await c.api("/api/state")).json();
      await expect.poll(async () => (await state()).phone?.url ?? "", { timeout: 60_000 }).toMatch(/\/\?key=/);
      const phone = await browser.newPage({ ...devices["iPhone 13"] });
      await phone.goto((await state()).phone.url);
      await phone.click("#go");
      await phone.click("nav button[data-mode=deck]");
      await phone.waitForFunction(() => library.length > 0, null, { timeout: 60_000 });
      // in a group of its own, by its recording's title and its tempo
      const sample = await phone.evaluate(() => library[0]);
      const groups = await phone.evaluate(() => [...$("track").querySelectorAll("optgroup")].map((g) => g.label));
      expect(groups.at(-1)).toBe("From the library");
      await expect(phone.locator(`#track option[value="${sample.track}"]`)).toHaveText("Funky Break · 100 BPM");
      await phone.selectOption("#track", sample.track);
      await phone.waitForFunction((t) => deck.track === t && deck.buf !== null, sample.track, { timeout: 60_000 });
      expect(await phone.evaluate(() => $("from").disabled), "a sample plays from its start").toBe(true);
      expect(await phone.evaluate(() => deck.buf.duration), "two bars at its 100 BPM").toBeCloseTo(4.8, 1);
      // with the motor on it turns at the song's tempo over its own, by octaves the nearest
      const bpm = await phone.evaluate(() => song.bpm);
      let want = bpm / 100;
      while (want > Math.SQRT2) want /= 2;
      while (want < Math.SQRT1_2) want *= 2;
      expect(sample.speed).toBeCloseTo(want, 6);
      await phone.click("#motor");
      const speed = await phone.evaluate(() => new Promise((r) => setTimeout(() => r(deck.vel), 600)));
      expect(speed).toBeCloseTo(want, 2);
    } finally {
      await browser.close();
      c.stop();
    }
  });

  test("a record recorded off the microphone goes on the turntable, and a scratch on it is a take", async () => {
    test.setTimeout(240_000);
    const c = await claudius();
    const microphone = join(c.home, "microphone.wav");
    tone(microphone, 3);
    const args = ["--autoplay-policy=no-user-gesture-required", "--use-fake-device-for-media-stream"];
    args.push("--use-fake-ui-for-media-stream", `--use-file-for-fake-audio-capture=${microphone}`);
    const browser = await chromium.launch({ args });
    try {
      const studio = await browser.newPage();
      await studio.goto(`${c.base}/?plan=${encodeURIComponent("wait:2;phone;wait:220")}`);
      const state = async () => (await c.api("/api/state")).json();
      await expect.poll(async () => (await state()).phone?.url ?? "", { timeout: 60_000 }).toMatch(/\/\?key=/);
      const url = new URL((await state()).phone.url);
      const deckOf = async (address, permissions) => {
        const page = await (await browser.newContext({ ...devices["iPhone 13"], permissions })).newPage();
        await page.goto(address);
        await page.click("#go");
        await page.click("nav button[data-mode=deck]");
        await page.waitForFunction(() => deck.buf !== null, null, { timeout: 60_000 });
        return page;
      };
      // the page on the computer itself, which a browser trusts with a microphone, as it trusts the demo's
      const phone = await deckOf(`http://127.0.0.1:${url.port}/${url.search}`, ["microphone"]);
      await phone.click("#mic");
      await phone.waitForTimeout(1500);
      await phone.click("#mic");
      await phone.waitForFunction(() => deck.track === "mic" && deck.buf !== null, null, { timeout: 10_000 });
      await expect(phone.locator('#track option[value="mic"]')).toHaveText("your recording");
      const secs = await phone.evaluate(() => deck.buf.duration);
      expect(secs).toBeGreaterThan(1);
      expect(secs).toBeLessThan(2.5);
      const peak = await phone.evaluate(() => deck.buf.getChannelData(0).reduce((m, v) => Math.max(m, Math.abs(v)), 0));
      expect(peak, "its peak at the master's ceiling").toBeCloseTo(10 ** (-1 / 20), 2);
      // a scratch on it, sent as the take sends it: the recording goes along, and the studio makes the take
      const reply = await phone.evaluate(async () => {
        const moves = [{ t: 0, at: 0, fader: 1 }, { t: 0.4, at: 0.3, fader: 1 }, { t: 0.8, at: 0, fader: 1 }];
        const take = { cmd: "add_scratch", from: song.sections[0].name, track: "mic", bars: 1, moves, recording: mic.wav };
        return (await link.post("api/take", take)).json();
      });
      expect(reply.ok, reply.message).toBe(true);
      await expect.poll(async () => (await state()).song.tracks.some((t) => t.name === "scratch1")).toBe(true);
      // the page as a phone opens it, on the network: the browser lets it use no microphone, and it says where to record
      const lan = await deckOf(url.href, []);
      expect(await lan.evaluate(() => window.isSecureContext)).toBe(false);
      await lan.click("#mic");
      await expect(lan.locator("#said")).toContainText("Record window");
    } finally {
      await browser.close();
      c.stop();
    }
  });

  test("a MIDI controller plays the pads by its notes and scratches with its jog wheel", async () => {
    test.setTimeout(240_000);
    const c = await claudius();
    const browser = await chromium.launch({ args: ["--autoplay-policy=no-user-gesture-required"] });
    try {
      const studio = await browser.newPage();
      await studio.goto(`${c.base}/?plan=${encodeURIComponent("wait:2;phone;wait:2;play;wait:200")}`);
      const state = async () => (await c.api("/api/state")).json();
      await expect.poll(async () => (await state()).phone?.url ?? "", { timeout: 60_000 }).toMatch(/\/\?key=/);
      const phone = await browser.newPage({ ...devices["iPhone 13"] });
      await phone.addInitScript(fakeController);
      await phone.goto((await state()).phone.url);
      await phone.click("#go");
      await expect(phone.locator("#where")).toContainText(/, bar \d+/, { timeout: 60_000 });
      await phone.waitForFunction(() => hits.size === song.lanes.length, null, { timeout: 60_000 });
      const { said } = await phone.evaluate(playTwoBars, true);
      expect(said).toMatch(/^drums played from \S+, 2 bars as their main patterns: /);
      const song = (await state()).song;
      const main = (lane) => song.drums.find((d) => d.lane === lane).variants.find(([n]) => n === "main")[1];
      const on = (lane) => main(lane).slice(0, 32).map((x, i) => (x > 0 ? i : -1)).filter((i) => i >= 0);
      const [low, high] = song.drums.map((d) => d.lane);
      expect([on(low), on(high)], "struck by the drum map's notes").toEqual([
        [0, 8, 16, 24],
        [4, 12, 20, 28],
      ]);
      // a key held as the screen changes is let go of all the same
      await phone.click("nav button[data-mode=keys]");
      await phone.waitForFunction(() => keys.buf !== null, null, { timeout: 60_000 });
      await phone.evaluate(() => window.sendMidi(0x90, 60, 100));
      expect(await phone.evaluate(() => held.size)).toBe(1);
      await phone.click("nav button[data-mode=pads]");
      await phone.evaluate(() => window.sendMidi(0x80, 60, 0));
      expect(await phone.evaluate(() => held.size), "the key sounds no more").toBe(0);
      // the turntable: the wheel and the fader learnt, then turning the record and cutting it
      await phone.click("nav button[data-mode=deck]");
      await phone.waitForFunction(() => deck.buf !== null, null, { timeout: 60_000 });
      await expect(phone.locator("#learn")).toHaveText("Learn MIDI");
      await phone.click("#learn");
      await phone.evaluate(() => window.sendMidi(0xb0, 33, 65));
      await expect(phone.locator("#learn")).toHaveText("move the crossfader");
      await phone.evaluate(() => window.sendMidi(0xb0, 8, 100));
      await expect(phone.locator("#learn")).toHaveText("learnt");
      const before = await phone.evaluate(() => deck.pos);
      await phone.evaluate(async () => {
        for (let i = 0; i < 16; i++) {
          window.sendMidi(0xb0, 33, 72);
          await new Promise((r) => setTimeout(r, 20));
        }
        window.sendMidi(0xb0, 8, 0);
      });
      // sixteen ticks of 8 a turn of 128: a turn, 1.8 s of the record
      await expect.poll(() => phone.evaluate(() => deck.pos)).toBeGreaterThan(before + 1.5);
      expect(await phone.inputValue("#fader")).toBe("0");
      expect(await phone.evaluate(() => JSON.parse(localStorage.getItem("claudius-phone-midi")).jog.cc)).toBe(33);
    } finally {
      await browser.close();
      c.stop();
    }
  });
});

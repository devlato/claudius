// Playing from a phone, end to end: a claudius serves the studio and the phone's page, the studio (a scripted plan)
// opens Play from a phone and plays the song, and a phone-sized page follows it, plays two bars on its pads in time and
// sends them, which become the song's drums. It needs what claudius.mjs says:
//
//   cargo build --bin claudius && cargo xtask web --plan
//   npx playwright test tests/phone.spec.mjs      (in site/)
import { test, expect, chromium, devices } from "@playwright/test";
import { claudius, playTwoBars, ready, why } from "./claudius.mjs";

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
      await expect(phone.locator(".pad")).not.toHaveCount(0);
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

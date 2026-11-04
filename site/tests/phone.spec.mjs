// Playing from a phone, end to end: a claudius serves the studio and the phone's page, the studio (a scripted plan)
// opens Play from a phone and plays the song, and a phone-sized page follows it, plays two bars on its pads in time and
// sends them, which become the song's drums. It needs what claudius.mjs says:
//
//   cargo build --bin claudius && cargo xtask web --plan
//   npx playwright test tests/phone.spec.mjs      (in site/)
import { test, expect, chromium, devices } from "@playwright/test";
import { claudius, ready, why } from "./claudius.mjs";

/**
 * On the phone's page: a take started on the bar after next, two bars of a beat struck on its pads in time with the
 * song (the first lane on every other beat, the second on the others), and the take sent. Returns what the page says.
 */
function playTwoBars() {
  const wait = (ms) => new Promise((r) => setTimeout(r, Math.max(0, ms)));
  const [low, high] = song.lanes;
  const pad = (lane) => [...document.querySelectorAll(".pad")].find((p) => p.textContent === lane);
  const strike = (lane) => {
    const r = pad(lane).getBoundingClientRect();
    pad(lane).dispatchEvent(new PointerEvent("pointerdown", { bubbles: true, clientX: r.left + 10, clientY: r.top + 10 }));
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
});

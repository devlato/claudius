// Playing from a phone along with the studio demo, end to end: the demo (the studio built with its plan feature, run as
// `?mock`) joins a room of the site's relay, as the site's test server runs it (rooms.mjs), and plays its song; a
// phone-sized page opens the phone's page from the address the demo shows, plays two bars on its pads in time with the
// song and sends them, and the demo's song gets them. (How a take folds, chances and all, phone.spec.mjs and the
// engine's tests check: here the demo renders in the same browser, and a busy run makes the script stop late.) It needs
// the studio built with its plan feature:
//
//   cargo xtask web --plan
//   npx playwright test tests/demo-phone.spec.mjs      (in site/)
import { test, expect, chromium, devices } from "@playwright/test";
import { spawn } from "node:child_process";
import { existsSync, mkdtempSync, rmSync, symlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { freePort, heard, playTwoBars, repo, studio } from "./claudius.mjs";

const ready = existsSync(join(studio, "index.html"));
const why = `needs the studio built with its plan feature in ${studio} (cargo xtask web --plan)`;

test.describe("a phone with the demo", () => {
  test.skip(!ready, why);

  test("pads played along with the demo, through the relay, become its song's drums", async () => {
    test.setTimeout(480_000);
    // the site as Cloudflare serves it: the demo, the phone's page, and the relay, from a server of its own (its
    // stand-in for the edge's cache stays out of this process)
    const root = mkdtempSync(join(tmpdir(), "claudius-demo-phone-"));
    symlinkSync(studio, join(root, "studio"));
    symlinkSync(join(repo, "crates", "claudius-server", "src", "phone"), join(root, "phone"));
    const port = await freePort();
    const origin = `http://127.0.0.1:${port}`;
    const server = spawn(process.execPath, [join(import.meta.dirname, "serve.mjs"), "--port", String(port), "--root", root]);
    await expect.poll(() => fetch(`${origin}/phone/`).then((r) => r.status, () => 0), { timeout: 30_000 }).toBe(200);
    const browser = await chromium.launch({ args: ["--autoplay-policy=no-user-gesture-required"] });
    try {
      const demo = await browser.newPage();
      const [page, phones, toasts] = ["phone", "phones", "toast"].map((t) => heard(demo, t));
      await demo.goto(`${origin}/studio/?mock&plan=${encodeURIComponent("wait:2;phone;wait:40;play;wait:280")}`);
      await expect.poll(() => page.at(-1) ?? "", { timeout: 60_000 }).toMatch(/\/phone\/\?room=[0-9a-f]{32}$/);
      const phone = await browser.newPage({ ...devices["iPhone 13"] });
      await phone.goto(page.at(-1));
      await phone.click("#go");
      await expect(phone.locator(".pad")).not.toHaveCount(0, { timeout: 60_000 });
      await expect.poll(() => phones.at(-1), { message: "the demo sees the phone" }).toBe("1");
      await expect(phone.locator("#where"), "it follows the demo as it plays").toContainText(/, bar \d+/, { timeout: 120_000 });
      // the demo's engine renders the song first, then the phone's sounds
      await phone.waitForFunction(() => hits.size === song.lanes.length, null, { timeout: 180_000 });
      const said = phone.locator("#said");
      // two bars on the pads, again if the demo stopped for a moment as the take began (its whole song taking the place
      // of its first bars)
      const take = async () => {
        for (let i = 0; i < 3; i++) {
          await phone.waitForFunction(() => transport.playing, null, { timeout: 60_000 });
          await phone.evaluate(playTwoBars);
          if (!/play the song in the studio first/.test(await said.textContent())) return;
        }
      };
      await take();
      // the take waits for the engine to finish what it was rendering
      await expect(said).toHaveText(/^drums played from \S+, 2 bars as their main patterns: /, { timeout: 150_000 });
      await expect.poll(() => toasts.join("\n"), { message: "the demo says so" }).toMatch(/from the phone: drums played/);
      // the demo reloaded keeps its room: the phone plays on with it once it is back
      await demo.reload();
      await phone.waitForFunction(() => hits.size === song.lanes.length && transport.playing, null, { timeout: 180_000 });
      await take();
      await expect(said).toHaveText(/^drums played from \S+, 2 bars as their main patterns: /, { timeout: 150_000 });
      // what was asked while the demo was away fails long after it is back, and says nothing of it
      await phone.waitForTimeout(6000);
      await expect(said).toHaveText(/^drums played from /);
      // the demo opened again in another tab (a duplicate keeps the tab's room) takes the room, and the first lets it
      const room = new URL(page.at(-1)).searchParams.get("room");
      const again = await browser.newPage();
      await again.addInitScript((r) => sessionStorage.setItem("claudius-phone-room", r), room);
      const told = heard(again, "toast");
      await again.goto(`${origin}/studio/?mock&plan=${encodeURIComponent("wait:2;phone;wait:40;play;wait:280")}`);
      await expect.poll(() => toasts.join("\n"), { timeout: 60_000 }).toMatch(/opened in another tab/);
      await phone.waitForFunction(() => hits.size === song.lanes.length && transport.playing, null, { timeout: 180_000 });
      await take();
      await expect(said).toHaveText(/^drums played from \S+, 2 bars as their main patterns: /, { timeout: 150_000 });
      await expect.poll(() => told.join("\n"), { message: "the tab that took the room got it" }).toMatch(/from the phone: drums played/);
      // the demo closed: the phone says so at once, rather than once what it asked has waited its time
      await again.close();
      await demo.close();
      await expect(said).toContainText("not open", { timeout: 15_000 });
    } finally {
      await browser.close();
      server.kill();
      rmSync(root, { recursive: true, force: true });
    }
  });
});

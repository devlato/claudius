// Playing from a phone along with the studio demo, end to end: the demo (the studio built with its plan feature, run as
// `?mock`) joins a room of the site's relay, as the site's test server runs it (rooms.mjs), and plays its song; a
// phone-sized page opens the phone's page from the address the demo shows, plays two bars on its pads in time with the
// song and sends them, and the demo's song gets them. It needs the studio built with its plan feature:
//
//   cargo xtask web --plan
//   npx playwright test tests/demo-phone.spec.mjs      (in site/)
import { test, expect, chromium, devices } from "@playwright/test";
import { existsSync, mkdtempSync, rmSync, symlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { freePort, heard, playTwoBars, repo, studio } from "./claudius.mjs";
import { start } from "./serve.mjs";

const ready = existsSync(join(studio, "index.html"));
const why = `needs the studio built with its plan feature in ${studio} (cargo xtask web --plan)`;

test.describe("a phone with the demo", () => {
  test.skip(!ready, why);

  test("pads played along with the demo, through the relay, become its song's drums", async () => {
    test.setTimeout(300_000);
    // the site as Cloudflare serves it: the demo, the phone's page, and the relay
    const root = mkdtempSync(join(tmpdir(), "claudius-demo-phone-"));
    symlinkSync(studio, join(root, "studio"));
    symlinkSync(join(repo, "crates", "claudius-server", "src", "phone"), join(root, "phone"));
    const { server, origin } = await start({ port: await freePort(), root });
    const browser = await chromium.launch({ args: ["--autoplay-policy=no-user-gesture-required"] });
    try {
      const demo = await browser.newPage();
      const [page, phones, toasts] = ["phone", "phones", "toast"].map((t) => heard(demo, t));
      await demo.goto(`${origin}/studio/?mock&plan=${encodeURIComponent("wait:2;phone;wait:4;play;wait:280")}`);
      await expect.poll(() => page.at(-1) ?? "", { timeout: 60_000 }).toMatch(/\/phone\/\?room=[0-9a-f]{32}$/);
      const phone = await browser.newPage({ ...devices["iPhone 13"] });
      await phone.goto(page.at(-1));
      await phone.click("#go");
      await expect(phone.locator(".pad")).not.toHaveCount(0, { timeout: 60_000 });
      await expect.poll(() => phones.at(-1), { message: "the demo sees the phone" }).toBe("1");
      await expect(phone.locator("#where"), "it follows the demo as it plays").toContainText(/, bar \d+/, { timeout: 60_000 });
      // the demo's engine renders the song first, then the phone's sounds
      await phone.waitForFunction(() => hits.size === song.lanes.length, null, { timeout: 180_000 });
      await phone.evaluate(playTwoBars);
      // the take waits for the engine to finish what it was rendering
      const said = phone.locator("#said");
      await expect(said).toHaveText(/^drums played from \S+, 2 bars as their main patterns: /, { timeout: 150_000 });
      await expect(said, "struck the one time the take passed each step").not.toContainText("by chance");
      await expect.poll(() => toasts.join("\n"), { message: "the demo says so" }).toMatch(/from the phone: drums played/);
    } finally {
      await browser.close();
      server.close();
      rmSync(root, { recursive: true, force: true });
    }
  });
});

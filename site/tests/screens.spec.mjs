// Screenshots of the site for review, not a test: SCREENSHOT_DIR=/tmp/claudius-site-screens npx playwright test screens
import { test } from "@playwright/test";

const dir = process.env.SCREENSHOT_DIR;

test.skip(!dir, "set SCREENSHOT_DIR to write screenshots");

async function settle(page) {
  await page.evaluate(async () => {
    for (let y = 0; y < document.documentElement.scrollHeight; y += 400) {
      window.scrollTo(0, y);
      await new Promise((r) => setTimeout(r, 80));
    }
  });
  await page.waitForFunction(() => Array.from(document.images).every((i) => i.complete && i.naturalWidth > 0), null, { timeout: 15_000 });
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.waitForLoadState("networkidle");
  await page.evaluate(() => document.fonts.ready);
  // the install tab a Mac or Linux visitor sees (Playwright's desktop Chrome reports Windows)
  await page.locator("#tab-unix").click();
}

for (const [width, height] of [
  [1440, 900],
  [390, 844],
]) {
  test(`landing page at ${width} px`, async ({ page }) => {
    await page.setViewportSize({ width, height });
    await page.goto("/");
    await settle(page);
    await page.screenshot({ path: `${dir}/landing-${width}.png`, fullPage: true });
    await page.screenshot({ path: `${dir}/landing-${width}-fold.png` });

    // playing: the hero's steps light up and the waveform fills
    await page.locator(".unit [data-play]").click();
    await page.waitForTimeout(2600);
    await page.locator(".unit").screenshot({ path: `${dir}/unit-playing-${width}.png` });
    await page.locator(".unit [data-play]").click();
    await page.locator("#listen").screenshot({ path: `${dir}/listen-${width}.png` });

    await page.goto("/no/such/page");
    await page.screenshot({ path: `${dir}/404-${width}.png` });
  });
}

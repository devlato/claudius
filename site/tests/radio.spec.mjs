// The radio (/radio/): its page under its own policy, and the station on air, a song mixed into the next. It needs the
// assembled site (cargo xtask site), whose /studio/ holds the engine the station runs.
import { test, expect, chromium } from "@playwright/test";
import { siteRoot } from "./serve.mjs";

const site = siteRoot();

test.describe("the radio", () => {
  test.skip(!site.assembled, "needs target/site from cargo xtask site");

  test("loads under its own policy and goes on air, then mixes the next song in", async ({ baseURL }) => {
    test.setTimeout(300_000);
    const b = await chromium.launch({ args: ["--autoplay-policy=no-user-gesture-required"] });
    try {
      const page = await b.newPage({ baseURL });
      const problems = [];
      page.on("console", (m) => {
        if (m.type() === "error" || /Content Security Policy|Refused to|Trusted Type/i.test(m.text())) problems.push(m.text());
      });
      page.on("pageerror", (e) => problems.push(e.message));
      await page.goto("/radio/?soon");
      await expect(page.locator("h1")).toHaveText("claudius radio");
      await page.locator("#tune").click();
      // the first song: written, rendered and playing
      await expect(page.locator("#tune")).toHaveText("Pause", { timeout: 120_000 });
      const first = await page.locator("#now-title").textContent();
      expect(first).not.toBe("Not tuned in");
      await expect(page.locator("#now-meta")).toHaveText(/ BPM · [A-G]/);
      const clock = () => page.locator("#now-time").textContent();
      const t0 = await clock();
      await expect.poll(clock, { timeout: 10_000 }).not.toBe(t0);
      // the next one, made while the first plays and mixed in on a bar line (the first started near its end)
      await expect(page.locator("#played li")).toHaveCount(1, { timeout: 240_000 });
      await expect(page.locator("#played li").first()).toContainText(first);
      await expect(page.locator("#now-title")).not.toHaveText(first);
      expect(problems).toEqual([]);
    } finally {
      await b.close();
    }
  });

  test("keeps to the genres chosen, hands the song on air over as a file, opens it in the studio and skips", async ({ baseURL }) => {
    test.setTimeout(420_000);
    const b = await chromium.launch({ args: ["--autoplay-policy=no-user-gesture-required"] });
    try {
      const page = await b.newPage({ baseURL, acceptDownloads: true });
      const problems = [];
      page.on("pageerror", (e) => problems.push(e.message));
      await page.goto("/radio/?genres=deep_house,house");
      await expect(page.locator("#genres-said")).toHaveText("deep house, house");
      await page.locator("#tune").click();
      await expect(page.locator("#tune")).toHaveText("Pause", { timeout: 120_000 });
      await expect(page.locator("#now-meta")).toHaveText(/^(deep house|house) · /);
      // the studio writes the same song again from what the station asked for
      const open = await page.locator("#open").getAttribute("href");
      expect(open).toMatch(/^\/studio\/\?mock&genre=(deep_house|house)&seed=\d+&bpm=[\d.]+&sampled=0/);
      // the song on air, as an MP3 the engine makes
      const [file] = await Promise.all([page.waitForEvent("download", { timeout: 240_000 }), page.locator("#download").click()]);
      expect(file.suggestedFilename()).toMatch(/\.mp3$/);
      const saved = await file.path();
      expect(saved).toBeTruthy();
      await expect(page.locator("#status")).toContainText("is in your downloads");
      // once the next song is written, Skip brings it in on the next beat, and it is of the genres chosen too
      const first = await page.locator("#now-title").textContent();
      await expect(page.locator("#next")).toContainText(/(mixes|comes) in .* from now/, { timeout: 240_000 });
      await page.locator("#skip").click();
      await expect(page.locator("#now-title")).not.toHaveText(first, { timeout: 5_000 });
      await expect(page.locator("#played li").first()).toContainText(first);
      await expect(page.locator("#now-meta")).toHaveText(/^(deep house|house) · /);
      expect(problems).toEqual([]);
    } finally {
      await b.close();
    }
  });
});

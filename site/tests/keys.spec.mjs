// The studio answers to its keys alone, as someone who does not use a mouse works it: a tab by its number, the window
// of the keys and the New song dialog by theirs, and Escape closes the window on top. Needs the claudius and the studio
// that tests/claudius.mjs says.
import { chromium, expect, test } from "@playwright/test";
import { claudius, heard, ready, why } from "./claudius.mjs";

test.describe("the keyboard", () => {
  test.skip(!ready, why);

  test("the studio answers to its keys alone", async () => {
    test.setTimeout(120_000);
    const c = await claudius();
    const b = await chromium.launch();
    const page = await b.newPage();
    try {
      const tab = heard(page, "tab");
      const windows = heard(page, "windows");
      await page.goto(`${c.base}/?plan=${encodeURIComponent("wait:600")}`);
      await expect.poll(() => tab.at(-1), { timeout: 60_000 }).toBeTruthy();
      // the page's studio has the keyboard once its canvas has the focus, as a click or Tab gives it
      await page.locator("canvas").first().focus();
      await page.keyboard.press("5");
      await expect.poll(() => tab.at(-1), { timeout: 10_000 }).toBe("Mixer");
      await page.keyboard.press("2");
      await expect.poll(() => tab.at(-1), { timeout: 10_000 }).toBe("Parts");
      await page.keyboard.press("Shift+Slash");
      await expect.poll(() => windows.at(-1), { timeout: 10_000 }).toBe("keys");
      // a control Tab gives the keyboard to lets go of it on the first Escape, and the window closes on the next
      await page.keyboard.press("Tab");
      await page.keyboard.press("Escape");
      await page.waitForTimeout(500);
      expect(windows.at(-1)).toBe("keys");
      await page.keyboard.press("Escape");
      await expect.poll(() => windows.at(-1), { timeout: 10_000 }).toBe("none");
      await page.keyboard.press("n");
      await expect.poll(() => windows.at(-1), { timeout: 10_000 }).toBe("new song");
      await page.keyboard.press("Escape");
      await expect.poll(() => windows.at(-1), { timeout: 10_000 }).toBe("none");
    } finally {
      await b.close();
      c.stop();
    }
  });

  test("the palette opens the chat, and Escape closes it", async () => {
    test.setTimeout(120_000);
    const c = await claudius();
    const b = await chromium.launch();
    const page = await b.newPage();
    try {
      const tab = heard(page, "tab");
      const windows = heard(page, "windows");
      await page.goto(`${c.base}/?plan=${encodeURIComponent("wait:600")}`);
      await expect.poll(() => tab.at(-1), { timeout: 60_000 }).toBeTruthy();
      await page.locator("canvas").first().focus();
      await page.keyboard.press("ControlOrMeta+k");
      await page.keyboard.type("chat with");
      await page.keyboard.press("Enter");
      await expect.poll(() => windows.at(-1), { timeout: 10_000 }).toBe("chat");
      await page.keyboard.press("Escape");
      await expect.poll(() => windows.at(-1), { timeout: 10_000 }).toBe("none");
    } finally {
      await b.close();
      c.stop();
    }
  });
});

// The studio demo in the browser: what its engine makes in its Web Worker as a claudius would. It needs what
// claudius.mjs says:
//
//   cargo build --bin claudius && cargo xtask web --plan
//   npx playwright test tests/demo.spec.mjs      (in site/)
import { test, expect, chromium } from "@playwright/test";
import { claudius, heard, ready, why } from "./claudius.mjs";

test.describe("the demo", () => {
  test.skip(!ready, why);

  test("mutates a song a variation at a time, and keeps the best when it beats the song", async () => {
    test.setTimeout(300_000);
    const c = await claudius();
    const b = await chromium.launch({ args: ["--autoplay-policy=no-user-gesture-required"] });
    try {
      const page = await b.newPage();
      const busy = heard(page, "busy");
      const toasts = heard(page, "toast");
      const mutate = JSON.stringify({ cmd: "mutate", count: 2, keep_best: true });
      await page.goto(`${c.base}/?mock&plan=${encodeURIComponent(`wait:3;cmd:${mutate};wait:240`)}`);
      await expect.poll(() => busy.join("\n"), { timeout: 180_000 }).toMatch(/^mutating: 1 of 3 judged$/m);
      // what came of it, once the song and its two variations are judged
      const done = /^(kept .* on the preview\)|no variation beat the song .*: it stays as it was)$/m;
      await expect.poll(() => toasts.join("\n"), { timeout: 240_000 }).toMatch(done);
      expect(busy.join("\n"), "each variation judged").toMatch(/^mutating: 2 of 3 judged$/m);
    } finally {
      await b.close();
      c.stop();
    }
  });
});

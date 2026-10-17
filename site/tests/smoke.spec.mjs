// Smoke test for the website: npm ci && npx playwright test (in site/).
// It serves target/site when `cargo xtask site` has built it, otherwise site/ itself (see tests/serve.mjs).
import { test, expect } from "@playwright/test";
import { gzipSync } from "node:zlib";
import { siteRoot, BUILD_PATHS } from "./serve.mjs";

const site = siteRoot();
const BUDGET = 300 * 1024;

/** collect everything that should not happen on a page */
function watch(page) {
  const problems = [];
  page.on("console", (m) => {
    if (m.type() === "error") problems.push(`console error: ${m.text()}`);
  });
  page.on("pageerror", (e) => problems.push(`page error: ${e.message}`));
  page.on("request", (r) => {
    const u = new URL(r.url());
    if (!["data:", "blob:"].includes(u.protocol) && u.hostname !== "127.0.0.1") {
      problems.push(`request to another site: ${r.url()}`);
    }
  });
  page.on("requestfailed", (r) => {
    const why = (r.failure() && r.failure().errorText) || "";
    // the browser cancels media requests itself when playback pauses or seeks
    if (r.resourceType() === "media" && /ERR_ABORTED/.test(why)) return;
    problems.push(`request failed: ${r.url()} (${why})`);
  });
  page.on("response", (r) => {
    if (r.status() >= 400) problems.push(`HTTP ${r.status()}: ${r.url()}`);
  });
  return problems;
}

async function cspViolations(page) {
  await page.addInitScript(() => {
    window.__csp = [];
    document.addEventListener("securitypolicyviolation", (e) => {
      window.__csp.push(`${e.violatedDirective} blocked ${e.blockedURI || "inline"} in ${e.sourceFile || e.documentURI}:${e.lineNumber}`);
    });
  });
  return () => page.evaluate(() => window.__csp || []);
}

// scroll down the page so lazy images load, and wait for them
async function scrollThrough(page) {
  await page.evaluate(async () => {
    for (let y = 0; y < document.documentElement.scrollHeight; y += 400) {
      window.scrollTo(0, y);
      await new Promise((r) => setTimeout(r, 80));
    }
  });
  await page.waitForFunction(() => Array.from(document.images).every((i) => i.complete && i.naturalWidth > 0), null, { timeout: 15_000 }).catch(() => {});
  await page.evaluate(() => window.scrollTo(0, 0));
}

const time = (audio) => audio.evaluate((a) => a.currentTime);

test.describe("landing page", () => {
  test("loads with no console errors, no failed requests and no requests to other sites", async ({ page }) => {
    const problems = watch(page);
    const violations = await cspViolations(page);
    const res = await page.goto("/");
    expect(res.status()).toBe(200);
    await expect(page.locator("h1")).toHaveText(/listens to its own/);
    await scrollThrough(page);
    await page.waitForLoadState("networkidle");

    const broken = await page.$$eval("img", (imgs) => imgs.filter((i) => !i.complete || i.naturalWidth === 0).map((i) => i.currentSrc || i.src));
    expect(broken, "images that did not load").toEqual([]);
    const unrendered = await page.$$eval("[data-wave]", (waves) => waves.filter((w) => w.classList.contains("is-empty") || w.querySelector("canvas").width === 0).map((w) => w.dataset.wave));
    expect(unrendered, "waveforms without peaks").toEqual([]);
    const placeholders = await page.evaluate(() => document.documentElement.outerHTML.match(/\{\{[a-z_]+\}\}/g));
    expect(placeholders, "unsubstituted placeholders").toBeNull();
    expect(await violations(), "Content-Security-Policy violations").toEqual([]);
    expect(problems).toEqual([]);
  });

  test("stays under the page-weight budget without audio", async ({ page }, info) => {
    const sizes = [];
    page.on("response", async (r) => {
      const type = r.headers()["content-type"] || "";
      if (r.request().resourceType() === "media" || type.startsWith("audio/")) return;
      try {
        const body = await r.body();
        const text = /text|json|svg|javascript|manifest/.test(type);
        sizes.push({ url: r.url(), raw: body.length, sent: text ? gzipSync(body).length : body.length });
      } catch {
        /* redirects have no body */
      }
    });
    await page.goto("/");
    await scrollThrough(page);
    await page.waitForLoadState("networkidle");
    const raw = sizes.reduce((s, x) => s + x.raw, 0);
    const sent = sizes.reduce((s, x) => s + x.sent, 0);
    const kb = (n) => (n / 1024).toFixed(1) + " KB";
    info.annotations.push({ type: "page weight", description: `${kb(raw)} uncompressed, about ${kb(sent)} with gzip, ${sizes.length} files` });
    console.log(`page weight: ${kb(raw)} uncompressed, about ${kb(sent)} with gzip, in ${sizes.length} files`);
    for (const s of sizes.sort((a, b) => b.raw - a.raw)) console.log(`  ${kb(s.raw).padStart(9)}  ${new URL(s.url).pathname}`);
    expect(raw).toBeLessThan(BUDGET);
  });

  test("players play, pause, play one at a time and seek", async ({ page }) => {
    const problems = watch(page);
    await page.goto("/");
    const lofi = page.locator("#paper-lanterns");
    const play = lofi.locator("[data-play]");
    const audio = lofi.locator("audio");

    await play.click();
    await expect(play).toHaveAttribute("aria-pressed", "true");
    await expect.poll(() => time(audio), { message: "playback advances" }).toBeGreaterThan(0.3);
    await expect(lofi.locator(".lcd-status")).toHaveText("PLAY");

    const techno = page.locator("#night-shift [data-play]");
    await techno.click();
    await expect(techno).toHaveAttribute("aria-pressed", "true");
    await expect(play).toHaveAttribute("aria-pressed", "false");
    expect(await audio.evaluate((a) => a.paused)).toBe(true);
    await techno.click();
    await expect(techno).toHaveAttribute("aria-pressed", "false");

    // seeking with the mouse moves the playhead, the readout and the slider value
    const wave = lofi.locator(".wave");
    const box = await wave.boundingBox();
    await wave.click({ position: { x: box.width * 0.75, y: box.height / 2 } });
    await expect.poll(() => time(audio)).toBeGreaterThan(30);
    await expect(wave).toHaveAttribute("aria-valuenow", /^3[0-4]$/);
    await expect(lofi.locator(".lcd-time")).toHaveText(/^0:3\d\.\d$/);

    // and with the keyboard
    await wave.focus();
    await page.keyboard.press("Home");
    await expect.poll(() => time(audio)).toBeLessThan(0.5);
    await page.keyboard.press("ArrowRight");
    await expect.poll(() => time(audio)).toBeGreaterThan(4.5);
    await page.keyboard.press("Space");
    await expect(play).toHaveAttribute("aria-pressed", "true");
    await page.keyboard.press("k");
    await expect(play).toHaveAttribute("aria-pressed", "false");

    // a track that has not loaded yet seeks too
    const dnb = page.locator("#low-light");
    const dnbWave = dnb.locator(".wave");
    await dnbWave.scrollIntoViewIfNeeded();
    const b2 = await dnbWave.boundingBox();
    await dnbWave.click({ position: { x: b2.width * 0.5, y: b2.height / 2 } });
    await expect(dnb.locator(".lcd-time")).toHaveText(/^0:2\d\.\d$/);
    await dnb.locator("[data-play]").click();
    await expect.poll(() => time(dnb.locator("audio"))).toBeGreaterThan(19);
    await dnb.locator("[data-play]").click();

    expect(problems).toEqual([]);
  });

  test("the hero unit plays Golden Pulse and lights its steps", async ({ page }) => {
    const problems = watch(page);
    await page.goto("/");
    const hero = page.locator(".unit [data-play]");
    await hero.click();
    await expect(hero).toHaveAttribute("aria-pressed", "true");
    await expect(page.locator("#golden-pulse [data-play]")).toHaveAttribute("aria-pressed", "true");
    await expect(page.locator(".unit .cell.is-now").first()).toBeVisible();
    await expect(page.locator('.unit .lane[data-role="kick"]')).toHaveClass(/is-out/);
    await hero.click();
    await expect(hero).toHaveAttribute("aria-pressed", "false");
    expect(problems).toEqual([]);
  });

  test("links inside the site resolve", async ({ page, request, baseURL }) => {
    const origin = new URL(baseURL).origin;
    const skipped = new Set();
    const checked = new Set();
    for (const path of ["/", "/404.html"]) {
      await page.goto(path);
      const refs = await page.evaluate(() => [
        ...Array.from(document.querySelectorAll("a[href]"), (a) => a.getAttribute("href")),
        ...Array.from(document.querySelectorAll("link[href]"), (l) => l.getAttribute("href")),
        ...Array.from(document.querySelectorAll("img[src], audio[src]"), (i) => i.getAttribute("src")),
        ...Array.from(document.querySelectorAll("img[srcset]"), (i) => i.getAttribute("srcset").split(",").map((s) => s.trim().split(/\s+/)[0])).flat(),
        ...Array.from(document.querySelectorAll("[data-peaks]"), (w) => w.dataset.peaks),
        ...Array.from(document.querySelectorAll('meta[property="og:image"], meta[name="twitter:image"]'), (m) => new URL(m.content).pathname),
      ]);
      for (const ref of refs) {
        if (ref.startsWith("#")) {
          await expect(page.locator(`[id="${ref.slice(1)}"]`), `anchor ${ref} on ${path}`).toHaveCount(1);
          continue;
        }
        const url = new URL(ref, page.url());
        if (url.origin !== origin) continue;
        const key = url.pathname + url.search;
        if (checked.has(key)) continue;
        checked.add(key);
        if (!site.assembled && BUILD_PATHS.some((re) => re.test(url.pathname))) {
          skipped.add(url.pathname);
          continue;
        }
        const res = await request.get(url.href, { maxRedirects: 5 });
        expect(res.status(), `${key} (linked from ${path})`).toBeLessThan(400);
        if (url.hash && (res.headers()["content-type"] || "").includes("text/html")) {
          expect(await res.text(), `${url.hash} in ${url.pathname}`).toContain(`id="${url.hash.slice(1)}"`);
        }
      }
    }
    // the manifest's icons
    const manifest = await (await request.get("/site.webmanifest")).json();
    for (const icon of manifest.icons) expect((await request.get(icon.src)).status(), icon.src).toBe(200);
    if (skipped.size) test.info().annotations.push({ type: "skipped until cargo xtask site", description: [...skipped].join(" ") });
  });

  test("short links redirect", async ({ request }) => {
    const to = async (path) => (await request.get(path, { maxRedirects: 0 })).headers().location;
    expect(await to("/github")).toMatch(/^https:\/\/github\.com\//);
    expect(await to("/changelog")).toBe("/docs/changelog.html");
    expect(await to("/docs")).toBe("/docs/");
    expect(await to("/studio")).toBe("/studio/?mock");
    expect(await to("/install")).toBe("/#install");
  });

  test("has no horizontal scroll at 375 px", async ({ page }) => {
    await page.setViewportSize({ width: 375, height: 812 });
    await page.goto("/");
    const offenders = () =>
      page.evaluate(() => {
        const w = document.documentElement.clientWidth;
        const out = [];
        if (document.documentElement.scrollWidth > w) {
          for (const el of document.querySelectorAll("body *")) {
            const r = el.getBoundingClientRect();
            if (r.right > w + 1 && getComputedStyle(el).position !== "fixed") {
              let p = el.parentElement;
              let clipped = false;
              while (p && p !== document.body) {
                const o = getComputedStyle(p).overflowX;
                if (o === "auto" || o === "hidden" || o === "scroll" || o === "clip") clipped = true;
                p = p.parentElement;
              }
              if (!clipped) out.push(`${el.tagName.toLowerCase()}.${el.className} right=${Math.round(r.right)}`);
            }
          }
          out.unshift(`scrollWidth ${document.documentElement.scrollWidth} > ${w}`);
        }
        return out.slice(0, 12);
      });
    expect(await offenders()).toEqual([]);
    for (const b of await page.locator("[data-made]").all()) {
      if ((await b.getAttribute("aria-expanded")) !== "true") await b.click();
    }
    expect(await offenders(), "with every 'How it was made' panel open").toEqual([]);
  });

  test("unknown paths get the 404 page", async ({ page }) => {
    const res = await page.goto("/no/such/page");
    expect(res.status()).toBe(404);
    await expect(page.locator("h1")).toHaveText("Nothing plays at this address");
  });
});

test.describe("assembled site", () => {
  test.skip(!site.assembled, "needs target/site from cargo xtask site");

  test("leaves the site's own tooling out", async ({ request }) => {
    for (const path of ["/package.json", "/package-lock.json", "/tests/smoke.spec.mjs", "/playwright.config.mjs", "/README.md", "/node_modules/.package-lock.json", "/tools/excerpt.py"]) {
      expect((await request.get(path)).status(), path).toBe(404);
    }
  });

  test("the docs and the studio demo load under their own CSP", async ({ page }) => {
    const violations = await cspViolations(page);
    const messages = [];
    page.on("console", (m) => {
      if (/Content Security Policy|Refused to|Trusted Type/i.test(m.text())) messages.push(m.text());
    });
    await page.goto("/docs/");
    await expect(page.locator("#mdbook-content, main").first()).toBeVisible();
    expect(await violations(), "docs").toEqual([]);
    await page.goto("/studio/?mock");
    await page.waitForFunction(() => !document.getElementById("loading"), null, { timeout: 30_000 });
    expect(await violations(), "studio").toEqual([]);
    expect(messages).toEqual([]);
  });

  test("the installers are served as text", async ({ request }) => {
    for (const path of ["/install.sh", "/install.ps1"]) {
      const res = await request.get(path);
      expect(res.status(), path).toBe(200);
      expect(res.headers()["content-type"], path).toMatch(/^text\/plain/);
    }
  });
});

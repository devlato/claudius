// What is said to the command palette, heard: the browser's microphone plays a request said aloud once
// (CLAUDIUS_SPEECH, a 16-bit WAV file), the palette's microphone is clicked, and the words come back into its field
// (CLAUDIUS_SPEECH_WORDS, as the palette reads them) with the row they find. The speech model is CLAUDIUS_WHISTLE_MODEL,
// copied into the claudius' cache, so that it does not download.
import { chromium, expect, test } from "@playwright/test";
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { claudius, heard, ready, why } from "./claudius.mjs";

const [speech, words, model] = ["CLAUDIUS_SPEECH", "CLAUDIUS_SPEECH_WORDS", "CLAUDIUS_WHISTLE_MODEL"].map((v) => process.env[v]);

/** `wav` (16-bit) with a second of silence before it, as a person pauses after clicking: the browser's fake
 * microphone plays its file from when the page asks for it, before the palette's recorder hears */
function paused(wav) {
  const b = readFileSync(wav);
  let at = 12;
  const chunks = {};
  while (at + 8 <= b.length) {
    chunks[b.toString("ascii", at, at + 4)] = [at + 8, b.readUInt32LE(at + 4)];
    at += 8 + b.readUInt32LE(at + 4) + (b.readUInt32LE(at + 4) % 2);
  }
  const [fmt] = chunks["fmt "];
  const [data, size] = chunks.data;
  const [channels, rate] = [b.readUInt16LE(fmt + 2), b.readUInt32LE(fmt + 4)];
  const silence = Buffer.alloc(rate * channels * 2);
  const head = Buffer.from(b.subarray(0, data));
  head.writeUInt32LE(size + silence.length, data - 4);
  head.writeUInt32LE(head.length - 8 + size + silence.length, 4);
  const out = join(mkdtempSync(join(tmpdir(), "claudius-speech-")), "said.wav");
  writeFileSync(out, Buffer.concat([head, silence, b.subarray(data, data + size)]));
  return out;
}

test.describe("saying what to do", () => {
  test.skip(!ready, why);
  test.skip(!speech || !words || !model, "needs CLAUDIUS_SPEECH, CLAUDIUS_SPEECH_WORDS and CLAUDIUS_WHISTLE_MODEL");

  test("what is said to the palette goes into its field, with the row it finds", async () => {
    test.setTimeout(180_000);
    const c = await claudius();
    const models = join(c.home, "cache", "models");
    mkdirSync(models, { recursive: true });
    copyFileSync(model, join(models, "whistle.cact"));
    const args = ["--autoplay-policy=no-user-gesture-required", "--use-fake-device-for-media-stream"];
    args.push("--use-fake-ui-for-media-stream", `--use-file-for-fake-audio-capture=${paused(speech)}%noloop`);
    const b = await chromium.launch({ args });
    try {
      const page = await b.newPage({ permissions: ["microphone"] });
      const lines = heard(page, "palette");
      await page.goto(`${c.base}/?plan=${encodeURIComponent("wait:3;palette;palette:say;wait:30")}`);
      await expect.poll(() => lines.join("\n"), { timeout: 60_000 }).toContain(`${words} -> `).catch((e) => {
        throw new Error(`${e.message}\nclaudius said:\n${c.log()}`);
      });
      // it opened the microphone, listened until a pause, then heard what was said, and nothing went wrong between
      expect(lines.indexOf("listening")).toBeGreaterThan(lines.indexOf("opening the microphone"));
      expect(lines.indexOf("hearing what was said")).toBeGreaterThan(lines.indexOf("listening"));
      expect(lines.join("\n")).not.toMatch(/not heard/);
    } finally {
      await b.close();
      c.stop();
    }
  });
});

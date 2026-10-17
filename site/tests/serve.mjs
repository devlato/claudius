// A static server that behaves like Cloudflare Pages for the smoke test: it applies _headers (including detached
// headers), follows _redirects, serves /dir/ from /dir/index.html, answers missing paths with 404.html and status
// 404, and supports byte ranges for audio. It serves the assembled site (target/site, from `cargo xtask site`) when
// it exists; otherwise site/ itself, with the placeholders filled in the way the build fills them.
//
//   node tests/serve.mjs [--port 4321] [--root DIR]
//
// Differences from Pages: no HTTPS, so `upgrade-insecure-requests` is left out of the CSP; nothing is cached.

import { createServer } from "node:http";
import { existsSync, readFileSync, statSync, createReadStream } from "node:fs";
import { extname, join, normalize, resolve, sep, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
export const SITE_SRC = resolve(here, "..");
export const ASSEMBLED = resolve(SITE_SRC, "..", "target", "site");

/** where the site to test is, and whether it is the assembled one (a build that stopped before the book is not) */
export function siteRoot() {
  const forced = process.env.SITE_DIR;
  if (forced) return { dir: resolve(forced), assembled: resolve(forced) !== SITE_SRC };
  if (existsSync(join(ASSEMBLED, "index.html")) && existsSync(join(ASSEMBLED, "docs", "index.html"))) {
    return { dir: ASSEMBLED, assembled: true };
  }
  return { dir: SITE_SRC, assembled: false };
}

/** paths that only exist once `cargo xtask site` has run */
export const BUILD_PATHS = [/^\/docs(\/|$)/, /^\/studio(\/|$)/, /^\/install\.(sh|ps1)$/];

const TYPES = {
  ".html": "text/html; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".json": "application/json",
  ".webmanifest": "application/manifest+json",
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".webp": "image/webp",
  ".ico": "image/x-icon",
  ".woff2": "font/woff2",
  ".mp3": "audio/mpeg",
  ".wav": "audio/wav",
  ".wasm": "application/wasm",
  ".txt": "text/plain; charset=utf-8",
  ".rhai": "text/plain; charset=utf-8",
  ".sh": "text/plain; charset=utf-8",
  ".ps1": "text/plain; charset=utf-8",
  ".xml": "application/xml",
};
const TEXT = new Set([".html", ".css", ".js", ".mjs", ".json", ".webmanifest", ".svg", ".txt", ".xml"]);
// Pages reads these and never serves them
const CONFIG = /^\/(_headers|_redirects)$/;
// files in site/ that are not part of the website (the build leaves them out)
export const PRIVATE = /^\/(node_modules|tests|tools|test-results|playwright-report|blob-report)(\/|$)|^\/(package(-lock)?\.json|playwright\.config\.mjs|README\.md|\.gitignore)$/;

function workspaceVersion() {
  try {
    const toml = readFileSync(resolve(SITE_SRC, "..", "Cargo.toml"), "utf8");
    const m = toml.match(/\[workspace\.package\][^[]*?version\s*=\s*"([^"]+)"/);
    return m ? m[1] : "0.0.0";
  } catch {
    return "0.0.0";
  }
}

function patternToRegex(p) {
  const parts = p.split("*").map((s) => s.replace(/[.+?^${}()|[\]\\]/g, "\\$&"));
  return new RegExp("^" + parts.join(".*").replace(/:[A-Za-z]\w*/g, "[^/]+") + "$");
}

function parseHeaders(text) {
  const rules = [];
  let rule = null;
  for (const raw of text.split(/\r?\n/)) {
    if (!raw.trim() || raw.trim().startsWith("#")) continue;
    if (!/^\s/.test(raw)) {
      rule = { re: patternToRegex(raw.trim()), set: [], unset: [] };
      rules.push(rule);
    } else if (rule) {
      const line = raw.trim();
      if (line.startsWith("!")) rule.unset.push(line.slice(1).trim().toLowerCase());
      else {
        const i = line.indexOf(":");
        rule.set.push([line.slice(0, i).trim(), line.slice(i + 1).trim()]);
      }
    }
  }
  return rules;
}

// Cloudflare's order: every matching rule in file order; a detach removes what earlier rules set; a header set by
// two rules is joined with a comma
function headersFor(rules, path) {
  const out = new Map();
  const seen = new Set();
  for (const r of rules) {
    if (!r.re.test(path)) continue;
    for (const name of r.unset) out.delete(name);
    for (const [name, value] of r.set) {
      const k = name.toLowerCase();
      if (seen.has(k) && out.has(k)) out.set(k, [out.get(k)[0], out.get(k)[1] + ", " + value]);
      else out.set(k, [name, value]);
      seen.add(k);
    }
  }
  return out;
}

function parseRedirects(text) {
  return text
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter((l) => l && !l.startsWith("#"))
    .map((l) => l.split(/\s+/))
    .filter((f) => f.length >= 2)
    .map(([from, to, code]) => ({ re: patternToRegex(from), to, code: Number(code || 302) }));
}

export function start({ port = 4321, root } = {}) {
  const site = root ? { dir: resolve(root), assembled: true } : siteRoot();
  const origin = `http://127.0.0.1:${port}`;
  const fill = (s) =>
    site.assembled
      ? s
      : s
          .replaceAll("{{version}}", workspaceVersion())
          .replaceAll("{{site_url}}", origin)
          .replaceAll("{{repo_url}}", "https://github.com/devlato/claudius");
  const read = (name) => (existsSync(join(site.dir, name)) ? fill(readFileSync(join(site.dir, name), "utf8")) : "");
  const headerRules = parseHeaders(read("_headers"));
  const redirects = parseRedirects(read("_redirects"));

  const server = createServer((req, res) => {
    const url = new URL(req.url, origin);
    let path = decodeURIComponent(url.pathname);

    const send = (status, file, extra = {}) => {
      const headers = headersFor(headerRules, path);
      for (const [, [name, value]] of headers) {
        if (name.toLowerCase() === "content-security-policy") {
          res.setHeader(name, value.replace(/;\s*upgrade-insecure-requests/g, ""));
        } else if (name.toLowerCase() !== "strict-transport-security") {
          res.setHeader(name, value);
        }
      }
      res.setHeader("Cache-Control", "no-store");
      for (const [k, v] of Object.entries(extra)) res.setHeader(k, v);
      if (!file) {
        res.statusCode = status;
        res.end();
        return;
      }
      const ext = extname(file).toLowerCase();
      if (!res.hasHeader("Content-Type")) res.setHeader("Content-Type", TYPES[ext] || "application/octet-stream");
      if (TEXT.has(ext)) {
        const body = Buffer.from(fill(readFileSync(file, "utf8")));
        res.statusCode = status;
        res.setHeader("Content-Length", body.length);
        res.end(req.method === "HEAD" ? undefined : body);
        return;
      }
      const size = statSync(file).size;
      res.setHeader("Accept-Ranges", "bytes");
      const range = req.headers.range && /^bytes=(\d*)-(\d*)$/.exec(req.headers.range);
      if (range && status === 200) {
        let a = range[1] === "" ? size - Number(range[2]) : Number(range[1]);
        let b = range[1] === "" || range[2] === "" ? size - 1 : Math.min(Number(range[2]), size - 1);
        if (a > b || a >= size) {
          res.statusCode = 416;
          res.setHeader("Content-Range", `bytes */${size}`);
          res.end();
          return;
        }
        res.statusCode = 206;
        res.setHeader("Content-Range", `bytes ${a}-${b}/${size}`);
        res.setHeader("Content-Length", b - a + 1);
        if (req.method === "HEAD") return res.end();
        createReadStream(file, { start: a, end: b }).pipe(res);
        return;
      }
      res.statusCode = status;
      res.setHeader("Content-Length", size);
      if (req.method === "HEAD") return res.end();
      createReadStream(file).pipe(res);
    };

    for (const r of redirects) {
      if (r.re.test(path)) return send(r.code, null, { Location: r.to });
    }
    const notFound = () => {
      const page = join(site.dir, "404.html");
      return existsSync(page) ? send(404, page) : send(404, null);
    };
    if (CONFIG.test(path) || (!site.assembled && PRIVATE.test(path))) return notFound();
    const file = normalize(join(site.dir, path));
    if (!file.startsWith(site.dir + sep) && file !== site.dir) return notFound();
    if (existsSync(file) && statSync(file).isDirectory()) {
      if (!path.endsWith("/")) return send(308, null, { Location: path + "/" + url.search });
      const index = join(file, "index.html");
      return existsSync(index) ? send(200, index) : notFound();
    }
    if (existsSync(file)) return send(200, file);
    if (existsSync(file + ".html")) return send(200, file + ".html");
    return notFound();
  });
  return new Promise((ok) => server.listen(port, "127.0.0.1", () => ok({ server, site, origin })));
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2);
  const opt = (name, dflt) => {
    const i = args.indexOf(name);
    return i >= 0 ? args[i + 1] : dflt;
  };
  const { site, origin } = await start({ port: Number(opt("--port", process.env.PORT || 4321)), root: opt("--root") });
  console.log(`serving ${site.dir}${site.assembled ? "" : " (placeholders filled in)"} at ${origin}`);
}

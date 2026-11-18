// Serves a built directory as a single-page app (unknown paths get index.html) on one address, for the fixtures build that people open on the iPhone Simulator over NetBird.
// Usage: node scripts/serve-fixtures.mjs <dir> <host> <port>. Static files only; no proxy, no cookies, nothing signed in: fixtures data. Run by deploy-preview.sh.
import { createServer } from "node:http";
import { readFile, stat } from "node:fs/promises";
import { extname, join, normalize } from "node:path";
const [dir, host, port] = process.argv.slice(2);
if (!dir || !host || !port) { console.error("usage: serve-fixtures.mjs <dir> <host> <port>"); process.exit(2); }
const TYPES = { ".html": "text/html; charset=utf-8", ".js": "text/javascript", ".css": "text/css", ".json": "application/json", ".svg": "image/svg+xml", ".png": "image/png", ".jpg": "image/jpeg", ".webp": "image/webp", ".woff2": "font/woff2", ".woff": "font/woff", ".ico": "image/x-icon", ".webmanifest": "application/manifest+json" };
createServer(async (req, res) => {
  const path = normalize(decodeURIComponent(new URL(req.url, "http://x").pathname)).replace(/^(\.\.[/\\])+/, "");
  let file = join(dir, path);
  try { if ((await stat(file)).isDirectory()) file = join(file, "index.html"); } catch { file = join(dir, "index.html"); }
  try {
    const body = await readFile(file);
    res.writeHead(200, { "content-type": TYPES[extname(file)] ?? "application/octet-stream", "cache-control": "no-cache", "access-control-allow-origin": "*" });
    res.end(body);
  } catch { res.writeHead(404); res.end("not found"); }
}).listen(Number(port), host, () => console.log(`fixtures on http://${host}:${port}/`));

// Serves the built mockup (dist/) over HTTP for the owner's browser: `node serve.mjs [port]` (default 4313). Static files only; nothing is proxied and there is no server behind the mockup.
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";
const root = join(fileURLToPath(new URL(".", import.meta.url)), "dist"), port = Number(process.argv[2] ?? process.env.PORT ?? 4313);
const types = { ".html": "text/html; charset=utf-8", ".js": "text/javascript", ".css": "text/css", ".woff2": "font/woff2", ".jpg": "image/jpeg", ".png": "image/png", ".json": "application/json", ".svg": "image/svg+xml" };
createServer(async (req, res) => {
  let p = normalize(decodeURIComponent((req.url ?? "/").split("?")[0])).replace(/^([/\\])+/, "");
  if (!p || p.endsWith("/")) p += "index.html";
  if (p.includes("..")) { res.writeHead(400).end(); return; }
  try { const b = await readFile(join(root, p)); res.writeHead(200, { "content-type": types[extname(p)] ?? "application/octet-stream", "cache-control": "no-cache" }).end(b); }
  catch { res.writeHead(404, { "content-type": "text/plain" }).end("Not found"); }
}).listen(port, "0.0.0.0", () => console.log(`calternal mockup on http://0.0.0.0:${port}/`));

// The overnight demo server: serves the built harness (`dist-harness`) at /app/ and a plain index page at /, and proxies the step 0 host's routes under /api/ so a browser needs no tunnel and never sees a bearer
// (the server adds it from a file on every request and drops any Authorization a client sends). Only the sealed routes are forwarded; request bodies are capped. No dependencies. Usage:
//   DEMO_PORT=4312 STEP0_BASE=http://127.0.0.1:43125/step0/v2 BEARER_FILE=~/Developer/calternal-ai-lanes/.owner.token node demo/server.mjs
import { createServer } from "node:http";
import { createReadStream, existsSync, readFileSync, statSync } from "node:fs";
import { dirname, extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const PORT = Number(process.env.DEMO_PORT ?? 4312), UP = (process.env.STEP0_BASE ?? "http://127.0.0.1:43125/step0/v2").replace(/\/$/, "");
const TOKEN_FILE = (process.env.BEARER_FILE ?? `${process.env.HOME}/Developer/calternal-ai-lanes/.owner.token`).replace(/^~/, process.env.HOME ?? "");
const DIST = join(here, "../dist-harness");
const ALLOW = /^\/api\/(fixture|view\.open|view\.renew|window|selection\.settled|selection\.set|activation\.(resolve|confirm|dispatch)|choice\.options|undo|status|search|row-images\/[\w.-]+)$/;
const TYPES = { ".html": "text/html; charset=utf-8", ".js": "text/javascript", ".css": "text/css", ".jpg": "image/jpeg", ".png": "image/png", ".svg": "image/svg+xml", ".json": "application/json" };
let token = "", read = 0;
const bearer = () => { if (Date.now() - read > 5000) { try { token = readFileSync(TOKEN_FILE, "utf8").trim(); } catch { token = ""; } read = Date.now(); } return token; };

async function proxy(req, res, path) {
  const chunks = []; let size = 0;
  for await (const c of req) { size += c.length; if (size > 131072) { res.writeHead(413).end(); return; } chunks.push(c); }
  const headers = { authorization: `Bearer ${bearer()}` };
  for (const [k, v] of Object.entries(req.headers)) if (k === "content-type" || k.startsWith("x-step0-")) headers[k] = v;
  try {
    const up = await fetch(`${UP}/${path}`, { method: req.method, headers, body: size ? Buffer.concat(chunks) : undefined, signal: AbortSignal.timeout(15000) });
    const body = Buffer.from(await up.arrayBuffer());
    res.writeHead(up.status, { "content-type": up.headers.get("content-type") ?? "application/json", "cache-control": "no-store" }).end(body);
  } catch { res.writeHead(502, { "content-type": "application/json" }).end(JSON.stringify({ code: "host_unreachable", message: "The demo host is not reachable right now." })); }
}

function file(res, p, type) { res.writeHead(200, { "content-type": type, "cache-control": "no-cache" }); createReadStream(p).pipe(res); }

createServer(async (req, res) => {
  const url = new URL(req.url ?? "/", "http://x");
  if (url.pathname.startsWith("/api/")) {
    if (!ALLOW.test(url.pathname) || !["GET", "POST"].includes(req.method ?? "")) { res.writeHead(404).end(); return; }
    return proxy(req, res, url.pathname.slice(5));
  }
  if (url.pathname === "/" || url.pathname === "/index.html") return file(res, join(here, "index.html"), TYPES[".html"]);
  if (url.pathname === "/app" ) { res.writeHead(302, { location: `/app/${url.search}` }).end(); return; }
  if (url.pathname.startsWith("/app/")) {
    const rel = normalize(url.pathname.slice(5) || "index.html").replace(/^(\.\.[/\\])+/, "");
    let p = join(DIST, rel === "." || rel === "" ? "index.html" : rel);
    if (!p.startsWith(DIST) || !existsSync(p) || statSync(p).isDirectory()) p = join(DIST, "index.html");
    return file(res, p, TYPES[extname(p)] ?? "application/octet-stream");
  }
  res.writeHead(404).end("Not found");
}).listen(PORT, "0.0.0.0", () => console.log(`demo on :${PORT}, host ${UP}`));

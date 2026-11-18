// Headed GPU Chrome (macOS): the composer collapsed, mid-morph and expanded, desktop and phone, dark, from compositor frames (Page.startScreencast: the screenshot call returns a stale frame in a VM).
// Usage on the Mac: CHANNEL=chrome HEADLESS=0 node gpu-fold.mjs <dist dir> <out dir>   (serves the dist itself)
import http from "node:http"; import fs from "node:fs"; import path from "node:path";
const [DIST, OUT] = process.argv.slice(2); fs.mkdirSync(OUT, { recursive: true });
const pw = await import("playwright-core");
const types = { ".js": "text/javascript", ".css": "text/css", ".html": "text/html", ".woff2": "font/woff2", ".jpg": "image/jpeg", ".png": "image/png", ".svg": "image/svg+xml", ".json": "application/json", ".webp": "image/webp" };
const srv = http.createServer((req, res) => { const root = path.resolve(DIST); let p = path.join(root, decodeURIComponent(new URL(req.url, "http://x").pathname)); if (!p.startsWith(root) || !fs.existsSync(p) || fs.statSync(p).isDirectory()) p = path.join(root, "index.html"); res.writeHead(200, { "content-type": types[path.extname(p)] ?? "application/octet-stream" }); fs.createReadStream(p).pipe(res); }).listen(4621, "127.0.0.1");
const b = await pw.chromium.launch({ channel: "chrome", headless: false });
for (const [name, w, h, mobile] of [["desktop", 1500, 970, false], ["phone", 390, 844, true]]) {
  const ctx = await b.newContext({ viewport: { width: w, height: h }, deviceScaleFactor: 2, isMobile: mobile, hasTouch: mobile, colorScheme: "dark" });
  await ctx.addInitScript(() => localStorage.setItem("cal.firstrun", "1"));
  const page = await ctx.newPage(); await page.bringToFront(); const cdp = await ctx.newCDPSession(page);
  await page.goto("http://127.0.0.1:4621/threads/t-idle"); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(1500);
  let last = null; cdp.on("Page.screencastFrame", (f) => { last = f.data; cdp.send("Page.screencastFrameAck", { sessionId: f.sessionId }).catch(() => {}); });
  await cdp.send("Page.startScreencast", { format: "png", maxWidth: 3000, maxHeight: 2000, everyNthFrame: 1 });
  // The screencast only sends a frame when the page changes: nudge the pointer until one has arrived.
  const save = async (n) => { for (let i = 0; i < 20 && !last; i++) { await page.mouse.move(w / 2 + (i % 2), 300); await page.waitForTimeout(100); } fs.writeFileSync(`${OUT}/${name}-${n}.png`, Buffer.from(last, "base64")); };
  await page.mouse.move(w / 2, 300); await page.waitForTimeout(700); await save("collapsed");
  const f = page.locator("#composer-field").boundingBox();
  const fb = await f; await page.mouse.click(fb.x + 40, fb.y + 10);
  await page.waitForTimeout(110); await save("mid");
  await page.waitForTimeout(900); await save("expanded");
  await page.mouse.click(w / 2, 200); await page.waitForTimeout(900); await save("collapsed-again");
  await cdp.send("Page.stopScreencast"); await ctx.close();
}
await b.close(); srv.close();

// Sealed-frame cost, measured: mount time and font readiness, heap per frame, streaming frame time against the same UI in the page, focus and shortcut behaviour, and whether a frame's own backdrop-filter can
// blur the page behind it. Usage: cd apps/web && node bench/ui-tree/run-frame.mjs (public/frame.html is served from bench/ui-tree/dist after a build, fonts from apps/web/public).
import http from "node:http"; import fs from "node:fs"; import path from "node:path"; import { fileURLToPath } from "node:url";
import { launch } from "../../e2e/lib.mjs";
const here = path.dirname(fileURLToPath(import.meta.url)); const dist = path.join(here, "dist"), pub = path.join(here, "../../public");
const T = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".woff2": "font/woff2" };
const srv = http.createServer((q, r) => { const u = decodeURIComponent(new URL(q.url, "http://x").pathname); let p = path.join(u.startsWith("/fonts/") ? pub : dist, u); if (!fs.existsSync(p) || fs.statSync(p).isDirectory()) { r.writeHead(404); return r.end(); } r.writeHead(200, { "content-type": T[path.extname(p)] ?? "application/octet-stream", ...(process.env.NOCORS || !u.startsWith("/fonts/") ? {} : { "access-control-allow-origin": "*" }) }); fs.createReadStream(p).pipe(r); });
await new Promise((r) => srv.listen(0, "127.0.0.1", r)); const base = `http://127.0.0.1:${srv.address().port}/frame.html`;
const b = await launch(); const q = (a, f) => { const s = [...a].sort((x, y) => x - y); return s[Math.min(s.length - 1, Math.floor(s.length * f))]; }; const f1 = (x) => x.toFixed(1);
const fresh = async (rate = 1) => { const ctx = await b.newContext({ viewport: { width: 1100, height: 800 } }); const page = await ctx.newPage(); await page.goto(base); await page.waitForFunction(() => window.bench); const cdp = await ctx.newCDPSession(page); await cdp.send("Performance.enable"); if (rate > 1) await cdp.send("Emulation.setCPUThrottlingRate", { rate }); return { ctx, page, cdp }; };
const metric = async (cdp, k) => (await cdp.send("Performance.getMetrics")).metrics.find((m) => m.name === k).value;
console.log("== mount: ms until every UI has painted (frames: and until its font is usable), heap per UI");
for (const rate of [1, 4]) for (const n of [1, 5, 20]) for (const sealed of [false, true]) {
  const { ctx, page, cdp } = await fresh(rate); await page.waitForTimeout(300); const h0 = await metric(cdp, "JSHeapUsedSize"), d0 = await metric(cdp, "Nodes");
  const r = await page.evaluate(([n, s]) => window.bench.mount(n, s), [n, sealed]); await page.waitForTimeout(300);
  const h1 = await metric(cdp, "JSHeapUsedSize"), d1 = await metric(cdp, "Nodes");
  console.log(`${rate}x ${sealed ? "frame " : "inpage"} x${String(n).padEnd(2)}  painted ${f1(r.all).padStart(7)} ms${sealed ? `  (font first ${f1(r.firstFont)}, last ${f1(r.lastFont)})` : ""}   main-process heap +${((h1 - h0) / n / 1024).toFixed(0)} KB per UI, nodes +${Math.round((d1 - d0) / n)} per UI`);
  await ctx.close();
}
console.log("== streaming into 5 UIs at 60 Hz for 3 s");
for (const rate of [1, 4]) for (const sealed of [false, true]) {
  const { ctx, page, cdp } = await fresh(rate); await page.evaluate(([n, s]) => window.bench.mount(n, s), [5, sealed]); await page.waitForTimeout(500);
  const t0 = await metric(cdp, "TaskDuration"), w0 = Date.now(); const d = (await page.evaluate(([ms, s]) => window.bench.stream(ms, s), [3000, sealed])).slice(2); const busy = ((await metric(cdp, "TaskDuration")) - t0) * 1000 / ((Date.now() - w0) / 1000);
  console.log(`${rate}x ${sealed ? "frame " : "inpage"}  frames ${d.length}  frame ms p50 ${f1(q(d, 0.5))}  p95 ${f1(q(d, 0.95))}  max ${f1(Math.max(...d))}  >25ms: ${d.filter((x) => x > 25).length}   main-thread busy ${f1(busy)} ms/s`);
  await ctx.close();
}
console.log("== focus, shortcuts, material");
{
  const { ctx, page } = await fresh(); await page.evaluate(() => { document.getElementById("host").insertAdjacentHTML("beforebegin", '<button id="before" style="position:relative;z-index:2">before</button>'); window.__keys = []; addEventListener("keydown", (e) => __keys.push(e.key)); });
  await page.evaluate(() => window.bench.mount(1, true)); await page.waitForTimeout(600);
  await page.focus("#before"); await page.keyboard.press("Tab"); await page.keyboard.press("Tab");
  const ae = await page.evaluate(() => ({ tag: document.activeElement.tagName, keysSeenByPage: window.__keys.slice() }));
  await page.keyboard.type("ab"); const after = await page.evaluate(() => window.__keys.slice());
  console.log(`Tab from a page button: focus lands on ${ae.tag} (the frame element); page keydown listener saw ${after.filter((k) => k === "a" || k === "b").length} of 2 typed keys inside the frame (a global shortcut would not fire unless the frame forwards it; it posts them as messages)`);
  await page.evaluate(() => document.querySelector("iframe").style.cssText += ";width:0;height:0"); await ctx.close();
}
for (const sealed of [false, true]) {
  const { ctx, page } = await fresh(); await page.evaluate(([n, s]) => window.bench.mount(n, s), [1, sealed]); await page.waitForTimeout(700);
  const box = await page.evaluate((s) => { if (s) { const f = window.__g.frame.getBoundingClientRect(), g = window.__g.g; return { x: f.x + g.x, y: f.y + g.y, w: g.width, h: g.height }; } const r = document.getElementById("g").getBoundingClientRect(); return { x: r.x, y: r.y, w: r.width, h: r.height }; }, sealed);
  const png = (await page.screenshot({ clip: { x: box.x + 10, y: box.y + box.h - 7, width: Math.min(240, box.w - 20), height: 4 } })).toString("base64");
  const edges = await page.evaluate(async (b64) => { const img = new Image(); img.src = "data:image/png;base64," + b64; await img.decode(); const c = document.createElement("canvas"); c.width = img.width; c.height = img.height; const x = c.getContext("2d"); x.drawImage(img, 0, 0); const d = x.getImageData(0, 1, img.width, 1).data; let sharp = 0; for (let i = 4; i < d.length; i += 4) if (Math.abs(d[i] - d[i - 4]) > 70) sharp++; return sharp; }, png);
  console.log(`${sealed ? "frame " : "inpage"} backdrop-filter:blur(20px) over the page's red/blue stripes: ${edges} sharp stripe edges left across the probe (${sealed ? "the frame cannot sample what is behind it: the page must supply the material" : "blurred by the page's own backdrop"})`);
  await ctx.close();
}
await b.close(); srv.close();

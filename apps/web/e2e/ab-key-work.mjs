// What a key in the composer costs, three ways, for two builds in alternating rounds at 4x CPU throttling (headless on a dev VM; headed Chrome on the Mac with CHANNEL=chrome HEADLESS=0).
//   (a) main-thread work per key: the key event's task plus the rendering task that draws it (style, layout, paint, commit), from a trace. No wait for the display is in it.
//   (b) frames missed: the key was not drawn in the very next frame: the first animation frame after the key came more than 20 ms after the one before it (a frame was dropped around it).
//   (c) key to next frame: the app's probe (?test=1, window.__keys): from the key event to the next animation frame, including the wait for the display's refresh.
// Usage: node e2e/ab-key-work.mjs <distA> <distB> [rounds]   (two builds of apps/web; serves them itself on ports 4611 and 4612; needs `playwright`, or `playwright-core` with CHANNEL)
import http from "node:http"; import fs from "node:fs"; import path from "node:path";
const [DA, DB, R = "20"] = process.argv.slice(2);
const pw = await import(process.env.CHANNEL ? "playwright-core" : "playwright");
const serve = (dir, port) => http.createServer((req, res) => {
  const root = path.resolve(dir); let p = path.join(root, decodeURIComponent(new URL(req.url, "http://x").pathname));
  if (!p.startsWith(root) || !fs.existsSync(p) || fs.statSync(p).isDirectory()) p = path.join(root, "index.html");
  const types = { ".js": "text/javascript", ".css": "text/css", ".html": "text/html", ".woff2": "font/woff2", ".jpg": "image/jpeg", ".png": "image/png", ".svg": "image/svg+xml", ".json": "application/json", ".webp": "image/webp" };
  res.setHeader("content-type", types[path.extname(p)] ?? "application/octet-stream"); fs.createReadStream(p).pipe(res);
}).listen(port, "127.0.0.1");
const servers = [serve(DA, 4611), serve(DB, 4612)];
const urls = { A: "http://127.0.0.1:4611", B: "http://127.0.0.1:4612" };
const LINE = "the quick brown fox jumps over the lazy dog, then naps @Priya friday 5pm. ";
const pct = (xs, p) => { const s = [...xs].sort((a, c) => a - c); return s.length ? s[Math.min(s.length - 1, Math.floor(p * (s.length - 1)))] : NaN; };
const b = await pw.chromium.launch(process.env.CHANNEL ? { channel: process.env.CHANNEL, headless: process.env.HEADLESS !== "0" } : {});
const PROBE = `window.__kd = []; window.__raf = []; addEventListener("keydown", (e) => { if (e.isTrusted) window.__kd.push(performance.now()); }, true); (function f(t) { window.__raf.push(t); requestAnimationFrame(f); })(0); requestAnimationFrame(function f(t) { window.__raf.push(t); requestAnimationFrame(f); });`;
async function one(base) {
  const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });
  await ctx.addInitScript(() => { try { localStorage.setItem("cal.firstrun", "1"); } catch {} });
  await ctx.addInitScript(PROBE);
  const page = await ctx.newPage(); const cdp = await ctx.newCDPSession(page);
  await page.goto(base + "/threads/t-idle?test=1"); await page.waitForSelector("#composer-field"); await page.waitForTimeout(1500);
  await cdp.send("Emulation.setCPUThrottlingRate", { rate: 4 });
  const f = page.locator("#composer-field"); await f.click(); await page.waitForTimeout(300);
  await page.evaluate(() => { window.__kd.length = 0; window.__raf.length = 0; });
  await cdp.send("Tracing.start", { categories: "devtools.timeline,disabled-by-default-devtools.timeline,toplevel", transferMode: "ReturnAsStream" });
  await f.pressSequentially(LINE, { delay: 60 }); await page.waitForTimeout(400);
  const done = new Promise((r) => cdp.once("Tracing.tracingComplete", r));
  await cdp.send("Tracing.end"); const { stream } = await done;
  let text = ""; for (;;) { const c = await cdp.send("IO.read", { handle: stream }); text += c.data; if (c.eof) break; }
  const ev = JSON.parse(text).traceEvents ?? JSON.parse(text);
  const probe = await page.evaluate(() => ({ keys: window.__keys ?? [], kd: window.__kd, raf: window.__raf }));
  await ctx.close();
  // (a) from the trace
  const main = ev.find((e) => e.name === "thread_name" && e.args?.name === "CrRendererMain");
  const onMain = ev.filter((e) => e.pid === main?.pid && e.tid === main?.tid && e.ph === "X");
  const tasks = onMain.filter((e) => e.name === "RunTask").sort((a, c) => a.ts - c.ts);
  const kids = (t) => onMain.filter((e) => e !== t && e.ts >= t.ts && e.ts < t.ts + t.dur);
  const downs = onMain.filter((e) => e.name === "EventDispatch" && e.args?.data?.type === "keydown").sort((a, c) => a.ts - c.ts);
  const work = [];
  for (const d of downs) {
    const kt = tasks.find((t) => t.ts <= d.ts && d.ts < t.ts + t.dur); if (!kt) continue;
    const ft = tasks.find((t) => t.ts >= kt.ts + kt.dur && kids(t).some((k) => k.name === "Commit" || k.name === "Layerize" || k.name === "PrePaint"));
    work.push((kt.dur + (ft ? ft.dur : 0)) / 1000);
    if (process.env.FIRST && work.length === 1) {
      const sums = new Map();
      for (const t of [kt, ft].filter(Boolean)) for (const k of kids(t)) sums.set(k.name, (sums.get(k.name) ?? 0) + k.dur / 1000);
      console.log(`first key: key task ${(kt.dur / 1000).toFixed(1)} ms + frame task ${((ft?.dur ?? 0) / 1000).toFixed(1)} ms; inside (events nest, so these overlap):\n` + [...sums.entries()].sort((a, c) => c[1] - a[1]).slice(0, 14).map(([k, v]) => `   ${v.toFixed(2).padStart(6)} ms  ${k}`).join("\n"));
    }
  }
  // (b) frames missed, from the animation frame ticks
  let missed = 0, n = 0;
  for (const k of probe.kd) { const i = probe.raf.findIndex((t) => t > k); if (i > 0) { n++; if (probe.raf[i] - probe.raf[i - 1] > 20) missed++; } }
  return { work, missed, n, keys: probe.keys };
}
const runs = { A: [], B: [] };
for (let i = 0; i < +R; i++) for (const k of ["A", "B"]) runs[k].push(await one(urls[k]));
const flat = (k, f) => runs[k].flatMap(f);
const row = (k) => { const w = flat(k, (r) => r.work), c = flat(k, (r) => r.keys), m = runs[k].reduce((s, r) => s + r.missed, 0), n = runs[k].reduce((s, r) => s + r.n, 0);
  return { keys: w.length, "(a) work median": +pct(w, 0.5).toFixed(1), "(a) p95": +pct(w, 0.95).toFixed(1), "(a) worst": +Math.max(...w).toFixed(1), "(a) over 8 ms": +(w.filter((v) => v > 8).length / w.length * 100).toFixed(1) + "%",
    "(b) keys not in the next frame": `${m} of ${n}`, "(c) key to frame median": +pct(c, 0.5).toFixed(1), "(c) p95": +pct(c, 0.95).toFixed(1), "(c) over 8 ms": +(c.filter((v) => v > 8).length / c.length * 100).toFixed(1) + "%" }; };
const first = (k) => +pct(runs[k].map((r) => r.work[0]), 0.5).toFixed(1);
console.log(JSON.stringify({ rounds: +R, A: row("A"), B: row("B"), "first key work (a), median": { A: first("A"), B: first("B") } }, null, 1));
await b.close(); for (const s of servers) s.close(); process.exit(0);

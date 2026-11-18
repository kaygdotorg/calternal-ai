// Drives the bench page: serves dist/, runs each scenario at 1x and 4x CPU throttling, in "worker" mode (patches cross a worker boundary, structured-cloned) and "direct" (same patches generated on the main thread),
// and prints frame-time and latency tables. Usage: cd apps/web && npx vite build --config bench/ui-tree/vite.config.ts && node bench/ui-tree/run.mjs
import http from "node:http"; import fs from "node:fs"; import path from "node:path"; import { fileURLToPath } from "node:url";
import { launch } from "../../e2e/lib.mjs";
const root = path.join(path.dirname(fileURLToPath(import.meta.url)), "dist");
const T = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".woff2": "font/woff2", ".svg": "image/svg+xml" };
const srv = http.createServer((q, r) => { let p = path.join(root, new URL(q.url, "http://x").pathname); if (!fs.existsSync(p) || fs.statSync(p).isDirectory()) p = path.join(root, "index.html"); r.writeHead(200, { "content-type": T[path.extname(p)] ?? "application/octet-stream" }); fs.createReadStream(p).pipe(r); });
await new Promise((r) => srv.listen(0, "127.0.0.1", r)); const base = `http://127.0.0.1:${srv.address().port}/`;
const b = await launch();
const q = (a, f) => { const s = [...a].sort((x, y) => x - y); return s.length ? s[Math.min(s.length - 1, Math.floor(s.length * f))] : NaN; };
const f1 = (x) => (Number.isFinite(x) ? x.toFixed(1) : "-");
const rows = [];
for (const rate of [1, 4]) for (const mode of ["worker", "direct"]) for (const scenario of ["idle", "scroll", "stream", "resize", "window"]) {
  if (scenario === "idle" && mode === "direct") continue;
  if (scenario === "scroll" && mode === "direct") continue;
  const ctx = await b.newContext({ viewport: { width: 1440, height: 900 } }); const page = await ctx.newPage();
  await page.goto(base); await page.waitForFunction(() => window.__bench); await page.waitForTimeout(800);
  const cdp = await ctx.newCDPSession(page); await cdp.send("Emulation.setCPUThrottlingRate", { rate }); await cdp.send("Performance.enable");
  const metrics = async () => Object.fromEntries((await cdp.send("Performance.getMetrics")).metrics.map((m) => [m.name, m.value]));
  await page.evaluate((m) => window.__bench.setMode(m), mode); await page.waitForTimeout(300);
  const m0 = await metrics(); const w0 = Date.now();
  const r = await page.evaluate(async ([scenario, ms]) => {
    const bn = window.__bench; bn.lat(); bn.applyMs();
    const d = []; let last = performance.now(), run = true; const loop = (t) => { d.push(t - last); last = t; if (run) requestAnimationFrame(loop); }; requestAnimationFrame(loop);
    const long = []; new PerformanceObserver((l) => l.getEntries().forEach((e) => long.push(e.duration))).observe({ entryTypes: ["longtask"] });
    if (scenario === "scroll") await bn.scrollList(ms); else if (scenario === "idle") await new Promise((r) => setTimeout(r, ms)); else { bn.start(scenario); await new Promise((r) => setTimeout(r, ms)); bn.stop(); }
    run = false; await new Promise((r) => setTimeout(r, 100));
    return { ph: bn.ph(), d: d.slice(2), lat: bn.lat(), apply: bn.applyMs(), long, dom: document.querySelectorAll("*").length };
  }, [scenario, scenario === "window" ? 5000 : 3500]);
  const m1 = await metrics(), secs = (Date.now() - w0) / 1000, per = (k) => ((m1[k] - m0[k]) * 1000) / secs;
  rows.push({ ph: r.ph.length ? r.ph.filter((x) => x > 0).length / r.ph.length : 0, phmax: r.ph.length ? Math.max(...r.ph) : 0, task: per("TaskDuration"), script: per("ScriptDuration"), layout: per("LayoutDuration"), style: per("RecalcStyleDuration"), rate, mode, scenario, frames: r.d.length, p50: q(r.d, 0.5), p95: q(r.d, 0.95), p99: q(r.d, 0.99), max: Math.max(...r.d), over: r.d.filter((x) => x > 25).length, lat50: q(r.lat, 0.5), lat95: q(r.lat, 0.95), ap50: q(r.apply, 0.5), ap95: q(r.apply, 0.95), long: r.long.length, dom: r.dom });
  await ctx.close();
}
for (const rate of [1, 4]) {
  const ctx = await b.newContext({ viewport: { width: 1440, height: 900 } }); const page = await ctx.newPage();
  await page.goto(base); await page.waitForFunction(() => window.__bench); await page.waitForTimeout(800);
  const cdp = await ctx.newCDPSession(page); await cdp.send("Emulation.setCPUThrottlingRate", { rate });
  const j = await page.evaluate(() => window.__bench.jumps(30));
  console.log(`jump ${rate}x (scrollbar drag to a far row of 10,000; time until real rows are drawn): p50 ${f1(q(j, 0.5))} ms, p95 ${f1(q(j, 0.95))} ms, max ${f1(Math.max(...j))} ms (a frame is 16.7)`);
  await ctx.close();
}
console.log("rate mode    scenario frames  frame ms p50/p95/p99/max      >25ms  patch-to-paint p50/p95  apply ms p50/p95  longtasks  DOM   main-thread busy ms/s: task script layout style");
for (const r of rows.filter((x) => x.scenario === "scroll")) console.log(`scroll ${r.rate}x: frames with any placeholder row ${(r.ph * 100).toFixed(0)}%, worst frame ${(r.phmax * 100).toFixed(0)}% of rows`);
for (const r of rows) console.log(`${r.rate}x   ${r.mode.padEnd(7)} ${r.scenario.padEnd(8)} ${String(r.frames).padStart(5)}  ${f1(r.p50).padStart(5)} ${f1(r.p95).padStart(5)} ${f1(r.p99).padStart(5)} ${f1(r.max).padStart(6)}   ${String(r.over).padStart(4)}    ${f1(r.lat50).padStart(6)} ${f1(r.lat95).padStart(6)}        ${f1(r.ap50).padStart(5)} ${f1(r.ap95).padStart(5)}       ${String(r.long).padStart(3)}    ${r.dom}   ${f1(r.task).padStart(7)} ${f1(r.script).padStart(6)} ${f1(r.layout).padStart(6)} ${f1(r.style).padStart(5)}`);
await b.close(); srv.close();

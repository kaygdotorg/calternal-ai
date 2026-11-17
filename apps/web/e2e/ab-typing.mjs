// A/B of typing in the long note at 4x CPU between two builds in the same session, alternating runs so machine noise hits both alike.
// Usage: node e2e/ab-typing.mjs <appDirA> <appDirB> [rounds] [path]   (each is an apps/web directory with a built dist; it starts a preview of each; default path /notes/n-long)
import { spawn } from "node:child_process";
import { launch } from "./lib.mjs";
const [DA, DB, R = "8", PATH = "/notes/n-long"] = process.argv.slice(2);
const A = "http://127.0.0.1:4191", B = "http://127.0.0.1:4192";
const servers = [[DA, 4191], [DB, 4192]].map(([cwd, port]) => spawn("npx", ["vite", "preview", "--port", String(port), "--strictPort", "--host", "127.0.0.1"], { cwd, stdio: "ignore", detached: true }));
const stopAll = () => { for (const p of servers) { try { process.kill(-p.pid); } catch { /* gone */ } } };
for (const base of [A, B]) for (let i = 0; i < 60; i++) { try { if ((await fetch(base)).ok) break; } catch { /* not yet */ } await new Promise((r) => setTimeout(r, 250)); }
const b = await launch();
const pct = (xs, p) => { const s = [...xs].sort((a, c) => a - c); return s[Math.min(s.length - 1, Math.floor(p * (s.length - 1)))]; };
const one = async (base) => {
  const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });
  await ctx.addInitScript(() => { try { localStorage.setItem("cal.firstrun", "1"); } catch {} });
  const page = await ctx.newPage(); const cdp = await ctx.newCDPSession(page);
  await page.goto(base + PATH + "?test=1"); await page.waitForSelector(".cm-h2"); await page.waitForTimeout(1200);
  await cdp.send("Emulation.setCPUThrottlingRate", { rate: 4 });
  await page.locator(".cm-content").click(); await page.keyboard.press("Control+End"); await page.waitForTimeout(500);
  await page.evaluate(() => { window.__keys = []; });
  await cdp.send("Performance.enable");
  const m0 = Object.fromEntries((await cdp.send("Performance.getMetrics")).metrics.map((x) => [x.name, x.value]));
  const TEXT = "the quick brown fox jumps over the lazy dog, then naps. ";
  await page.keyboard.type(TEXT, { delay: 60 });
  await page.waitForTimeout(400);
  const m1 = Object.fromEntries((await cdp.send("Performance.getMetrics")).metrics.map((x) => [x.name, x.value]));
  const per = (k) => ((m1[k] - m0[k]) * 1000) / TEXT.length; // ms per key of main-thread time (at 4x): the script, the layout and style work, and the whole task
  busy.push({ script: per("ScriptDuration"), layout: per("LayoutDuration") + per("RecalcStyleDuration"), task: per("TaskDuration") });
  const keys = await page.evaluate(() => window.__keys ?? []);
  await ctx.close();
  return keys;
};
const out = { A: [], B: [] }; let busy = [];
const busyOf = { A: [], B: [] };
for (let i = 0; i < +R; i++) for (const [k, base] of [["A", A], ["B", B]]) { busy = []; out[k].push(await one(base)); busyOf[k].push(busy[0]); }
const bm = (k, f) => { const xs = busyOf[k].map((x) => x[f]).sort((a, c) => a - c); return +xs[xs.length >> 1].toFixed(2); };
const sum = (runs) => { const all = runs.flat(); const med = runs.map((r) => pct(r, 0.5)); return { keysTotal: all.length, medianOfMedians: +pct(med, 0.5).toFixed(1), medianRange: [+Math.min(...med).toFixed(1), +Math.max(...med).toFixed(1)], p95: +pct(all, 0.95).toFixed(1) }; };
console.log(JSON.stringify({ A: { ...sum(out.A), msPerKey: { script: bm("A", "script"), layoutStyle: bm("A", "layout"), task: bm("A", "task") } }, B: { ...sum(out.B), msPerKey: { script: bm("B", "script"), layoutStyle: bm("B", "layout"), task: bm("B", "task") } } }));
await b.close(); stopAll();

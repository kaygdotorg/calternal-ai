// Shell interactions at 4x CPU slowdown: frame times while dragging a sidebar handle, and time to ready plus the frames over budget (20 ms) for a space switch, an app switch and opening Settings.
// Usage: node e2e/shell-perf.mjs [--1x] (after `npm run build`).
import { chromium } from "playwright";
import { serve, BASE, DEVICES } from "./lib.mjs";
const RATE = process.argv.includes("--1x") ? 1 : 4;
const stop = await serve(); const b = await chromium.launch(); const out = { cpuSlowdown: RATE };
const pct = (xs, p) => { const s = [...xs].sort((a, c) => a - c); return s[Math.min(s.length - 1, Math.floor(p * (s.length - 1)))]; };
const fresh = async () => { const ctx = await b.newContext({ ...DEVICES.desktop }); const page = await ctx.newPage(); const cdp = await ctx.newCDPSession(page); await page.goto(BASE + "/threads/t-idle?firstrun=0"); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(1500); if (RATE > 1) await cdp.send("Emulation.setCPUThrottlingRate", { rate: RATE }); await page.addInitScript(""); return { ctx, page }; };
const frames = (page) => page.evaluate(() => { window.__f = []; let last = performance.now(); window.__on = true; const f = (t) => { window.__f.push(t - last); last = t; if (window.__on) requestAnimationFrame(f); }; requestAnimationFrame(f); });
const stopFrames = (page) => page.evaluate(() => { window.__on = false; const f = window.__f.slice(1); return f; });
const sum = (f) => ({ frames: f.length, median: +pct(f, 0.5).toFixed(1), p95: +pct(f, 0.95).toFixed(1), worst: +Math.max(...f).toFixed(1), over20: f.filter((x) => x > 20).length });
// 1. Dragging a sidebar handle.
{
  const { ctx, page } = await fresh();
  const g = await page.getByRole("separator", { name: "Resize the sidebar" }).boundingBox(); const x = g.x + g.width / 2;
  await page.mouse.move(x, 450); await page.mouse.down(); await frames(page);
  for (let i = 1; i <= 50; i++) { await page.mouse.move(x + (i % 2 ? i * 2 : 100 - i), 450); await page.waitForTimeout(16); }
  const f = await stopFrames(page); await page.mouse.up();
  out.resizeDrag = sum(f);
  await ctx.close();
}
// 2. Interactions: time to ready (two frames after the change shows) and frames over budget in the 600 ms after.
const time = async (page, start, ready) => {
  await frames(page);
  const ms = await page.evaluate(async ([ready]) => { const t0 = performance.now(); return new Promise((res) => { const iv = setInterval(() => { if (document.querySelector(ready)) { clearInterval(iv); requestAnimationFrame(() => requestAnimationFrame(() => res(performance.now() - t0))); } }, 1); }); }, [ready]).catch(() => -1);
  return ms;
};
for (const [name, act, ready, back] of [
  ["spaceSwitch", (p) => p.getByRole("radio", { name: "Infra" }).click(), "aside.side .slide:not([data-x])", async (p) => p.getByRole("radio", { name: "Personal" }).click()],
  ["appSwitch", (p) => p.getByRole("tab", { name: "Notes" }).click(), 'nav[aria-label="Notes"] a.row', async (p) => p.getByRole("tab", { name: "Threads" }).click()],
  ["settingsOpen", (p) => p.getByRole("button", { name: /Account and settings/ }).click(), ".page h1", async (p) => p.getByRole("tab", { name: "Threads" }).click()],
]) {
  const runs = [];
  const { ctx, page } = await fresh();
  for (let i = 0; i < 6; i++) {
    await frames(page);
    const t0 = Date.now();
    await act(page);
    const ms = await page.evaluate(async (ready) => { const t0 = performance.now(); await new Promise((res) => { const iv = setInterval(() => { if (document.querySelector(ready)) { clearInterval(iv); res(); } }, 1); }); await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))); return performance.now() - t0; }, ready);
    await page.waitForTimeout(500);
    const f = await stopFrames(page);
    runs.push({ ms, over20: f.filter((x) => x > 20).length, worst: Math.max(...f) });
    await back(page); await page.waitForTimeout(500);
  }
  out[name] = { timeToReadyMs: { median: +pct(runs.map((r) => r.ms), 0.5).toFixed(0), worst: +Math.max(...runs.map((r) => r.ms)).toFixed(0) }, framesOver20msPerRun: runs.map((r) => r.over20), worstFrameMs: +Math.max(...runs.map((r) => r.worst)).toFixed(0) };
  await ctx.close();
}
console.log(JSON.stringify(out, null, 1));
await b.close(); stop();

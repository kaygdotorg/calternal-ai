// Paint and key-to-selection numbers for a long list (hard gate of the lane): a 5,000-row list drawn from the in-page mock host, at 1x and 4x CPU slowdown.
// Measures: the first real row painted after the view opens; Down held for SECS seconds at 30 keys a second: how long each key's handler runs, how long after the key the moved cursor is painted, the
// heartbeat gaps of the main thread, and how many window requests the hold cost. Timings on a shared dev VM are reported, not judged (AGENTS.md): judge on the perf VMs.
// Usage: node e2e/perf.mjs [rows=5000] [seconds=5]
import { launch, open, pct, serve } from "./lib.mjs";
const ROWS = Number(process.argv[2] ?? 5000), SECS = Number(process.argv[3] ?? 5), RATE = 30;
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
for (const cpu of [1, 4]) {
  const { page: p, cdp } = await open(browser, `mock=${ROWS}`);
  await cdp.send("Emulation.setCPUThrottlingRate", { rate: cpu });
  await p.reload(); // so the first paint is measured under the slowdown
  await p.waitForFunction(() => window.__painted !== undefined, null, { timeout: 30000 });
  const painted = await p.evaluate(() => ({ painted: window.__painted, opened: window.__opened, requests: window.__transport.windowCalls }));
  const main = p.locator("[data-node=list] [data-i] button, [data-node=list] [data-i] a").first();
  await main.waitFor();
  await main.focus();
  await p.evaluate(() => {
    const w = window; w.__keys = []; w.__gaps = []; w.__gap = 0;
    const cur = () => document.querySelector("[data-node=list] [data-cursor]")?.getAttribute("data-i") ?? "";
    document.addEventListener("keydown", (e) => { if (e.key !== "ArrowDown") return; const t0 = performance.now(), before = cur(), rec = { t0, before, run: 0, paint: NaN }; w.__keys.push(rec);
      requestAnimationFrame(() => requestAnimationFrame(() => { rec.paint = performance.now() - t0; rec.moved = cur() !== before; })); }, true);
    window.addEventListener("keydown", (e) => { const r = w.__keys[w.__keys.length - 1]; if (r && e.key === "ArrowDown" && !r.run) r.run = performance.now() - r.t0; });
    const ch = new MessageChannel(); let last = performance.now(); ch.port1.onmessage = () => { const t = performance.now(), g = t - last; if (g > w.__gap) w.__gap = g; if (g > 16) w.__gaps.push(g); last = t; setTimeout(() => ch.port2.postMessage(0), 0); }; ch.port2.postMessage(0);
    w.__req0 = w.__transport.windowCalls;
  });
  const n = SECS * RATE, t0 = Date.now();
  for (let i = 0; i < n; i++) { const wait = t0 + (i * 1000) / RATE - Date.now(); if (wait > 0) await p.waitForTimeout(wait); await p.keyboard.press("ArrowDown"); }
  await p.waitForTimeout(600);
  const r = await p.evaluate(() => ({ keys: window.__keys, gaps: window.__gaps, gap: window.__gap, reqs: window.__transport.windowCalls - window.__req0, rows: document.querySelectorAll("[data-node=list] [data-i]").length }));
  const runs = r.keys.map((k) => k.run), paints = r.keys.filter((k) => Number.isFinite(k.paint)).map((k) => k.paint);
  console.log(`TABLE | ${cpu}x | ${ROWS} rows | first rows painted ${painted.painted.toFixed(0)} ms after view open (open ${painted.opened.toFixed(0)} ms, ${painted.requests} window requests) | key handler p95 ${pct(runs, 0.95).toFixed(1)} ms | key to painted selection p50 ${pct(paints, 0.5).toFixed(1)} p95 ${pct(paints, 0.95).toFixed(1)} max ${Math.max(...paints).toFixed(1)} ms | heartbeat gaps over 16 ms: ${r.gaps.length}, max ${r.gap.toFixed(0)} ms | ${r.reqs} window requests in ${SECS} s`);
  ok(r.keys.length >= n * 0.9, `${cpu}x: every key reached the page`, `${r.keys.length} of ${n}`);
  ok(r.keys.filter((k) => k.moved).length >= r.keys.length * 0.9, `${cpu}x: every key moved the selection`, `${r.keys.filter((k) => k.moved).length} moved`);
  ok(r.reqs <= 4 * (SECS + 2), `${cpu}x: window requests stayed within 4 a second`, `${r.reqs} in ${SECS} s`);
  ok(r.rows < 150, `${cpu}x: the DOM holds only a window of the ${ROWS} rows`, `${r.rows} rows in the main list`);
  await p.context().close();
}
await browser.close(); stop();
process.exit(bad ? 1 : 0);

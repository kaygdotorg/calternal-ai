// Churn and scale against the step 0 host's 5,000-row Notes and Threads templates (three mutations a second per app): Down held for SECS seconds at 30 keys a second through the app's own list, at 1x and 4x CPU. Reports
// first rows painted, key to painted selection, how many times a row was a skeleton while the key was held, window requests, and result-version changes seen. The host is reached through two SSH hops, so request latency is
// larger than a real deployment's; timings on this shared VM are reported, not judged. Usage: node e2e/scale.mjs [notes|threads] [seconds=5]
import { readFileSync } from "node:fs";
import { launch, open, pct, serve } from "./lib.mjs";
const HOST = process.env.HOST ?? "http://127.0.0.1:43125/step0/v2";
const bearer = readFileSync(process.env.HOST_BEARER_FILE ?? `${process.env.HOME}/Developer/calternal-ai-lanes/.owner.token`, "utf8").trim();
const view = process.argv[2] ?? "notes", SECS = Number(process.argv[3] ?? 5), RATE = 30;
const LIST = view === "notes" ? "[data-section=notes]" : "[data-sidebar]";
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
for (const cpu of [1, 4]) {
  const { page: p, cdp } = await open(browser, `host=${encodeURIComponent(HOST)}&bearer=${encodeURIComponent(bearer)}&viewName=${view}`);
  await cdp.send("Emulation.setCPUThrottlingRate", { rate: cpu });
  const reqs = []; p.on("request", (r) => { if (r.url().endsWith("/window")) reqs.push(Date.now()); });
  const versions = new Set(); p.on("response", async (r) => { if (r.url().endsWith("/window")) { try { const j = await r.json(); if (j.result_version) versions.add(j.result_version); } catch { /* not json */ } } });
  await p.waitForSelector(`${LIST} [data-i] button`, { timeout: 60000 });
  await p.waitForFunction((sel) => document.querySelectorAll(`${sel} [data-i] .t`).length > 5, LIST, { timeout: 60000 });
  const painted = await p.evaluate(() => window.__painted);
  await p.locator(`${LIST} [data-i] button`).first().focus();
  await p.evaluate((sel) => {
    const w = window; w.__keys = []; w.__skel = 0; w.__gaps = [];
    const cur = () => document.querySelector(`${sel} [data-cursor]`)?.getAttribute("data-i") ?? "";
    document.addEventListener("keydown", (e) => { if (e.key !== "ArrowDown") return; const t0 = performance.now(), before = cur(), rec = { t0, run: 0, paint: NaN }; w.__keys.push(rec);
      requestAnimationFrame(() => requestAnimationFrame(() => { rec.paint = performance.now() - t0; rec.moved = cur() !== before; w.__skel += document.querySelectorAll(`${sel} [data-skeleton]`).length > 0 ? 1 : 0; })); }, true);
    window.addEventListener("keydown", (e) => { const r = w.__keys[w.__keys.length - 1]; if (r && e.key === "ArrowDown" && !r.run) r.run = performance.now() - r.t0; });
    const ch = new MessageChannel(); let last = performance.now(); ch.port1.onmessage = () => { const t = performance.now(), g = t - last; if (g > 16) w.__gaps.push(g); last = t; setTimeout(() => ch.port2.postMessage(0), 0); }; ch.port2.postMessage(0);
  }, LIST);
  const r0 = reqs.length, n = SECS * RATE, t0 = Date.now();
  for (let i = 0; i < n; i++) { const w = t0 + (i * 1000) / RATE - Date.now(); if (w > 0) await p.waitForTimeout(w); await p.keyboard.press("ArrowDown"); }
  await p.waitForTimeout(800);
  const r = await p.evaluate((sel) => ({ keys: window.__keys, skel: window.__skel, gaps: window.__gaps, rows: document.querySelectorAll(`${sel} [data-i]`).length, count: document.querySelector(`${sel} .list`)?.getAttribute("data-count") }), LIST);
  const runs = r.keys.map((k) => k.run), paints = r.keys.filter((k) => Number.isFinite(k.paint)).map((k) => k.paint);
  console.log(`TABLE | ${view} | ${cpu}x | ${r.count} rows | first rows ${painted?.toFixed(0)} ms | handler p95 ${pct(runs, 0.95).toFixed(1)} ms | key to painted p50 ${pct(paints, 0.5).toFixed(1)} p95 ${pct(paints, 0.95).toFixed(1)} max ${Math.max(...paints).toFixed(1)} ms | keys with a skeleton on screen ${r.skel}/${r.keys.length} | heartbeat gaps over 16 ms ${r.gaps.length} | ${reqs.length - r0} window requests in ${SECS} s | ${versions.size} result versions seen`);
  ok(r.keys.length >= n * 0.9, `${view} ${cpu}x: keys delivered`, `${r.keys.length} of ${n}`);
  ok(r.keys.filter((k) => k.moved).length >= r.keys.length * 0.9, `${view} ${cpu}x: every key moved the cursor`);
  ok(reqs.length - r0 <= 4 * (SECS + 2), `${view} ${cpu}x: at most 4 window requests a second`, `${reqs.length - r0}`);
  ok(r.rows < 150, `${view} ${cpu}x: the DOM holds a window of the list`, `${r.rows}`);
  await p.context().close();
}
await browser.close(); stop();
process.exit(bad ? 1 : 0);

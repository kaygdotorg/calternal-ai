// The conversation view at scale, measured the way the list suites are (perf.mjs, scale.mjs): a 5,000-message thread from the fixture host (`?fixture=threads-next&scenario=scale&messages=5000`: the newest 60 messages, then
// a host cursor for every older 60), at 1x and 4x CPU, windowed (only the messages in and near view are drawn, the default); `&nocv` is kept for the browser-skipped comparison, which no longer applies to threads over 120 messages. Reports: the first page painted, how long reading
// every older page takes as the top is reached, the DOM and heap after, then ArrowUp held through the messages at 30 keys a second (handler, key to the focused message painted) and the wheel at 60 events a second (main-thread
// heartbeat gaps, layout and style-recalculation counts). Timings on a shared dev VM are reported, not judged: judge on the perf VMs. Usage: node e2e/threads-scale.mjs [messages=5000] [seconds=5]
import { launch, open, pct, serve } from "./lib.mjs";
const N = Number(process.argv[2] ?? 5000), SECS = Number(process.argv[3] ?? 5), RATE = 30;
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
for (const mode of (process.env.MODES ?? "cv,nocv").split(",")) for (const cpu of [1, 4]) {
  const tag = `${mode === "cv" ? "windowed" : "all drawn"} ${cpu}x`;
  const { page: p, cdp } = await open(browser, `fixture=threads-next&scenario=scale&messages=${N}${mode === "nocv" ? "&nocv=1" : ""}`);
  await cdp.send("Emulation.setCPUThrottlingRate", { rate: cpu });
  await cdp.send("Performance.enable");
  await p.waitForSelector("[data-strip=conv-pins] [data-row-id=t-lunch] button", { timeout: 60000 });
  const t0 = Date.now();
  await p.locator("[data-strip=conv-pins] [data-row-id=t-lunch] button").evaluate((e) => e.click());
  await p.waitForSelector("[data-entry]", { timeout: 60000 });
  const first = Date.now() - t0;
  // read every older page: go to the top as the person would; the view reads one as the top comes near
  const t1 = Date.now(); let pages = 0;
  for (let i = 0; i < 600; i++) {
    if ((await p.locator("[data-older]").count()) === 0 && i > 0) break;
    const before = (await p.evaluate(() => window.__transport.log.filter((l) => l.route === "item_view.open").length));
    await p.locator("[role=log]").evaluate((e) => { e.scrollTop = 0; });
    await p.waitForFunction((k) => window.__transport.log.filter((l) => l.route === "item_view.open").length > k, before, { timeout: 30000 }).catch(() => {});
    pages++;
    await p.waitForTimeout(30);
  }
  const opens = await p.evaluate(() => window.__transport.log.filter((l) => l.route === "item_view.open").length);
  const loaded = Math.min(N, opens * 60), loadMs = Date.now() - t1;
  await p.waitForTimeout(500);
  const dom = await p.evaluate(() => document.querySelectorAll("*").length), drawn = await p.locator("[data-entry]").count();
  // jump to the end from the top (the stall the first run found)
  await p.locator("[role=log]").evaluate((e) => { e.scrollTop = 0; }); await p.waitForTimeout(600);
  await p.evaluate(() => { window.__jg = []; const ch = new MessageChannel(); let last = performance.now(); ch.port1.onmessage = () => { const t = performance.now(); if (t - last > 16) window.__jg.push(t - last); last = t; if (!window.__jstop) setTimeout(() => ch.port2.postMessage(0), 0); }; ch.port2.postMessage(0); });
  const tj = Date.now(); await p.locator("[data-jump-latest]").click().catch(async () => { await p.locator("[role=log]").focus(); await p.keyboard.press("End"); });
  await p.waitForFunction(() => { const e = document.querySelector("[role=log]"); return e.scrollHeight - e.clientHeight - e.scrollTop < 2; }, null, { timeout: 120000 }).catch(() => {});
  await p.waitForTimeout(400);
  const jumpMs = Date.now() - tj - 400, jg = await p.evaluate(() => { window.__jstop = true; return window.__jg; });
  // ArrowUp held through the messages
  await p.locator("[role=log]").focus(); await p.keyboard.press("End"); await p.waitForTimeout(600);
  await p.evaluate(() => {
    const w = window; w.__keys = []; w.__gaps = [];
    const cur = () => document.activeElement?.getAttribute("data-anchor") ?? "";
    document.addEventListener("keydown", (e) => { if (e.key !== "ArrowUp") return; const t0 = performance.now(), before = cur(), rec = { t0, run: 0, paint: NaN }; w.__keys.push(rec); requestAnimationFrame(() => requestAnimationFrame(() => { rec.paint = performance.now() - t0; rec.moved = cur() !== before; })); }, true);
    window.addEventListener("keydown", (e) => { const r = w.__keys[w.__keys.length - 1]; if (r && e.key === "ArrowUp" && !r.run) r.run = performance.now() - r.t0; });
    const ch = new MessageChannel(); let last = performance.now(); ch.port1.onmessage = () => { const t = performance.now(), g = t - last; if (g > 16) w.__gaps.push(g); last = t; setTimeout(() => ch.port2.postMessage(0), 0); }; ch.port2.postMessage(0);
  });
  const n = SECS * RATE, tk = Date.now();
  for (let i = 0; i < n; i++) { const w = tk + (i * 1000) / RATE - Date.now(); if (w > 0) await p.waitForTimeout(w); await p.keyboard.press("ArrowUp"); }
  await p.waitForTimeout(500);
  const r = await p.evaluate(() => ({ keys: window.__keys, gaps: window.__gaps.splice(0) }));
  const runs = r.keys.map((k) => k.run), paints = r.keys.filter((k) => Number.isFinite(k.paint)).map((k) => k.paint);
  // the wheel through the thread
  await p.locator("[role=log]").hover();
  const g0 = await p.evaluate(() => window.__gaps.length), m1 = Object.fromEntries((await cdp.send("Performance.getMetrics")).metrics.map((x) => [x.name, x.value]));
  const tw = Date.now(); for (let i = 0; i < SECS * 60; i++) { const w = tw + (i * 1000) / 60 - Date.now(); if (w > 0) await p.waitForTimeout(w); await p.mouse.wheel(0, -240); }
  await p.waitForTimeout(400);
  const wg = await p.evaluate((g) => window.__gaps.slice(g), g0), m2 = Object.fromEntries((await cdp.send("Performance.getMetrics")).metrics.map((x) => [x.name, x.value]));
  const heap = (m2.JSHeapUsedSize / 1048576).toFixed(0);
  console.log(`TABLE | thread ${N} | ${tag} | first page ${first} ms | ${pages} pages read in ${(loadMs / 1000).toFixed(1)} s (${loaded} messages) | DOM ${dom} nodes (${drawn} messages drawn), heap ${heap} MB | jump top to end ${jumpMs} ms, longest main-thread gap ${Math.max(0, ...jg).toFixed(0)} ms | ArrowUp: handler p95 ${pct(runs, 0.95).toFixed(1)} ms, key to painted p50 ${pct(paints, 0.5).toFixed(1)} p95 ${pct(paints, 0.95).toFixed(1)} max ${Math.max(...paints).toFixed(1)} ms, main-thread gaps >16 ms ${r.gaps.length} (max ${Math.max(0, ...r.gaps).toFixed(0)} ms) | wheel: gaps >16 ms ${wg.length} (max ${Math.max(0, ...wg).toFixed(0)} ms), layouts ${m2.LayoutCount - m1.LayoutCount}, style recalcs ${m2.RecalcStyleCount - m1.RecalcStyleCount}`);
  ok(loaded >= N, `${tag}: every message was reachable by reading older pages`, `${loaded} of ${N}`);
  ok(drawn < 150, `${tag}: the DOM holds only the messages in and near view`, `${drawn} of ${N} drawn`);
  ok(jumpMs < 5000, `${tag}: a jump from the top to the end settles`, `${jumpMs} ms`);
  ok(r.keys.length >= n * 0.9, `${tag}: every key reached the page`, `${r.keys.length} of ${n}`);
  ok(r.keys.filter((k) => k.moved).length >= r.keys.length * 0.9, `${tag}: every key moved the focus to the previous message`);
  await p.context().close();
}
await browser.close(); stop();
process.exit(bad ? 1 : 0);

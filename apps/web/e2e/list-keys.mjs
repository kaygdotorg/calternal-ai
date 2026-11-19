// List key repeat on the flood (owner requirement, 2026-11-19): in a list of rows, holding Down spam-scrolls through every entry and the app is fast enough to draw it, with a whole new thread or note
// on the main pane after each rest. Runs on the flood fixtures (`?flood`, 5,000 threads and 5,000 notes with varied titles, heavy transcripts, states and sections; lib/flood.ts) with the churn stream off
// and on (`?churn`: several changes a second), once for the Threads sidebar (All Threads) and once for Notes, with the CPU slowed 4x. Down is held for five seconds at 30 keys per second. Measured: how long the
// keydown handler runs, how long after each key the moved selection is painted, the heartbeat gaps (how long the main thread was away from other work), how many times the main pane was asked to load during the hold
// (none: only when the selection settles), and how long after the last key the main pane shows the settled item (budget 150 ms). Also Page Down, End and Home. Timings are judged on a quiet machine (the quiet
// list). Usage: node e2e/list-keys.mjs [rows=5000] [seconds=5] [surfaces=threads,notes] [churn=0,1]
import { judgeTimings, launch, open, serve } from "./lib.mjs";
const ROWS = Number(process.argv[2] ?? 5000), SECS = Number(process.argv[3] ?? 5), RATE = 30, FRAME = 16.7, SHOW_BUDGET = 150;
const RATE_CPU = Number(process.env.CAL_CPU ?? 4);
const SURFACES = (process.argv[4] ?? "threads,notes").split(","), CHURNS = (process.argv[5] ?? "0,1").split(",").map(Number);
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const judge = judgeTimings();
const timing = (c, m, note) => { if (judge) ok(c, m, note); else console.log(`SKIP  ${m}: not judged, the machine is busy  ${note}`); };
/** Threads under a slowed CPU, and the Notes heartbeat check with churn on (second known defect, issue #15: p95 84 and 49 ms against about 41 in two of three quiet runs), are owner-approved known defects (issue #15): its numbers are reported, tagged KNOWN, and do not block; every 1x check, every Notes check and the single-load check block. */
const KNOWN = "https://git.kayg.org/kayg/calternal-ai/issues/15";
const timingFor = (surface, churn) => (c, m, note) => ((surface === "threads" || (surface === "notes" && churn && /main thread is free again/.test(m))) && RATE_CPU > 1 ? (console.log(`${c ? "PASS" : "KNOWN"}  ${m}  ${note}${c ? "" : `  (known defect, not blocking: ${KNOWN})`}`)) : timing(c, m, note));
const pct = (xs, q) => { const s = [...xs].sort((a, b) => a - b); return s.length ? s[Math.min(s.length - 1, Math.floor(s.length * q))] : NaN; };
for (const surface of SURFACES) for (const churn of CHURNS) {
  const tag = `${surface}, churn ${churn ? "on" : "off"}`;
  const tm = timingFor(surface, churn);
  console.log(`-- ${tag}: ${ROWS} ${surface}, Down held ${SECS} s at ${RATE} per second, CPU ${RATE_CPU}x slower`);
  const path = surface === "threads" ? `/threads?flood=${ROWS}${churn ? "&churn=4" : ""}` : `/notes?flood=${ROWS}${churn ? "&churn=4" : ""}`;
  const { page: p, ctx } = await open(browser, "desktop", "dark", path);
  const nav = surface === "threads" ? "nav[aria-label=Threads]" : "nav[aria-label=Notes]";
  if (surface === "threads") { await p.getByRole("radio", { name: "All Threads" }).click().catch(() => p.getByText("All Threads").first().click()); await p.waitForTimeout(500); }
  const first = p.locator(`${nav} a.row`).first();
  await first.waitFor();
  await p.evaluate((sel) => document.querySelector(`${sel} a.row`)?.focus(), nav); // not a locator: churn may replace the row while a locator waits for it to be stable
  await p.evaluate(() => {
    const w = window; w.__keys = []; w.__gap = 0; w.__gaps = []; w.__routes = 0; w.__loaf = [];
    try { new PerformanceObserver((l) => { for (const e of l.getEntries()) w.__loaf.push([e.duration, e.scripts.reduce((a, x) => a + x.duration, 0)]); }).observe({ type: "long-animation-frame" }); } catch { /* none */ }
    const cur = () => document.activeElement?.closest("a,button")?.getAttribute("href") ?? "";
    document.addEventListener("keydown", (e) => { if (e.key !== "ArrowDown") return; const t0 = performance.now(), before = cur(), rec = { t0, before, run: 0, paint: NaN }; w.__keys.push(rec);
      requestAnimationFrame(() => requestAnimationFrame(() => { rec.paint = performance.now() - t0; rec.moved = cur() !== before; })); }, true);
    window.addEventListener("keydown", (e) => { const r = w.__keys[w.__keys.length - 1]; if (r && e.key === "ArrowDown" && !r.run) r.run = performance.now() - r.t0; });
    const rs = history.pushState; history.pushState = function (...a) { w.__routes++; return rs.apply(this, a); };
    const ch = new MessageChannel(); let last = performance.now(); ch.port1.onmessage = () => { const t = performance.now(), g = t - last; if (g > w.__gap) w.__gap = g; if (g > 16) w.__gaps.push(g); last = t; setTimeout(() => ch.port2.postMessage(0), 0); }; w.__beat = () => { last = performance.now(); ch.port2.postMessage(0); };
  });
  const cdp = await ctx.newCDPSession(p);
  await cdp.send("Emulation.setCPUThrottlingRate", { rate: RATE_CPU });
  await p.evaluate(() => { window.__path0 = location.pathname; window.__beat(); });
  const n = SECS * RATE, t0 = Date.now();
  for (let i = 0; i < n; i++) { const due = t0 + (i * 1000) / RATE; const wait = due - Date.now(); if (wait > 0) await p.waitForTimeout(wait); await p.keyboard.press("ArrowDown"); }
  const tLast = Date.now();
  const startPath = await p.evaluate(() => window.__path0);
  const during = await p.evaluate(() => ({ routes: window.__routes, gap: window.__gap, gaps: window.__gaps.slice(), loaf: window.__loaf.length }));
  // the selection at the last key: its address and, where the row has one, its title; the pane must show it
  const want = await p.evaluate(() => { const a = document.activeElement?.closest("a"); return { href: a?.getAttribute("href") ?? "", title: (a?.textContent ?? "").trim() }; });
  let shown = NaN; // the pane has the item the selection settled on: the address left where the hold began and the page's heading is the row's title (under churn the row's item may have moved on, so the address is not compared)
  for (let k = 0; k < 400 && Number.isNaN(shown); k++) {
    const there = await p.evaluate(([p0, threads]) => location.pathname !== p0 && (threads ? document.documentElement.dataset.threadRendered === location.pathname.split("/").pop() : !!document.querySelector(".note, main h1, h1")), [startPath, surface === "threads"]); // threads: the thread's own stamp (calternal:thread-rendered), the same one the native shell reads
    if (there) shown = Date.now() - tLast; else await p.waitForTimeout(10);
  }
  await p.waitForTimeout(400);
  const r = await p.evaluate(() => ({ keys: window.__keys, routes: window.__routes, focus: document.activeElement?.closest("nav")?.getAttribute("aria-label") ?? document.activeElement?.tagName }));
  await cdp.send("Emulation.setCPUThrottlingRate", { rate: 1 });
  const stampMs = surface === "threads" ? await p.evaluate(() => performance.getEntriesByName("calternal:thread:open").at(-1)?.duration ?? NaN) : NaN;
  const runs = r.keys.map((k) => k.run), paints = r.keys.filter((k) => Number.isFinite(k.paint)).map((k) => k.paint);
  console.log(`TABLE | ${RATE_CPU}x | ${surface} | churn ${churn ? "on" : "off"} | stamp ${Number.isNaN(stampMs) ? "-" : stampMs.toFixed(0)} ms | last key to pane ${shown} ms | paint p95 ${pct(paints, 0.95).toFixed(1)} ms | heartbeat p95 ${(during.gaps.length ? pct(during.gaps, 0.95) : 0).toFixed(0)} ms (max ${during.gap.toFixed(0)})`);
  console.log(`   ${r.keys.length} keys: handler p50 ${pct(runs, 0.5).toFixed(1)} p95 ${pct(runs, 0.95).toFixed(1)} max ${Math.max(...runs).toFixed(1)} ms; paint after key p50 ${pct(paints, 0.5).toFixed(1)} p95 ${pct(paints, 0.95).toFixed(1)} max ${Math.max(...paints).toFixed(1)} ms; heartbeat gaps over 16 ms: ${during.gaps.length}, p95 ${(during.gaps.length ? pct(during.gaps, 0.95) : 0).toFixed(0)} ms, max ${during.gap.toFixed(0)} ms; main pane shows the settled item ${shown} ms after the last key; ${during.routes} loads during the hold`);
  ok(r.keys.length >= n * 0.9, `${tag}: every key was delivered to the page`, `${r.keys.length} of ${n}`);
  ok(r.keys.filter((k) => k.moved).length >= r.keys.length * 0.9, `${tag}: every key moved the selection`, `${r.keys.filter((k) => k.moved).length} moved`);
  ok(/Threads|Notes/.test(String(r.focus)), `${tag}: focus stayed in the list`, String(r.focus));
  ok(during.routes <= 1, `${tag}: the main pane was not asked to load while the key was held (only when the selection settled)`, `${during.routes} loads during the hold`);
  ok(!Number.isNaN(shown), `${tag}: the main pane shows the settled item`, `${startPath} -> ${await p.evaluate(() => location.pathname)}`);
  if (surface === "threads") { const openMs = await p.evaluate(() => performance.getEntriesByName("calternal:thread:open").at(-1)?.duration ?? NaN); console.log(`   the thread's own stamp (calternal:thread:open, view made to first screenful drawn): ${openMs.toFixed(0)} ms at 4x`); tm(openMs <= SHOW_BUDGET, `${tag}: the settled thread is drawn within ${SHOW_BUDGET} ms of its view being made (the shell's stamp, 4x CPU; the 50 ms rest comes before it)`, `${openMs.toFixed(0)} ms`); }
  tm(shown <= SHOW_BUDGET, `${tag}: the main pane shows the settled item within ${SHOW_BUDGET} ms of the last key`, `${shown} ms`);
  tm(pct(runs, 0.95) <= FRAME, `${tag}: the handler's p95 is within one frame`, `${pct(runs, 0.95).toFixed(1)} ms of ${FRAME}`);
  tm(pct(paints, 0.95) <= 2 * FRAME + 8, `${tag}: the moved selection is painted within one frame of each key (p95, to the second frame after the key)`, `${pct(paints, 0.95).toFixed(1)} ms`);
  tm((during.gaps.length === 0 || pct(during.gaps, 0.95) <= FRAME * 2 + 8) && during.gaps.filter((g) => g > 100).length <= 1, `${tag}: while the key is held the main thread is free again within about two frames (p95 of the heartbeat gaps, at most one over 100 ms)`, `p95 ${(during.gaps.length ? pct(during.gaps, 0.95) : 0).toFixed(0)} ms, max ${during.gap.toFixed(0)} ms`);
  // (not under churn: the stream may insert a row above, so the neighbour can be the open item itself)
  if (!churn) {
  // the settled item at 1x: one more key with the CPU at full speed, the pane must show the new item within the budget (the 4x figure above is for a slow device and only reported)
  {
    const before = await p.evaluate(() => location.pathname);
    const fo = await p.evaluate(() => document.activeElement?.tagName + " " + (document.activeElement?.getAttribute("href") ?? ""));
    await p.keyboard.press("ArrowDown");
    const t1 = Date.now(); let at1 = NaN;
    for (let k = 0; k < 300 && Number.isNaN(at1); k++) { if ((await p.evaluate(() => location.pathname)) !== before) at1 = Date.now() - t1; else await p.waitForTimeout(5); }
    if (Number.isNaN(at1)) console.log("   debug:", JSON.stringify(await p.evaluate(() => ({ path: location.pathname, cur: document.querySelector("[data-cursor]")?.getAttribute("aria-posinset"), act: document.activeElement?.getAttribute("href"), actI: document.activeElement?.closest("[data-i]")?.getAttribute("data-i"), current: document.querySelector("[aria-current]")?.getAttribute("href") }))));
    timing(at1 <= SHOW_BUDGET, `${tag}: at full speed the main pane shows the settled item within ${SHOW_BUDGET} ms of the last key`, `${at1} ms (focus was on ${fo})`);
  }
  }
  if (!churn) {
    await p.waitForTimeout(1500); // the settled thread's own load must be over before the jump keys are read
    const at = async () => { let prev = -2; for (let k = 0; k < 40; k++) { const v = await p.evaluate(() => { const e = document.activeElement?.closest("[data-i]"); return e ? Number(e.getAttribute("data-i")) : -1; }); if (v === prev) return v; prev = v; await p.waitForTimeout(80); } return prev; };
    await p.keyboard.press("End"); const last = await at();
    await p.keyboard.press("Home"); const top = await at();
    await p.keyboard.press("PageDown"); const pg = await at();
    ok(last > 40 && top === 0 && pg > 3 && pg < 80, `${tag}: End, Home and Page Down move to the last loaded row, the first and a page on`, `last ${last}, home ${top}, page ${pg}`);
  }
  await ctx.close();
}
await browser.close(); stop();
console.log(bad ? `${bad} failed` : "list keys ok");
process.exit(bad ? 1 : 0);

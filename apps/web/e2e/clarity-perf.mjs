// Dragging "Clear to tinted" must stay smooth. Statistical, not one lucky or unlucky run: on a fresh page (the first step's cost included), at 4x CPU throttling, the slider is moved through all five steps by keyboard (up and
// down) and dragged along its rail, FIVE times per colour scheme (CAL_PERF_SAMPLES). Each sample records the longest stretch the main thread was blocked (a heartbeat; the long-task observer sees nothing under 50 ms) and the
// slowest step on the screen. The checks pass when the MEDIAN longest long task is under 40 ms (that is, no task reaches the observer\'s 50 ms) with at most one sample over 50 ms; the median heartbeat stretch is under 110 ms with at most one over 150 ms (the heartbeat sees the frame work as well, about 65 to 90 ms on a quiet dev-02, so it is a guard against the 300 ms kind); the median step is under 110 ms with at most one over 150 ms; every sample is printed, and a
// sample taken while the host gave more than 1 percent of the CPU to another VM (steal time) is printed but not judged (fewer than three judged samples: not judged at all). The blurred wallpaper copies must never be rebuilt (their blob
// URLs stay the same) and the live preview must settle on the veil the slider left. A step used to re-blur the picture and re-solve everything (hundreds of milliseconds at 4x). Usage: node e2e/clarity-perf.mjs (after `npm run build`, through with-preview).
import { chromium } from "playwright";
import { BASE, STEAL_LIMIT, busyNow, judgeSamples, judgeTimings, serve, stealWindow } from "./lib.mjs";
const stop = await serve(); const b = await chromium.launch(); let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const q = (a, f) => { const s = [...a].sort((x, y) => x - y); return s.length ? s[Math.min(s.length - 1, Math.floor(s.length * f))] : 0; };
const N = Number(process.env.CAL_PERF_SAMPLES ?? 5);
/** One sample: a fresh page (so the first step's cost is in it), the slider moved through all five steps by keyboard and then dragged along its rail at 4x CPU. Returns the longest blocked stretch of the main thread (a heartbeat's largest gap: a long-task observer sees nothing under 50 ms), the slowest step on screen, whether the blurred copies were rebuilt, and whether the preview settled. */
async function sample(scheme) {
  const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, colorScheme: scheme });
  await ctx.addInitScript(() => localStorage.setItem("cal.firstrun", "1"));
  const page = await ctx.newPage();
  await page.goto(`${BASE}/settings/appearance/appearance.clarity?firstrun=0${process.env.CAL_PERF_QS ?? ""}`); await page.waitForSelector("html[data-ready]");
  await page.waitForFunction(() => getComputedStyle(document.documentElement).getPropertyValue("--wall-side").includes("blob"), null, { timeout: 20000 }); await page.waitForTimeout(800);
  const slider = page.getByRole("slider", { name: /Clear to tinted/ }); await slider.scrollIntoViewIfNeeded();
  const walls = () => page.evaluate(() => ["--wall-side", "--wall-card"].map((k) => getComputedStyle(document.documentElement).getPropertyValue(k)).join("|"));
  const step = async (key) => { const t = await page.evaluate(() => performance.now()); await page.keyboard.press(key); return page.evaluate((t0) => new Promise((res) => requestAnimationFrame(() => requestAnimationFrame(() => res(performance.now() - t0)))), t); };
  const before = await walls();
  const cdp = await ctx.newCDPSession(page); await cdp.send("Emulation.setCPUThrottlingRate", { rate: 4 });
  // The heartbeat: a timer chain at the browser's minimum interval; the largest gap between two beats, less that interval, is the longest the main thread was blocked.
  await page.evaluate(() => { window.__long = []; new PerformanceObserver((l) => l.getEntries().forEach((e) => window.__long.push(e.duration))).observe({ entryTypes: ["longtask"] }); });
  await page.evaluate(() => { window.__gap = 0; let last = performance.now(); const beat = () => { const n = performance.now(); window.__gap = Math.max(window.__gap, n - last - 4); last = n; setTimeout(beat, 0); }; setTimeout(beat, 0); });
  const w = stealWindow();
  await slider.focus(); const lat = [];
  lat.push(await step("Home"));
  for (let i = 0; i < 4; i++) lat.push(await step("ArrowRight"));
  for (let i = 0; i < 4; i++) lat.push(await step("ArrowLeft"));
  const r = await slider.boundingBox();
  await page.mouse.move(r.x + 4, r.y + r.height / 2); await page.mouse.down();
  for (let i = 1; i <= 20; i++) { await page.mouse.move(r.x + 4 + ((r.width - 8) * i) / 20, r.y + r.height / 2); await page.waitForTimeout(16); }
  await page.mouse.up(); await page.waitForTimeout(500);
  const steal = w.end();
  const gap = await page.evaluate(() => window.__gap), lt = Math.max(0, ...(await page.evaluate(() => window.__long))), after = await walls();
  // The live preview wears its own copy of the veil (so a step restyles it in a frame of its own, not the step's): once the slider rests it must match the root's.
  await page.waitForTimeout(400);
  const synced = await page.evaluate(() => { const rs = document.documentElement.style, pv = document.querySelector(".pv"); if (!pv) return "none"; const names = [...rs].filter((n) => /^--(?:[lcr]-(?:tint|dim)(?:-a)?|veil-[lcr])$/.test(n)); return names.length && names.every((n) => pv.style.getPropertyValue(n) === rs.getPropertyValue(n)) ? "same" : `differs: ${names.filter((n) => pv.style.getPropertyValue(n) !== rs.getPropertyValue(n)).join(",")}`; });
  await ctx.close();
  return { gap, lt, lat: Math.max(...lat), p50: q(lat, 0.5), steal, notJudged: steal > STEAL_LIMIT, same: before === after, synced };
}
for (const scheme of ["dark", "light"]) {
  const samples = []; for (let i = 0; i < N; i++) samples.push(await sample(scheme));
  console.log(`   ${scheme}: step p50 ${samples.map((x) => x.p50.toFixed(0)).join(" ")} ms`);
  ok(samples.every((x) => x.same), `${scheme}: the blurred wallpaper copies are not rebuilt (same blob URLs before and after, in every sample)`);
  ok(samples.every((x) => x.synced === "same" || x.synced === "none"), `${scheme}: the live preview shows the veil the slider settled on`, [...new Set(samples.map((x) => x.synced))].join(","));
  const judge = judgeTimings();
  const rows = (key) => samples.map((x) => ({ value: x[key], notJudged: x.notJudged, steal: x.steal }));
  const l = judgeSamples(`${scheme}: longest main-thread task (long-task observer, 0 when none reaches 50 ms) over the slider's five steps and a drag, 4x CPU`, rows("lt"), { bound: 40, hard: 50 });
  const a = judgeSamples(`${scheme}: longest blocked stretch of the main thread (heartbeat, includes frame work), 4x CPU`, rows("gap"), { bound: 110, hard: 150 });
  const c = judgeSamples(`${scheme}: slowest step on the screen, 4x CPU`, rows("lat"), { bound: 110, hard: 150 });
  for (const j of [l, a, c]) {
    if (!judge || j.pass === null) console.log(`SKIP  ${j.text}  (not judged: ${judge ? "too few quiet samples" : "the machine is busy: " + busyNow().why})`);
    else ok(j.pass, j.text);
  }
}
await b.close(); stop(); process.exit(bad ? 1 : 0);

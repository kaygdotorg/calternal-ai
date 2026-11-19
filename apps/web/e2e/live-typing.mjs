// Typing alone in a note costs no more with live editing on than off: a keystroke to paint (the editor's probe, ?test=1) in the 50,000-word note at 4x CPU slowdown, the same note and keys, once over HTTP saves
// and once in a room (the fixture room with nobody else in it, ?live=1&peer=0). Quiet-list suite: it measures time, so it runs alone on a quiet machine (CAL_QUIET).
// Usage: node e2e/with-preview.mjs e2e/live-typing.mjs
import { chromium } from "playwright";
import { serve, open, DEVICES, BASE } from "./lib.mjs";
const stop = await serve(); const b = await chromium.launch();
const pct = (xs, p) => { const s = [...xs].sort((a, c) => a - c); return s[Math.min(s.length - 1, Math.floor(p * (s.length - 1)))]; };
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
async function measure(query) {
  const ctx = await b.newContext({ ...DEVICES.desktop });
  await ctx.addInitScript(() => { try { localStorage.setItem("cal.firstrun", "1"); } catch {} });
  const page = await ctx.newPage(); const cdp = await ctx.newCDPSession(page);
  await cdp.send("Emulation.setCPUThrottlingRate", { rate: 4 });
  await page.goto(`${BASE}/notes/n-long?test=1&${query}`);
  await page.waitForSelector(".cm-h2", { timeout: 20000 }); await page.waitForTimeout(1200);
  await page.locator(".cm-content").click(); await page.keyboard.press("Control+End"); await page.waitForTimeout(500);
  // 40 characters to warm the page up (not measured), then 240 measured: enough keystrokes that one load's median is stable to well under a millisecond.
  await page.keyboard.type("the quick brown fox jumps over the lazy d", { delay: 40 });
  await page.evaluate(() => { window.__keys = []; });
  await page.keyboard.type("og, then naps. the quick brown fox jumps over the lazy dog, then naps. ".repeat(4).slice(0, 240), { delay: 40 });
  await page.waitForTimeout(700);
  const keys = await page.evaluate(() => window.__keys ?? []);
  await ctx.close();
  return { n: keys.length, median: pct(keys, 0.5), p95: pct(keys, 0.95) };
}
if (process.env.CAL_SELF) {
  // The measurement judged by itself: "off" against "off", interleaved, ten times. Until it passes its own gate every time the suite cannot judge a difference.
  const a = [], c = [];
  for (let i = 0; i < Number(process.env.CAL_PAIRS ?? 10); i++) { const x = await measure("live=0"), y = await measure("live=0"); a.push(x); c.push(y); }
  const spread = (xs) => +(Math.max(...xs) - Math.min(...xs)).toFixed(2);
  const pass = a.filter((x, i) => c[i].median <= Math.max(x.median * 1.25, x.median + 3) && c[i].p95 <= Math.max(x.p95 * 1.3, x.p95 + 6)).length;
  console.log(JSON.stringify({ off_medians: a.concat(c).map((x) => +x.median.toFixed(1)), off_median_spread_ms: spread(a.concat(c).map((x) => x.median)), pair_diff_max_ms: +Math.max(...a.map((x, i) => Math.abs(x.median - c[i].median))).toFixed(2), off_vs_off_pairs_passing_gate: `${pass}/${a.length}` }));
  await b.close(); stop(); process.exit(0);
}
try {
  // Three independent page loads of each, interleaved: one load's median varies by a millisecond or more on its own (the page, the machine), so the comparison is between the medians of the three.
  const offs = [], ons = [];
  for (let i = 0; i < 3; i++) { offs.push(await measure("live=0")); ons.push(await measure("live=1&peer=0")); }
  const mid = (xs, k) => pct(xs.map((x) => x[k]), 0.5);
  const off = { n: Math.min(...offs.map((x) => x.n)), median: mid(offs, "median"), p95: mid(offs, "p95") };
  const on = { n: Math.min(...ons.map((x) => x.n)), median: mid(ons, "median"), p95: mid(ons, "p95") };
  console.log(JSON.stringify({ off, on, runs: { off: offs.map((x) => +x.median.toFixed(1)), on: ons.map((x) => +x.median.toFixed(1)) } }));
  ok(on.n > 200 && off.n > 200, "both runs recorded the keystrokes", `${off.n} / ${on.n}`);
  ok(on.median <= Math.max(off.median * 1.25, off.median + 3), "live: the median keystroke costs no more than without", `${on.median.toFixed(1)} vs ${off.median.toFixed(1)} ms`);
  ok(on.p95 <= Math.max(off.p95 * 1.3, off.p95 + 6), "live: the p95 keystroke costs no more than without", `${on.p95.toFixed(1)} vs ${off.p95.toFixed(1)} ms`);
} catch (e) { console.log("FAIL  " + e.message); bad++; }
await b.close(); stop();
process.exit(bad ? 1 : 0);

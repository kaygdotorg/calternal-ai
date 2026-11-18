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
  await page.evaluate(() => { window.__keys = []; });
  await page.keyboard.type("the quick brown fox jumps over the lazy dog, then naps. ", { delay: 60 });
  await page.waitForTimeout(600);
  const keys = await page.evaluate(() => window.__keys ?? []);
  await ctx.close();
  return { n: keys.length, median: pct(keys, 0.5), p95: pct(keys, 0.95) };
}
try {
  const off = await measure("live=0");
  const on = await measure("live=1&peer=0");
  console.log(JSON.stringify({ off, on }));
  ok(on.n > 20 && off.n > 20, "both runs recorded the keystrokes", `${off.n} / ${on.n}`);
  ok(on.median <= Math.max(off.median * 1.25, off.median + 3), "live: the median keystroke costs no more than without", `${on.median.toFixed(1)} vs ${off.median.toFixed(1)} ms`);
  ok(on.p95 <= Math.max(off.p95 * 1.3, off.p95 + 6), "live: the p95 keystroke costs no more than without", `${on.p95.toFixed(1)} vs ${off.p95.toFixed(1)} ms`);
} catch (e) { console.log("FAIL  " + e.message); bad++; }
await b.close(); stop();
process.exit(bad ? 1 : 0);

// A/B of composer typing at 4x CPU between two builds in one session, alternating runs. Reports, over all runs, the first key, the key that opens the picker, and key-to-next-frame p50/p95.
// Usage: node e2e/ab-composer.mjs <appDirA> <appDirB> [rounds]
import { spawn } from "node:child_process";
import { launch } from "./lib.mjs";
const [DA, DB, R = "8"] = process.argv.slice(2);
const urls = { A: "http://127.0.0.1:4191", B: "http://127.0.0.1:4192" };
const servers = [[DA, 4191], [DB, 4192]].map(([cwd, port]) => spawn("npx", ["vite", "preview", "--port", String(port), "--strictPort", "--host", "127.0.0.1"], { cwd, stdio: "ignore", detached: true }));
const stopAll = () => { for (const p of servers) { try { process.kill(-p.pid); } catch { /* gone */ } } };
for (const base of Object.values(urls)) for (let i = 0; i < 60; i++) { try { if ((await fetch(base)).ok) break; } catch { /* not yet */ } await new Promise((r) => setTimeout(r, 250)); }
const b = await launch();
const LINE = "the quick brown fox jumps over the lazy dog, then naps @Priya friday 5pm. ";
const pct = (xs, p) => { const s = [...xs].sort((a, c) => a - c); return s[Math.min(s.length - 1, Math.floor(p * (s.length - 1)))]; };
const one = async (base) => {
  const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });
  await ctx.addInitScript(() => { try { localStorage.setItem("cal.firstrun", "1"); } catch {} });
  const page = await ctx.newPage(); const cdp = await ctx.newCDPSession(page);
  await page.goto(base + "/threads/t-idle?test=1"); await page.waitForSelector("#composer-field"); await page.waitForTimeout(1200);
  await cdp.send("Emulation.setCPUThrottlingRate", { rate: 4 });
  const f = page.locator("#composer-field"); await f.click();
  await f.pressSequentially(LINE, { delay: 60 }); await page.waitForTimeout(300);
  const keys = await page.evaluate(() => window.__keys ?? []);
  await ctx.close();
  return keys;
};
const runs = { A: [], B: [] };
for (let i = 0; i < +R; i++) for (const k of ["A", "B"]) runs[k].push(await one(urls[k]));
const sum = (rs) => ({ firstKey: +pct(rs.map((r) => r[0]), 0.5).toFixed(1), atKey: +pct(rs.map((r) => r[LINE.indexOf("@")]), 0.5).toFixed(1), firstLetterAfterAt: +pct(rs.map((r) => r[LINE.indexOf("@") + 1]), 0.5).toFixed(1), median: +pct(rs.flat(), 0.5).toFixed(1), p95: +pct(rs.flat(), 0.95).toFixed(1), p95PerRunMedian: +pct(rs.map((r) => pct(r, 0.95)), 0.5).toFixed(1) });
const slow = (rs) => { const c = {}; rs.forEach((r) => r.forEach((v, i) => { if (v > 8) c[`${JSON.stringify(LINE[i])}@${i}`] = (c[`${JSON.stringify(LINE[i])}@${i}`] ?? 0) + 1; })); return Object.entries(c).sort((a, b) => b[1] - a[1]).slice(0, 8).map(([k, n]) => `${k}:${n}/${rs.length}`).join(" "); };
console.log(JSON.stringify({ A: sum(runs.A), B: sum(runs.B) }));
console.log("keys over 8 ms, A:", slow(runs.A), "\nkeys over 8 ms, B:", slow(runs.B));
await b.close(); stopAll();

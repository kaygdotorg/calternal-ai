// Keystroke to paint in the composer with the editor package's probe (?test=1), at 4x CPU slowdown: 80 characters typed at about 16 keys a second.
import { chromium } from "playwright";
import { serve, open } from "./lib.mjs";
const stop = await serve(); const b = await chromium.launch();
const { page, ctx } = await open(b, "desktop", "light", "/threads/t-idle?test=1");
await page.waitForSelector("#composer-field");
const cdp = await ctx.newCDPSession(page);
await cdp.send("Emulation.setCPUThrottlingRate", { rate: 4 });
const f = page.locator("#composer-field");
await f.click();
const LINE = "the quick brown fox jumps over the lazy dog, then naps @Priya friday 5pm. ";
await f.pressSequentially(LINE, { delay: 60 });
await page.waitForTimeout(300);
const keys = await page.evaluate(() => window.__keys ?? []);
const work = await page.evaluate(() => window.__work ?? []);
const sw = [...work].sort((a, c) => a - c);
const steady = work.filter((w, i) => i > 0 && !"@#/".includes(LINE[i]) && !"@#/".includes(LINE[i - 1] ?? "")); // plain typing: not the first key, not the keys that open a picker
const ss = [...steady].sort((a, c) => a - c);
const qs = (p) => ss[Math.min(ss.length - 1, Math.floor(p * (ss.length - 1)))];
const qw = (p) => sw[Math.min(sw.length - 1, Math.floor(p * (sw.length - 1)))];
const s = [...keys].sort((a, c) => a - c);
const q = (p) => s[Math.min(s.length - 1, Math.floor(p * (s.length - 1)))];
if (process.env.SLOW_KEYS) console.log("slow keys:", work.map((w, i) => (w > 8 ? `${JSON.stringify(LINE[i])}@${i}=${w.toFixed(1)}` : null)).filter(Boolean).join(" "));
console.log(JSON.stringify({ cpuSlowdown: 4, steady: { keys: ss.length, median: +qs(0.5).toFixed(1), p95: +qs(0.95).toFixed(1) }, work: { median: +qw(0.5).toFixed(1), p95: +qw(0.95).toFixed(1), worst: +sw[sw.length - 1].toFixed(1) }, keys: keys.length, median: +q(0.5).toFixed(1), p95: +q(0.95).toFixed(1), worst: +s[s.length - 1].toFixed(1) }));
await b.close(); stop();

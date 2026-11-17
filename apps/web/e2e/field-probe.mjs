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
await f.pressSequentially("the quick brown fox jumps over the lazy dog, then naps @Priya friday 5pm. ".repeat(1), { delay: 60 });
await page.waitForTimeout(300);
const keys = await page.evaluate(() => window.__keys ?? []);
const s = [...keys].sort((a, c) => a - c);
const q = (p) => s[Math.min(s.length - 1, Math.floor(p * (s.length - 1)))];
console.log(JSON.stringify({ cpuSlowdown: 4, keys: keys.length, median: +q(0.5).toFixed(1), p95: +q(0.95).toFixed(1), worst: +s[s.length - 1].toFixed(1) }));
await b.close(); stop();

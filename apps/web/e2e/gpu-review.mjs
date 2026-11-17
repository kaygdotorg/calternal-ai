// A set of shell screenshots from a headed, GPU-backed Chrome (backdrop-filter drawn) for the design review. Usage: BASE=http://host/ai node gpu-review.mjs <outDir>
import { chromium } from "playwright-core";
import { mkdirSync } from "node:fs";
const BASE = process.env.BASE, OUT = process.argv[2]; mkdirSync(OUT, { recursive: true });
const b = await chromium.connectOverCDP("http://127.0.0.1:9222"); const ctx = await b.newContext();
const shot = async (name, w, h, scheme, path, act) => {
  const page = await ctx.newPage(); const cdp = await ctx.newCDPSession(page);
  await cdp.send("Emulation.setDeviceMetricsOverride", { width: w, height: h, deviceScaleFactor: 2, mobile: w < 500, hasTouch: w < 500 });
  await page.emulateMedia({ colorScheme: scheme });
  await page.goto(BASE + path); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(2200);
  if (act) await act(page);
  await page.waitForTimeout(700);
  await page.screenshot({ path: `${OUT}/${name}.png` });
  await page.close();
};
await shot("d-dark-threads", 1500, 970, "dark", "/threads/t-idle?firstrun=0");
await shot("d-light-threads", 1500, 970, "light", "/threads/t-idle?firstrun=0");
await shot("d-dark-notes", 1500, 970, "dark", "/notes/n-1?firstrun=0");
await shot("d-light-settings", 1500, 970, "light", "/settings/appearance?firstrun=0");
await shot("d-dark-settings-compare", 1500, 970, "dark", "/settings/appearance?firstrun=0", async (p) => { await p.evaluate(() => document.querySelector(".scroller").scrollTop = 1400); });
await shot("d-dark-closed", 1500, 970, "dark", "/threads/t-idle?firstrun=0", async (p) => { await p.getByRole("button", { name: "Hide the sidebar" }).click(); await p.getByRole("button", { name: "Hide the side panel" }).click(); });
await shot("d-dark-firstrun", 1500, 970, "dark", "/threads/t-idle");
await shot("d-dark-popover", 1500, 970, "dark", "/threads/t-idle?firstrun=0", async (p) => { await p.getByRole("button", { name: /Least access/ }).click(); });
await shot("p-light-threads", 390, 844, "light", "/threads/t-idle?firstrun=0");
await shot("p-light-side", 390, 844, "light", "/threads/t-idle?firstrun=0", async (p) => { await p.getByRole("button", { name: "Show threads" }).click(); });
await shot("p-dark-settings", 390, 844, "dark", "/settings/appearance?firstrun=0");
console.log("ok");
await b.close();

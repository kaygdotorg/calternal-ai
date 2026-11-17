// The documentation site on the preview host: light, dark and a preset, plus a size and speed check. Usage: CAL_DOCS=http://127.0.0.1/docs/ node e2e/docs-shots.mjs <outdir>
import { mkdirSync } from "node:fs";
import { chromium } from "playwright";
const base = process.env.CAL_DOCS ?? "http://127.0.0.1/docs/";
const out = process.argv[2] ?? "/tmp/docs-shots"; mkdirSync(out, { recursive: true });
const b = await chromium.launch();
const run = async (name, scheme, preset) => {
  const ctx = await b.newContext({ viewport: { width: 1280, height: 800 }, colorScheme: scheme, deviceScaleFactor: 1 });
  const page = await ctx.newPage();
  if (preset) await ctx.addInitScript((p) => localStorage.setItem("cal-docs-preset", p), preset);
  const t0 = Date.now();
  await page.goto(base + "user/spaces-and-roles/");
  await page.waitForLoadState("load");
  const ms = Date.now() - t0;
  await page.evaluate(() => document.fonts.ready);
  const fam = await page.evaluate(() => getComputedStyle(document.body).fontFamily);
  const bytes = await page.evaluate(() => performance.getEntriesByType("resource").reduce((s, r) => s + (r.transferSize || 0), 0));
  console.log(`${name}: ${ms} ms to load, ${(bytes / 1024).toFixed(0)} KiB transferred, font ${fam.split(",")[0]}`);
  await page.screenshot({ path: `${out}/docs-${name}.png` });
  await ctx.close();
};
await run("light", "light");
await run("dark", "dark");
await run("nord-dark", "dark", "nord");
await run("gruvbox-light", "light", "gruvbox");
await b.close();

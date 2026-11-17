// The right sidebar's three lists and its tabs, cropped. Usage: node e2e/panels-shots.mjs <outdir>
import { mkdirSync } from "node:fs";
import { chromium } from "playwright";
import { open, serve } from "./lib.mjs";
const out = process.argv[2] ?? "/tmp/panels"; mkdirSync(out, { recursive: true });
const stop = await serve(); const b = await chromium.launch();
for (const [scheme, panel] of [["light", "changes"], ["dark", "changes"], ["light", "files"], ["light", "run"]]) {
  const { page, ctx } = await open(b, "desktop", scheme, `/threads/t-run?panel=${panel}`);
  await page.waitForTimeout(700);
  await page.screenshot({ path: `${out}/panel-${panel}-${scheme}.png`, clip: { x: 1060, y: 0, width: 380, height: 900 } });
  await ctx.close();
}
await b.close(); stop();

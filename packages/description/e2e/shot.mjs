// Screenshot of the harness for a query string: node e2e/shot.mjs "mock=40&fixture=reading-list" out.png [cpu] [WxH]
import { launch, open, serve } from "./lib.mjs";
const [query, out, cpu = "1", size = "1100x720"] = process.argv.slice(2);
const [width, height] = size.split("x").map(Number);
const stop = await serve(); const b = await launch();
const { page, cdp } = await open(b, query, { width, height });
await cdp.send("Emulation.setCPUThrottlingRate", { rate: Number(cpu) });
await page.waitForFunction(() => window.__painted !== undefined, null, { timeout: 20000 }).catch(() => {});
await page.waitForTimeout(500);
await page.screenshot({ path: out });
await b.close(); stop();

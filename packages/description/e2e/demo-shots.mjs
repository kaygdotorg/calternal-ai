// Screenshots of the running demo (default http://127.0.0.1:4312): node e2e/demo-shots.mjs outdir [width x height]
import { mkdirSync } from "node:fs";
import { launch } from "./lib.mjs";
const [out = "/tmp/demo", size = "1280x800"] = process.argv.slice(2); mkdirSync(out, { recursive: true });
const [width, height] = size.split("x").map(Number), BASE = process.env.DEMO ?? "http://127.0.0.1:4312";
const b = await launch();
for (const v of ["reading", "notes", "threads"]) {
  const ctx = await b.newContext({ viewport: { width, height } }); const p = await ctx.newPage();
  await p.goto(`${BASE}/app/?host=/api&viewName=${v}`);
  await p.waitForSelector("[data-i]", { timeout: 30000 }).catch(() => {});
  await p.waitForTimeout(3500);
  await p.screenshot({ path: `${out}/${v}.png` });
  await ctx.close();
}
await b.close();

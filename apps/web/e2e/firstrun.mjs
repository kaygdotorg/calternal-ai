// The first-run preset gallery: shown on a fresh browser and with ?firstrun=1; choosing a preset applies it and ends the first run; Keep the default ends it too.
import { chromium } from "playwright";
import { serve, BASE } from "./lib.mjs";
const stop = await serve(); const b = await chromium.launch();
let bad = 0;
const ok = (n, p) => { console.log(`${p ? "PASS" : "FAIL"}  ${n}`); if (!p) bad++; };
{ // a fresh browser
  const ctx = await b.newContext({ viewport: { width: 1280, height: 800 } }); const page = await ctx.newPage();
  await page.goto(BASE + "/threads"); await page.waitForSelector("html[data-ready]");
  const dlg = page.getByRole("dialog", { name: "Choose a look" });
  await dlg.waitFor();
  ok("a fresh browser sees the gallery", true);
  ok("it offers every preset", (await dlg.getByRole("radio").count()) >= 10);
  await dlg.getByRole("radio", { name: "Nord" }).click();
  await page.waitForTimeout(500);
  ok("choosing a preset closes the sheet", (await page.getByRole("dialog", { name: "Choose a look" }).count()) === 0);
  const st = await page.evaluate(() => ({ app: JSON.parse(localStorage.getItem("cal.appearance") ?? "{}"), fr: localStorage.getItem("cal.firstrun") }));
  ok("the preset is stored", st.app.presetId === "nord");
  ok("the first run is over", st.fr === "1");
  await page.reload(); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(500);
  ok("it does not come back on reload", (await page.getByRole("dialog", { name: "Choose a look" }).count()) === 0);
  await page.goto(BASE + "/threads?firstrun=1"); await page.waitForSelector("html[data-ready]");
  ok("?firstrun=1 shows it again", (await page.getByRole("dialog", { name: "Choose a look" }).count()) === 1);
  await page.getByRole("button", { name: "Keep the default" }).click(); await page.waitForTimeout(300);
  ok("Keep the default closes it and drops the parameter", (await page.getByRole("dialog", { name: "Choose a look" }).count()) === 0 && !page.url().includes("firstrun"));
  await ctx.close();
}
await b.close(); stop(); process.exit(bad ? 1 : 0);

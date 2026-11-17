// Contrast on every built-in wallpaper in both schemes (axe color-contrast, a real browser): the sidebars sit straight on the picture, so
// their ink must stay legible whatever it is. The wash derived from each wallpaper's brightness (packages/ui `washFor`) is what makes this pass.
// Usage: node e2e/wallpapers.mjs   (after `npm run build`). Exits 1 on any violation.
import { mkdirSync, readFileSync } from "node:fs";
import AxeBuilder from "@axe-core/playwright";
import { launch, open, serve } from "./lib.mjs";

const IDS = ["haze", "dusk", "sage", "slate", "dune", "lagoon", "moss", "orchid", "fbV03MdVtJA", "BnOdWOMW7Is", "Nkwrpa_v0WA", "B8zfbse7mOc", "_zDcx4dD7L0", "EX_STserBZM", "ngWVYP9NIYM", "I_xBR4NmDhU"];
const GRID = process.env.GRID; // a directory: also writes a screenshot per wallpaper and scheme, and a contact grid per scheme (grid-light.png, grid-dark.png)
const stop = await serve();
const browser = await launch();
let bad = 0;
try {
  for (const scheme of ["light", "dark"]) for (const id of IDS) {
    const { page, ctx } = await open(browser, "desktop", scheme, "/threads/t-run?panel=changes");
    await page.evaluate((w) => localStorage.setItem("cal.appearance", JSON.stringify({ wallpaperId: w })), id);
    await page.reload();
    await page.waitForSelector("html[data-ready]");
    await page.waitForFunction(() => getComputedStyle(document.documentElement).getPropertyValue("--c-dim-a").trim() !== "" && getComputedStyle(document.documentElement).getPropertyValue("--wall-card").includes("blob"), null, { timeout: 15000 });
    await page.waitForTimeout(300);
    const wash = await page.evaluate(() => { const cs = getComputedStyle(document.documentElement); const t = (k) => 1 - (1 - +cs.getPropertyValue(`--${k}-tint-a`)) * (1 - +cs.getPropertyValue(`--${k}-dim-a`)); return `veil left ${t("l").toFixed(2)} card ${t("c").toFixed(2)} right ${t("r").toFixed(2)}`; });
    const r = await new AxeBuilder({ page }).withRules(["color-contrast"]).analyze();
    const v = r.violations.flatMap((x) => x.nodes.map((n) => `${n.target.join(" ")} ${n.any[0]?.message?.slice(0, 90)}`));
    console.log(`${v.length ? "FAIL" : "ok  "} ${scheme} ${id} (${wash})${v.length ? "\n   " + v.slice(0, 4).join("\n   ") : ""}`);
    bad += v.length;
    if (GRID) { mkdirSync(GRID, { recursive: true }); await page.screenshot({ path: `${GRID}/${scheme}-${id}.png` }); }
    await ctx.close();
  }
  if (GRID) for (const scheme of ["light", "dark"]) {
    const { page, ctx } = await open(browser, "desktop", scheme, "/threads");
    const cells = IDS.map((id) => `<figure><img src="data:image/png;base64,${readFileSync(`${GRID}/${scheme}-${id}.png`).toString("base64")}"><figcaption>${id}</figcaption></figure>`).join("");
    await page.setViewportSize({ width: 1800, height: 1000 });
    await page.evaluate((h) => { document.body.innerHTML = `<style>html,body{background:#fff;overflow:auto;height:auto;visibility:visible}body{display:grid;grid-template-columns:repeat(4,1fr);gap:8px;padding:8px;font:12px monospace}figure{margin:0}img{width:100%;display:block;border-radius:6px}</style>${h}`; document.documentElement.style.visibility = "visible"; }, cells);
    await page.waitForTimeout(800);
    await page.screenshot({ path: `${GRID}/grid-${scheme}.png`, fullPage: true });
    await ctx.close();
  }
} finally {
  await browser.close();
  stop();
}
process.exit(bad ? 1 : 0);

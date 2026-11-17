// Contrast on every built-in wallpaper in both schemes (axe color-contrast, a real browser): the sidebars sit straight on the picture, so
// their ink must stay legible whatever it is. The wash derived from each wallpaper's brightness (packages/ui `washFor`) is what makes this pass.
// Usage: node e2e/wallpapers.mjs   (after `npm run build`). Exits 1 on any violation.
import AxeBuilder from "@axe-core/playwright";
import { launch, open, serve } from "./lib.mjs";

const IDS = ["dune", "lagoon", "aurora", "ember", "orchid", "moss", "slate", "paper"];
const stop = await serve();
const browser = await launch();
let bad = 0;
try {
  for (const scheme of ["light", "dark"]) for (const id of IDS) {
    const { page, ctx } = await open(browser, "desktop", scheme, "/threads/t-run?panel=changes");
    await page.evaluate((w) => localStorage.setItem("cal.appearance", JSON.stringify({ wallpaperId: w })), id);
    await page.reload();
    await page.waitForSelector("html[data-ready]");
    await page.waitForFunction(() => getComputedStyle(document.documentElement).getPropertyValue("--wash").trim() !== "", null, { timeout: 8000 });
    await page.waitForTimeout(300);
    const wash = await page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue("--wash").trim());
    const r = await new AxeBuilder({ page }).withRules(["color-contrast"]).analyze();
    const v = r.violations.flatMap((x) => x.nodes.map((n) => `${n.target.join(" ")} ${n.any[0]?.message?.slice(0, 90)}`));
    console.log(`${v.length ? "FAIL" : "ok  "} ${scheme} ${id} (wash ${wash})${v.length ? "\n   " + v.slice(0, 4).join("\n   ") : ""}`);
    bad += v.length;
    await ctx.close();
  }
} finally {
  await browser.close();
  stop();
}
process.exit(bad ? 1 : 0);

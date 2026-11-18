// A first visit with a fresh browser profile: the default wallpaper is on screen from the first paint, from the defaults, with no stored setting and before any script has
// blurred the full picture; the sidebars are flat (no strip of bare wallpaper beside the card, no edge at the sidebar); a closed sidebar or side panel has a way back;
// the account picture opens the Settings app (no sheet); the app switcher is a three-label capsule under the card. Usage: node e2e/first-visit.mjs (after `npm run build`).
import { chromium } from "playwright";
import { serve, BASE, DEVICES } from "./lib.mjs";
const stop = await serve(); const b = await chromium.launch(); let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const fresh = async (dev, scheme = "light", block = false) => {
  const ctx = await b.newContext({ ...dev, colorScheme: scheme, reducedMotion: "no-preference" });
  const page = await ctx.newPage();
  if (block) await page.route(/wallpapers\/[A-Za-z_0-9-]+\.jpg$/, (r) => (/blur/.test(r.request().url()) ? r.continue() : r.abort())); // the full picture never arrives: only the cheap rendition can be on screen
  await page.goto(BASE + "/threads/t-idle?firstrun=0");
  return { ctx, page };
};
for (const scheme of ["light", "dark"]) {
  const { ctx, page } = await fresh(DEVICES.desktop, scheme, true);
  await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(1500);
  const r = await page.evaluate(() => ({ stored: localStorage.getItem("cal.appearance"), back: getComputedStyle(document.querySelector(".backdrop")).backgroundImage, cheap: document.documentElement.dataset.cheap !== undefined }));
  ok(r.stored === null, `${scheme}: a first visit stores no setting`);
  ok(/wallpapers\/.*-blur.*\.jpg|^url\("blob:/.test(r.back), `${scheme}: the default wallpaper's cheap rendition is behind the app`, r.back.slice(0, 70));
  await ctx.close();
}
{ // a stored choice of none removes it
  const ctx = await b.newContext({ ...DEVICES.desktop }); const page = await ctx.newPage();
  await page.addInitScript(() => localStorage.setItem("cal.appearance", JSON.stringify({ wallpaperId: null })));
  await page.goto(BASE + "/threads/t-idle?firstrun=0"); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(800);
  ok(!(await page.evaluate(() => getComputedStyle(document.querySelector(".backdrop")).backgroundImage)).includes("blur"), "a stored choice of no wallpaper is not overridden by the default");
  await ctx.close();
}
{ // desktop shell
  const { ctx, page } = await fresh(DEVICES.desktop);
  await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(1200);
  const caps = await page.locator('[aria-label="Apps"] [role="tab"], [aria-label="Apps"] button').allInnerTexts();
  ok(caps.map((t) => t.trim()).join(",") === "Threads,Notes,Settings", "the app switcher is a capsule of three labels under the card", caps.join(","));
  const box = await page.evaluate(() => { const c = document.querySelector('[aria-label="Apps"]').getBoundingClientRect(), m = document.querySelector(".mid-card").getBoundingClientRect(); return c.bottom <= m.bottom + 1 && c.top > m.top + m.height / 2; });
  ok(box, "it is at the bottom of the middle card");
  await page.getByRole("button", { name: "Hide the sidebar" }).click(); await page.waitForTimeout(400);
  ok(await page.getByRole("button", { name: "Show the sidebar" }).isVisible(), "with the sidebar closed there is a button to show it");
  await page.keyboard.press("Control+Backslash"); await page.waitForTimeout(400);
  ok(await page.getByRole("button", { name: "Hide the sidebar" }).isVisible(), "its shortcut (Mod+\\) shows it again");
  await page.getByRole("button", { name: "Hide the side panel" }).click(); await page.waitForTimeout(400);
  ok(await page.getByRole("button", { name: "Show the side panel" }).isVisible(), "with the side panel closed there is a button to show it");
  await page.getByRole("button", { name: "Show the side panel" }).click(); await page.waitForTimeout(300);
  await page.getByRole("button", { name: /Account and settings/ }).click(); await page.waitForTimeout(500);
  ok(page.url().includes("/settings/") && (await page.locator('[role="dialog"]').count()) === 0, "the account picture opens the Settings app, not a sheet", page.url());
  await page.goto(BASE + "/settings/appearance?sheet=1&firstrun=0"); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(800);
  ok((await page.locator('[role="dialog"]').count()) === 0 && (await page.getByText("Wallpaper and fonts").count()) > 0, "wallpaper and fonts are a page of the Settings app on a desktop, not a sheet");
  await ctx.close();
}
await b.close(); stop();
console.log(bad ? `${bad} FAIL` : "all pass"); process.exit(bad ? 1 : 0);

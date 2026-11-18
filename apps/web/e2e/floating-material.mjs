// One material for every floating surface (docs/ui-design-system.md "One material for floating surfaces"): a context menu, a popover/picker menu, a sheet (the palette), a tooltip and the server-link banner are each the shared
// glass (an element with the `.glass` class, backdrop blur, no background of the surface's own) that the app tints from what is really behind it (`--mat-a`, written by packages/ui lib/material.ts from the source the shell registers),
// in both schemes. The check looks at the structure and the written values; the readability floor itself is proven on pixels by the veil tests and e2e/seam.mjs.
import { chromium } from "playwright";
import { BASE, expandComposer, serve } from "./lib.mjs";
const stop = await serve(); const b = await chromium.launch(); let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
for (const scheme of ["light", "dark"]) {
  const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, colorScheme: scheme }); await ctx.addInitScript(() => localStorage.setItem("cal.firstrun", "1"));
  const page = await ctx.newPage(); await page.goto(`${BASE}/threads/t-run`); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(1500);
  const surface = (name, sel) => page.evaluate(([sel]) => { const e = document.querySelector(sel); if (!e) return null; const g = e.matches(".glass") ? e : e.querySelector(":scope > .glass") ?? e.querySelector(".glass"); const host = e.matches("[style*='--mat-a']") ? e : e.querySelector("[style*='--mat-a']") ?? e.closest("[style*='--mat-a']") ?? e; const gs = g ? getComputedStyle(g) : null; return { glass: !!g, blur: gs ? /blur/.test(gs.backdropFilter || gs.webkitBackdropFilter || "") : false, a: parseFloat(host.style.getPropertyValue("--mat-a") || getComputedStyle(host).getPropertyValue("--mat-a")), ownBg: getComputedStyle(e.matches(".glass") ? e.parentElement : e).backgroundColor }; }, [sel]);
  const check = (name, r) => ok(!!r && r.glass && r.blur && r.a > 0, `${scheme}: ${name} is the shared glass, tinted from what is behind it`, r ? `--mat-a ${r.a}, own background ${r.ownBg}` : "not found");
  // a context menu over the sidebar
  await page.locator("aside.side a.row").first().click({ button: "right" }); await page.waitForTimeout(500);
  check("a context menu", await surface("menu", '[role="menu"], .pop [role=menu]'));
  await page.keyboard.press("Escape"); await page.waitForTimeout(300);
  // a picker over the card
  await expandComposer(page); await page.locator(".composer .chip[data-chip=model] .pill").click(); await page.waitForTimeout(600);
  check("a picker (popover)", await surface("popover", '[role="dialog"].pop, .pop[data-radius]'));
  await page.keyboard.press("Escape"); await page.waitForTimeout(300);
  // the palette sheet
  await page.keyboard.press("Control+k"); await page.waitForTimeout(700);
  check("a sheet (the palette)", await surface("sheet", '.sheet[role="dialog"]'));
  await page.keyboard.press("Escape"); await page.waitForTimeout(300);
  // a tooltip
  await page.locator("aside.side button, aside.side [role=button]").first().hover(); await page.waitForTimeout(900);
  check("a tooltip", await surface("tooltip", ".warm-tip .wt-surface"));
  await ctx.close();
}
await b.close(); stop(); process.exit(bad ? 1 : 0);

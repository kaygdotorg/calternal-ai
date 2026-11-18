// The inset sheet on a phone (ported from the first app, lib/sheet-radius.ts): a bottom sheet sits 8 in from the sides and the bottom, its top corners 40 and its bottom corners concentric with the display's
// (the iPhone 17 Pro's 61 less the inset = 53). Elsewhere (no `?sheetcorners=device`, not an iPhone) the pane radius all round. Usage: node e2e/inset-sheet.mjs (after `npm run build`, through with-preview).
import { chromium } from "playwright";
import { BASE, serve } from "./lib.mjs";
const stop = await serve(); const b = await chromium.launch(); let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const sheet = async (q) => {
  const ctx = await b.newContext({ viewport: { width: 402, height: 874 }, screen: { width: 402, height: 874 }, deviceScaleFactor: 3, isMobile: true, hasTouch: true });
  await ctx.addInitScript(() => localStorage.setItem("cal.firstrun", "1"));
  const page = await ctx.newPage(); await page.goto(`${BASE}/threads/t-run${q}`); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(600);
  await page.keyboard.press("Control+k"); await page.waitForSelector(".sheet.bottom[data-ready]"); await page.waitForTimeout(500);
  const m = await page.evaluate(() => { const e = document.querySelector(".sheet.bottom"), r = e.getBoundingClientRect(), c = getComputedStyle(e); return { l: r.left, r: innerWidth - r.right, b: innerHeight - r.bottom, tl: parseFloat(c.borderTopLeftRadius), tr: parseFloat(c.borderTopRightRadius), br: parseFloat(c.borderBottomRightRadius), bl: parseFloat(c.borderBottomLeftRadius) }; });
  await ctx.close(); return m;
};
const d = await sheet("?sheetcorners=device");
ok(d.l === 8 && d.r === 8 && d.b === 8, "the sheet sits 8 in from the sides and the bottom", JSON.stringify(d));
ok(d.tl === 40 && d.tr === 40 && d.br === 53 && d.bl === 53, "top corners 40, bottom corners the display's 61 less the inset", JSON.stringify(d));
const u = await sheet("");
ok([u.tl, u.tr, u.br, u.bl].every((x) => x === 28) && u.l === 8, "without an iPhone the corners are the pane radius all round (28)", JSON.stringify(u));
await b.close(); stop();
console.log(bad ? `${bad} FAIL` : "all pass"); process.exit(bad ? 1 : 0);

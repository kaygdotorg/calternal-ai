// On a phone a menu is a dropdown anchored to its button; only a menu with a search field (or a long list) is an inset sheet (docs/ui-design-system.md "Menus and sheets"). Also the side panel is the list's mirror.
import { chromium } from "playwright";
import { BASE, serve } from "./lib.mjs";
const stop = await serve(); const b = await chromium.launch(); let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const ctx = await b.newContext({ viewport: { width: 390, height: 844 }, deviceScaleFactor: 2, isMobile: true, hasTouch: true });
await ctx.addInitScript(() => localStorage.setItem("cal.firstrun", "1"));
const page = await ctx.newPage(); await page.goto(`${BASE}/threads/t-run`); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(700);
await page.getByRole("button", { name: "Thread actions" }).tap(); await page.waitForTimeout(500);
const m = await page.evaluate(() => { const e = document.querySelector('[role=menu]'); const t = document.querySelector('button[aria-label="Thread actions"]').getBoundingClientRect(); const r = e?.getBoundingClientRect(); return { menu: !!e, sheet: !!document.querySelector(".sheet.bottom"), nearButton: r ? Math.abs(r.right - t.right) < 60 && r.top >= t.top : false }; });
ok(m.menu && !m.sheet && m.nearButton, "the three-dot menu is a dropdown at its button, not a sheet", JSON.stringify(m));
await page.keyboard.press("Escape"); await page.waitForTimeout(300);
const w = 390;
await page.getByRole("button", { name: "Show the side panel" }).tap(); await page.waitForTimeout(600);
const g = await page.evaluate(() => { const mid = new DOMMatrixReadOnly(getComputedStyle(document.querySelector("main.mid")).transform).m41; const rs = document.querySelector(".right > .rs").getBoundingClientRect(); return { mid, left: rs.left, right: rs.right, scrim: !!document.querySelector(".scrim") }; });
ok(g.mid < -250 && g.right >= w - 1 && !g.scrim, "the side panel opens under the card, which slides left (no overlay, no scrim)", JSON.stringify(g));
await b.close(); stop();
console.log(bad ? `${bad} FAIL` : "all pass"); process.exit(bad ? 1 : 0);

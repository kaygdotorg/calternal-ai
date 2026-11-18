// Safe areas (docs/ui-shell.md "Safe areas"). A browser on a laptop has no notch, so the insets are faked through the same `--safe-*` tokens the shell reads from `env()`: portrait (status bar 47, home indicator 34) and
// landscape (notch 47 each side, indicator 21). The picture is painted under all of it, the content is padded by it, the browser's bars take the page's top colour (`theme-color`), a home-screen app is as tall as
// the probe says, and the keyboard lifts the shell.
import { chromium } from "playwright";
import { serve, BASE } from "./lib.mjs";
const stop = await serve(); const b = await chromium.launch(); let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const INSETS = { portrait: { w: 390, h: 844, top: 47, right: 0, bottom: 34, left: 0 }, landscape: { w: 844, h: 390, top: 0, right: 47, bottom: 21, left: 47 } };
for (const [orient, i] of Object.entries(INSETS)) for (const scheme of ["light", "dark"]) {
  const ctx = await b.newContext({ viewport: { width: i.w, height: i.h }, deviceScaleFactor: 3, isMobile: true, hasTouch: true, colorScheme: scheme });
  await ctx.addInitScript((i) => { localStorage.setItem("cal.firstrun", "1"); const css = `html:root{--safe-top:${i.top}px !important;--safe-right:${i.right}px !important;--safe-bottom:${i.bottom}px !important;--safe-left:${i.left}px !important}`; const add = () => document.head.appendChild(Object.assign(document.createElement("style"), { textContent: css })); if (document.head) add(); else document.addEventListener("DOMContentLoaded", add); }, i);
  const page = await ctx.newPage(); await page.goto(`${BASE}/threads/t-run`); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(1200);
  const r = await page.evaluate(() => { const bd = document.querySelector(".backdrop").getBoundingClientRect(), sh = document.querySelector(".shell"), cs = getComputedStyle(sh), card = document.querySelector(".mid-card").getBoundingClientRect(), hd = document.querySelector(".mid header")?.getBoundingClientRect(); return { bdTop: bd.top, bdBottom: bd.bottom, bdLeft: bd.left, bdRight: bd.right, padL: parseFloat(cs.paddingLeft), padR: parseFloat(cs.paddingRight), cardLeft: card.left, cardRight: card.right, hdTop: hd?.top ?? null, theme: document.querySelector('meta[name="theme-color"]').getAttribute("content"), iw: innerWidth, ih: innerHeight }; });
  const tag = `${orient} ${scheme}`;
  ok(r.bdTop === 0 && r.bdBottom === r.ih && r.bdLeft === 0 && r.bdRight === r.iw, `${tag}: the picture is painted under the status bar, the notch and the home indicator (the backdrop fills the screen)`);
  ok(r.padL === i.left && r.padR === i.right, `${tag}: the shell pads its regions by the side insets`, `${r.padL}/${r.padR}`);
  ok(r.cardLeft >= i.left - 0.5 && r.cardRight <= r.iw - i.right + 0.5, `${tag}: the card stays between the notches`, `${r.cardLeft.toFixed(0)}..${r.cardRight.toFixed(0)} of ${r.iw}`);
  if (r.hdTop !== null) ok(r.hdTop >= i.top - 0.5, `${tag}: the card's header sits below the status bar`, `${r.hdTop.toFixed(0)} >= ${i.top}`);
  ok(/^#[0-9a-f]{6}$/.test(r.theme) && r.theme !== (scheme === "dark" ? "#1f1f1f" : "#f7f7f7"), `${tag}: the browser's bars take the page's top colour (theme-color)`, r.theme);
  await ctx.close();
}
{
  // A home-screen app: marked, sized by the probe; and the keyboard lifts the shell.
  const ctx = await b.newContext({ viewport: { width: 390, height: 780 }, deviceScaleFactor: 3, isMobile: true, hasTouch: true });
  await ctx.addInitScript(() => { localStorage.setItem("cal.firstrun", "1"); globalThis.__homeScreen = true; });
  const page = await ctx.newPage(); await page.goto(`${BASE}/threads/t-run`); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(800);
  const r = await page.evaluate(() => ({ hs: document.documentElement.hasAttribute("data-home-screen"), vh: document.documentElement.style.getPropertyValue("--vh"), shellH: document.querySelector(".shell").getBoundingClientRect().height }));
  ok(r.hs && parseFloat(r.vh) >= 780, "a home-screen app is marked and sized by the probe", `--vh ${r.vh}, shell ${r.shellH}`);
  await page.evaluate(() => document.documentElement.style.setProperty("--kb", "300px")); await page.waitForTimeout(100);
  const lifted = await page.evaluate(() => document.querySelector(".shell").getBoundingClientRect().height);
  ok(Math.abs(lifted - (parseFloat(r.vh) - 300)) < 1, "the keyboard lifts the shell by its height (--kb)", `${lifted} vs ${parseFloat(r.vh) - 300}`);
  await ctx.close();
}
await b.close(); stop(); process.exit(bad ? 1 : 0);

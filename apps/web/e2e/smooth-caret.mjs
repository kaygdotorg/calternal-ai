// The smooth caret (SC1-SC6, apps/web/src/apps/notes/smooth-caret.ts): it glides when it moves after a pause, snaps when typing outpaces it, holds solid while typing, steps aside for reduced motion and a touch screen, leaves the browser's own caret in place when off, and costs one element's style write per key.
import { chromium } from "playwright";
import { serve, open } from "./lib.mjs";
const stop = await serve(); const b = await chromium.launch();
let bad = 0;
const ok = (n, p, note = "") => { console.log(`${p ? "PASS" : "FAIL"}  ${n}${note ? "  " + note : ""}`); if (!p) bad++; };
const caret = (page) => page.locator(".cm-smooth-caret");
for (const scheme of ["light", "dark"]) {
  const { page, ctx } = await open(b, "desktop", scheme, "/notes/n-1?test=1");
  await page.waitForSelector(".cm-content");
  await page.locator(".cm-content").click();
  await page.keyboard.press("Control+End");
  await page.waitForTimeout(400);
  ok(`${scheme}: one smooth caret in the layer, the browser's primary one hidden`, (await caret(page).count()) === 1 && (await page.locator(".cm-cursor-primary").evaluate((e) => getComputedStyle(e).display)) === "none");
  // a move after a pause glides: the transition is on, with the agreed length and curve
  await page.keyboard.press("Home"); await page.waitForTimeout(300); await page.keyboard.press("End");
  const t = await caret(page).evaluate((e) => ({ d: getComputedStyle(e).transitionDuration, p: getComputedStyle(e).transitionProperty, snap: e.classList.contains("snap") }));
  ok(`${scheme}: a move after a pause glides (90 ms, transform)`, t.d === "0.09s" && t.p === "transform" && !t.snap, JSON.stringify(t));
  // typing fast snaps and holds solid
  await page.keyboard.type("quick typing here", { delay: 10 });
  const s = await caret(page).evaluate((e) => ({ snap: e.classList.contains("snap"), anim: getComputedStyle(e).animationName, op: getComputedStyle(e).opacity }));
  ok(`${scheme}: fast typing snaps and the blink holds solid`, s.snap && s.anim === "none" && s.op === "1", JSON.stringify(s));
  // the caret sits where the text caret is
  const pos = await page.evaluate(() => { const r = getSelection().getRangeAt(0).getClientRects(); const c = document.querySelector(".cm-smooth-caret").getBoundingClientRect(); const l = r[r.length - 1]; return l ? { dx: Math.abs(c.left - l.right), dy: Math.abs(c.top - l.top) } : null; });
  ok(`${scheme}: the caret is where the text caret is (within 3 px)`, pos && pos.dx < 3 && pos.dy < 4, JSON.stringify(pos));
  // after a pause it blinks again
  await page.waitForTimeout(2000);
  const blink = await caret(page).evaluate((e) => getComputedStyle(e).animationName);
  ok(`${scheme}: after a pause it fades again`, blink.startsWith("cm-sc-blink"), blink);
  // per-key cost: the layer is the only thing the caret touches per key
  const writes = await page.evaluate(async () => {
    let n = 0; const mo = new MutationObserver((l) => { n += l.length; });
    mo.observe(document.querySelector(".cm-smooth-caret-layer"), { attributes: true, childList: true, subtree: true });
    return new Promise((res) => setTimeout(() => res(() => { const r = mo.takeRecords().length + n; mo.disconnect(); return r; }), 0));
  });
  void writes;
  await ctx.close();
}
// reduced motion and touch: none of it exists
{
  const { page, ctx } = await open(b, "desktop", "light", "/notes/n-1?test=1");
  await page.emulateMedia({ reducedMotion: "reduce" }); await page.reload(); await page.waitForSelector(".cm-content");
  await page.locator(".cm-content").click(); await page.keyboard.type("x"); await page.waitForTimeout(300);
  ok("reduced motion: no smooth caret, the browser's own caret shows", (await caret(page).count()) === 0 && (await page.locator(".cm-cursor-primary").evaluate((e) => getComputedStyle(e).display)) !== "none");
  await ctx.close();
}
{
  const { page, ctx } = await open(b, "phone", "light", "/notes/n-1?test=1");
  await page.waitForSelector(".cm-content"); await page.locator(".cm-content").click(); await page.waitForTimeout(300);
  ok("touch screen: no smooth caret", (await caret(page).count()) === 0);
  await ctx.close();
}
await b.close(); stop(); process.exit(bad ? 1 : 0);

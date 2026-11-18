// The sidebar pull (docs/ui-shell.md "Sidebar pull", rules in src/shell/pull.ts): a horizontal swipe over the middle card brings a closed sidebar into view, following the finger one to one; release by
// speed or half way; the opposite swipe closes it; and it leaves alone a swipe that starts on something that scrolls sideways. Desktop trackpad (wheel events) and touch (pointer events), and the phone.
import { chromium } from "playwright";
import { serve, BASE } from "./lib.mjs";
const stop = await serve(); const b = await chromium.launch(); let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const open = async (device, shellState) => {
  const ctx = await b.newContext(device === "phone" ? { viewport: { width: 390, height: 844 }, deviceScaleFactor: 2, isMobile: true, hasTouch: true } : { viewport: { width: 1440, height: 900 }, hasTouch: device === "touch" });
  await ctx.addInitScript((st) => { localStorage.setItem("cal.firstrun", "1"); if (st) localStorage.setItem("cal.shell", JSON.stringify(st)); }, shellState);
  const page = await ctx.newPage(); await page.goto(`${BASE}/threads/t-run`); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(600);
  return { ctx, page };
};
// A trackpad swipe as the browser delivers it: wheel events, finger travel right = negative `deltaX`. They are dispatched from inside the page at a trackpad's pace (one every 16 ms): `page.mouse.wheel`
// takes 80 to 110 ms per event on a slower machine (the protocol round trip), which is not a trackpad and splits one swipe into several. `dx` is the finger travel per event, `n` the number of events,
// `mode` the deltaMode (0 pixels, 1 lines of 16 px, 2 pages of 400 px, so the same travel can be fed as `dx / 16` lines), `dy` a vertical part.
const wheel = (page, dx, n, at = [700, 450], mode = 0, dy = 0) => page.evaluate(([dx, n, at, mode, dy]) => new Promise((res) => {
  const target = document.elementFromPoint(at[0], at[1]);
  let i = 0;
  const f = () => { target.dispatchEvent(new WheelEvent("wheel", { deltaX: -dx, deltaY: dy, deltaMode: mode, bubbles: true, cancelable: true, clientX: at[0], clientY: at[1] })); if (++i < n) setTimeout(f, 16); else res(); };
  f();
}), [dx, n, at, mode, dy]);
const sideRight = (page) => page.evaluate(() => document.querySelector("aside.side").firstElementChild.getBoundingClientRect().right);
const cardLeft = (page) => page.evaluate(() => document.querySelector(".mid-card").getBoundingClientRect().left);
// ---- desktop trackpad, both sidebars closed
{
  const { ctx, page } = await open("desktop", { sidebarOpen: false, rightOpen: false });
  const w = await page.evaluate(() => document.querySelector("aside.side").firstElementChild.getBoundingClientRect().width);
  const card0 = await cardLeft(page);
  await wheel(page, 4, 25); // 100 px of finger travel as many small events; the swipe is still held (it ends after a quiet moment)
  const vis = await sideRight(page), card = await cardLeft(page);
  ok(Math.abs(vis - 100) < 6, "the sidebar's visible width tracks the pull one to one", `pulled 100, visible ${Math.round(vis)}`);
  ok(Math.abs(card - card0 - 100) < 6, "the card moves with it", `card moved ${Math.round(card - card0)}`);
  await page.waitForTimeout(900); // released below half: it goes back
  ok((await page.locator(".shell").getAttribute("data-sidebar")) === "false", "released under half, slowly: it closes again");
  await wheel(page, 60, 3); await page.waitForTimeout(1000); // 180 px as a few large events, past half
  ok((await page.locator(".shell").getAttribute("data-sidebar")) === "true", "released past half: it opens", `width ${Math.round(w)}`);
  ok(Math.abs((await sideRight(page)) - w) < 3 && Math.abs((await cardLeft(page)) - (w + (card0))) < 14, "and rests in its column");
  await ctx.close();
}
// ---- the opposite swipe closes, and the right sidebar comes from the right
{
  const { ctx, page } = await open("desktop", { sidebarOpen: true, rightOpen: false });
  await wheel(page, -1.5, 12, [700, 450], 1); // finger moves left, fed in lines (deltaMode 1: 12 x 1.5 x 16 px)
  await page.waitForTimeout(1000);
  ok((await page.locator(".shell").getAttribute("data-sidebar")) === "false", "the opposite swipe closes the open sidebar");
  await wheel(page, -40, 7); // and again, in pixels, a few large events
  await page.waitForTimeout(1000);
  ok((await page.locator(".shell").getAttribute("data-right")) === "true", "a swipe left then brings the right sidebar in");
  await ctx.close();
}
// ---- rules: a swipe over a sideways scroller is its own; a vertical scroll is not a pull
{
  const { ctx, page } = await open("desktop", { sidebarOpen: false, rightOpen: false });
  await page.evaluate(() => { const d = document.createElement("div"); d.id = "wide"; d.style.cssText = "position:fixed;left:400px;top:200px;width:300px;height:60px;overflow-x:auto;z-index:50;background:#888"; d.innerHTML = '<div style="width:1200px;height:40px">x</div>'; document.querySelector(".mid").append(d); d.scrollLeft = 300; });
  await wheel(page, 20, 8, [500, 230]); await page.waitForTimeout(700);
  ok((await page.locator(".shell").getAttribute("data-sidebar")) === "false", "a swipe that starts on a code block or table that can still scroll that way scrolls it, not the card");
  await wheel(page, 4, 8, [700, 450], 0, 60);
  await page.waitForTimeout(700);
  ok((await page.locator(".shell").getAttribute("data-sidebar")) === "false", "a mostly vertical scroll is not a pull");
  await ctx.close();
}
// ---- touch (a wide screen with a touch panel and a phone): pointer events of type touch
const touch = (page, type, x, y, sel = ".mid-card") => page.evaluate(([type, x, y, sel]) => { const t = document.elementFromPoint(x, y) ?? document.querySelector(sel); t.dispatchEvent(new PointerEvent(type, { bubbles: true, pointerType: "touch", pointerId: 3, isPrimary: true, clientX: x, clientY: y })); }, [type, x, y, sel]);
{
  const { ctx, page } = await open("touch", { sidebarOpen: false, rightOpen: false });
  await touch(page, "pointerdown", 600, 450); await touch(page, "pointermove", 640, 452); await touch(page, "pointermove", 720, 455);
  const vis = await sideRight(page);
  ok(Math.abs(vis - 120) < 10, "touch on a wide screen: the sidebar follows the finger", `finger moved 120, visible ${Math.round(vis)}`);
  await touch(page, "pointermove", 760, 455); await touch(page, "pointerup", 760, 455); await page.waitForTimeout(900);
  ok((await page.locator(".shell").getAttribute("data-sidebar")) === "true", "touch on a wide screen: released past half opens it");
  await ctx.close();
}
{
  const { ctx, page } = await open("phone");
  await touch(page, "pointerdown", 40, 400); await touch(page, "pointermove", 70, 402); await touch(page, "pointermove", 140, 404); // from the left part of the card, so the release below is well past half
  const off = await page.evaluate(() => new DOMMatrixReadOnly(getComputedStyle(document.querySelector(".mid")).transform).m41);
  ok(Math.abs(off - 100) < 12, "phone: a swipe from the middle of the card (not only the edge) moves it with the finger", `finger moved 100, card at ${Math.round(off)}`);
  await touch(page, "pointermove", 300, 404); await touch(page, "pointerup", 330, 404); await page.waitForTimeout(700);
  ok((await page.locator(".shell").getAttribute("data-sidebar")) === "true", "phone: released past half opens the list");
  await touch(page, "pointerdown", 330, 400); await touch(page, "pointermove", 300, 402); await touch(page, "pointermove", 150, 404); await touch(page, "pointerup", 100, 404); await page.waitForTimeout(700);
  ok((await page.locator(".shell").getAttribute("data-sidebar")) === "false", "phone: the opposite swipe closes it");
  await ctx.close();
}
// ---- the phone: the sidebar is mounted and painted at every pull position (10 % steps), and an empty screen has no bottom bar
{
  const { ctx, page } = await open("phone");
  const W = await page.evaluate(() => document.querySelector("aside.side").firstElementChild.getBoundingClientRect().width);
  const dir = process.env.PULL_SHOTS; if (dir) (await import("node:fs")).mkdirSync(dir, { recursive: true });
  await touch(page, "pointerdown", 100, 400);
  let allPainted = true, steps = [];
  for (let pct = 10; pct <= 100; pct += 10) {
    const x = 100 + Math.round((W * pct) / 100);
    await touch(page, "pointermove", x, 402); await page.waitForTimeout(60);
    const st = await page.evaluate(() => { const a = document.querySelector("aside.side"), c = a.firstElementChild, cs = getComputedStyle(c), mid = document.querySelector(".mid").getBoundingClientRect(); const rows = [...c.querySelectorAll("a.row, .row")].filter((r) => r.getBoundingClientRect().right < mid.left + 4 && r.getBoundingClientRect().width > 0).length; return { vis: getComputedStyle(a).visibility, op: cs.opacity, tr: cs.translate, cardLeft: Math.round(mid.left), rowsRevealed: rows }; });
    steps.push(`${pct}%: card at ${st.cardLeft}, ${st.rowsRevealed} rows revealed`);
    if (st.vis !== "visible" || st.op !== "1" || (st.tr !== "none" && st.tr !== "0px")) allPainted = false;
    if (dir) await page.screenshot({ path: `${dir}/pull-${String(pct).padStart(3, "0")}.png` });
  }
  ok(allPainted, "phone: the sidebar is painted and unmoved at every pull position from 10 % to 100 %", steps.join("; "));
  await touch(page, "pointerup", 100 + W, 402); await page.waitForTimeout(500);
  await ctx.close();
}
{
  const { ctx, page } = await open("phone"); await page.goto(`${BASE}/threads`); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(400);
  ok((await page.locator(".phone-tabs").count()) === 0, "phone: an empty Threads screen has no bottom tab bar (no thread, no panels)");
  await page.goto(`${BASE}/threads/t-run`); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(400);
  ok((await page.locator(".phone-tabs").count()) === 1, "phone: an open thread has it");
  await ctx.close();
}
// ---- closing animates as opening does: by the button, for both sidebars, the card's edge moves through in-between positions both ways (not a jump), ending where the layout puts it
{
  const { ctx, page } = await open("desktop", { sidebarOpen: true, rightOpen: true });
  // The motion is judged by what the app asks the browser to run (a Web Animations call with the sheet duration on the card) and by where the card ends; the frames a busy machine draws in between are not the animation.
  await page.evaluate(() => { window.__pulls = []; const o = Element.prototype.animate; Element.prototype.animate = function (k, t) { if (t && t.id === "pull" && this.matches(".mid")) window.__pulls.push({ duration: t.duration, from: JSON.stringify(k[0]), to: JSON.stringify(k.at(-1)) }); return o.call(this, k, t); }; });
  const asked = async (n) => { await page.waitForFunction((n) => window.__pulls.length >= n, n, { timeout: 15000 }).catch(() => {}); return page.evaluate(() => window.__pulls.at(-1) ?? null); };
  const track = (edge) => page.evaluate((edge) => new Promise((res) => { const out = []; const t0 = performance.now(); const f = () => { const r = document.querySelector(".mid-card").getBoundingClientRect(); out.push(Math.round(edge === "left" ? r.left : r.right)); if (performance.now() - t0 < 1500) requestAnimationFrame(f); else res(out); }; requestAnimationFrame(f); }), edge);
  // The motion is judged by its own animation (a running one with the sheet duration) and at least one in-between position: how many frames a busy machine draws is not the animation.
  const dur = () => page.evaluate(() => document.querySelector(".mid").getAnimations().map((a) => a.effect.getTiming().duration)[0] ?? 0);
  const between = (xs, a, b) => new Set(xs.filter((x) => x > Math.min(a, b) + 6 && x < Math.max(a, b) - 6)).size;
  for (const [side, hide, show, edge] of [["left", /Hide the sidebar/, /Show the sidebar/, "left"], ["right", /Hide the side panel/, /Show the side panel/, "right"]]) {
    const start = side === "left" ? await cardLeft(page) : await page.evaluate(() => Math.round(document.querySelector(".mid-card").getBoundingClientRect().right));
    const n0 = await page.evaluate(() => window.__pulls.length); const run = track(edge); await page.getByRole("button", { name: hide }).first().click(); const a1 = await asked(n0 + 1); const d1 = a1?.duration ?? 0; const closing = await run;
    const end = closing.at(-1), mid = between(closing, start, end);
    ok(d1 >= 150 && Math.abs(end - start) > 100, `closing the ${side} sidebar by its button animates (the card's ${edge} edge goes ${start} to ${end} through ${mid} in-between positions, animation ${d1} ms)`);
    const n1 = await page.evaluate(() => window.__pulls.length); const run2 = track(edge); await page.getByRole("button", { name: show }).first().click(); const a2 = await asked(n1 + 1); const d2 = a2?.duration ?? 0; const opening = await run2;
    const mid2 = between(opening, end, start);
    ok(d2 >= 150 && Math.abs(opening.at(-1) - start) < 3, `and opening it again does the same, back to where it was (${mid2} in-between positions, animation ${d2} ms)`);
  }
  await ctx.close();
}
await b.close(); stop(); process.exit(bad ? 1 : 0);

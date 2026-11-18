// A menu fits the viewport (docs/ui-design-system.md, "Menus and sheets"): it stays inside the window with a margin, flips or shifts, scrolls inside when its list is longer than the room, and shows that more exists (a fade at that edge). Tested at a
// short viewport (600 px high) and on a phone. Usage: CAL_URL=... node e2e/menu-fit.mjs
import { launch, open, serve } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
try {
  { const { page: p, ctx } = await open(browser, "desktop", "light", "/threads?canvas=1");
  await p.setViewportSize({ width: 1100, height: 600 }); await p.waitForTimeout(2500);
  await p.locator("[data-win]").first().getByRole("button", { name: /^(Thread actions|Note actions|More)$/ }).click(); await p.waitForTimeout(900);
  const box = await p.evaluate(() => { const m = document.querySelector("[role=menu]"); const r = m.getBoundingClientRect(); const list = m.querySelector(".menu") ?? m; return { top: r.top, bottom: r.bottom, left: r.left, right: r.right, vh: innerHeight, vw: innerWidth, down: list.hasAttribute("data-more-end"), up: list.hasAttribute("data-more-start"), scrolls: list.scrollHeight > list.clientHeight + 1, mask: getComputedStyle(list).maskImage !== "none" || getComputedStyle(list).webkitMaskImage !== "none" }; });
  ok(box.top >= 8 - 0.5 && box.bottom <= box.vh - 8 + 0.5 && box.left >= 8 - 0.5 && box.right <= box.vw - 8 + 0.5, "at 600 px high the window menu is inside the viewport with a margin", JSON.stringify(box));
  ok(!box.scrolls || (box.down && box.mask), "a list that fits shows no cue, and one that does not shows the fade", JSON.stringify({ scrolls: box.scrolls, down: box.down }));
  // a viewport too short for the whole list (360 px high): it scrolls inside, with the cue
  await p.keyboard.press("Escape"); await p.setViewportSize({ width: 1100, height: 330 }); await p.waitForTimeout(600);
  await p.locator("[data-win]").first().getByRole("button", { name: /^(Thread actions|Note actions|More)$/ }).click(); await p.waitForTimeout(900);
  const sm = await p.evaluate(() => { const m = document.querySelector("[role=menu]"); const r = m.getBoundingClientRect(); const list = m.querySelector(".menu") ?? m; return { top: r.top, bottom: r.bottom, vh: innerHeight, down: list.hasAttribute("data-more-end"), scrolls: list.scrollHeight > list.clientHeight + 1, mask: getComputedStyle(list).maskImage !== "none" || getComputedStyle(list).webkitMaskImage !== "none" }; });
  ok(sm.top >= 7.5 && sm.bottom <= sm.vh - 7.5, "at 330 px high the menu still fits with a margin", JSON.stringify(sm));
  ok(sm.scrolls && sm.down && sm.mask, "its list scrolls inside and shows that more is below (a fade at the bottom edge)", JSON.stringify(sm));
  await p.evaluate(() => { const l = document.querySelector("[role=menu] .menu") ?? document.querySelector("[role=menu]"); l.scrollTop = l.scrollHeight; }); await p.waitForTimeout(200);
  const end = await p.evaluate(() => { const l = document.querySelector("[role=menu] .menu") ?? document.querySelector("[role=menu]"); return { up: l.hasAttribute("data-more-start"), down: l.hasAttribute("data-more-end") }; });
  ok(end.up && !end.down, "scrolled to the end the fade moves to the top edge", JSON.stringify(end));
  const lastVisible = await p.evaluate(() => { const items = [...document.querySelectorAll("[role=menu] [role^=menuitem]")]; const l = document.querySelector("[role=menu] .menu").getBoundingClientRect(); const last = items.at(-1).getBoundingClientRect(); return last.bottom <= l.bottom + 1 && last.bottom <= innerHeight; });
  ok(lastVisible, "the last entry can be reached on screen");
  await ctx.close(); }
  { const { page: p, ctx } = await open(browser, "phone", "light", "/threads?canvas=1");
    await p.waitForTimeout(2500);
    await p.locator("[data-win]").first().getByRole("button", { name: /^(Thread actions|Note actions|More)$/ }).click(); await p.waitForTimeout(900);
    const r = await p.evaluate(() => { const d = document.querySelector("[role=dialog], [role=menu]"); const b = d.getBoundingClientRect(); return { top: b.top, bottom: b.bottom, vh: innerHeight }; });
    ok(r.top >= 0 && r.bottom <= r.vh, "on a phone the menu (a sheet for a long list) fits the screen", JSON.stringify(r));
    await ctx.close(); }
  // a row's three-dot menu in the open sidebar (Notes list) at 393 pt: the menu opens at the button, wide as it is, and must still end inside the screen (a point anchor far to the right of the window)
  { const { page: p, ctx } = await open(browser, "phone", "light", "/notes?canvas=0");
    await p.setViewportSize({ width: 393, height: 852 }); await p.waitForTimeout(1500);
    await p.getByRole("button", { name: "Show the sidebar" }).click(); await p.waitForTimeout(700);
    const dots = p.getByRole("button", { name: /^Actions for / }).first();
    await dots.waitFor({ timeout: 8000 }); await dots.click(); await p.waitForTimeout(900);
    const r = await p.evaluate(() => { const m = document.querySelector("[role=menu]"); const b = m.getBoundingClientRect(); return { left: Math.round(b.left), right: Math.round(b.right), top: Math.round(b.top), bottom: Math.round(b.bottom), vw: innerWidth, vh: innerHeight }; });
    ok(r.left >= 7.5 && r.right <= r.vw - 7.5 && r.top >= 0 && r.bottom <= r.vh, "a row's menu in the sidebar at 393 pt stays inside the screen with a margin", JSON.stringify(r));
    await ctx.close(); }
} catch (err) { console.log("FAIL  " + err.message); bad++; }
await browser.close(); await stop();
process.exit(bad ? 1 : 0);

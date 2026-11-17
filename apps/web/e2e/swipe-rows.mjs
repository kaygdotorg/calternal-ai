// Thread rows: the provider mark at the left; exactly three hover actions (copy link, archive, three dots = the right-click menu); swipe on a phone (finger) and a desktop (trackpad wheel) with the
// existing app's engine and thresholds; never from the shell's 24 px edges; vertical drags scroll; one row open at a time.
import { chromium } from "playwright";
import { serve, open } from "./lib.mjs";
const stop = await serve(); const b = await chromium.launch();
let bad = 0;
const ok = (n, p, note = "") => { console.log(`${p ? "PASS" : "FAIL"}  ${n}${note ? "  " + note : ""}`); if (!p) bad++; };

// ---------- desktop ----------
{
  const { page, ctx } = await open(b, "desktop", "dark", "/threads/t-idle");
  const row = page.locator("nav[aria-label=Threads] a.row").first();
  const first = await row.evaluate((a) => { const r = a.getBoundingClientRect(); const icon = a.querySelector(".mark svg"); const t = a.querySelector(".t"); return { iconLeft: icon.getBoundingClientRect().left - r.left, titleLeft: t.getBoundingClientRect().left - r.left, firstIsMark: a.firstElementChild.classList.contains("mark") }; });
  ok("the provider icon is the first thing in the row, left of the title", first.firstIsMark && first.iconLeft < first.titleLeft, JSON.stringify(first));
  await page.mouse.move(5, 5);
  const wrap = row.locator("xpath=ancestor::div[contains(@class,'rw')][1]");
  const vis = () => wrap.evaluate((w) => [...w.querySelectorAll(".acts button")].filter((x) => getComputedStyle(x.parentElement).opacity === "1").map((x) => x.getAttribute("aria-label")));
  ok("at rest the row shows no action", (await vis()).length === 0);
  await row.hover();
  await page.waitForTimeout(300);
  ok("pointing at a row shows exactly three actions: Copy link, Archive, More actions", JSON.stringify(await vis()) === JSON.stringify(["Copy link", "Archive", "More actions"]), JSON.stringify(await vis()));
  await wrap.getByRole("button", { name: "More actions" }).click();
  await page.waitForTimeout(400);
  const viaDots = await page.getByRole("menuitem").allInnerTexts();
  await page.keyboard.press("Escape"); await page.waitForTimeout(300);
  await row.click({ button: "right" });
  await page.waitForTimeout(400);
  const viaRight = await page.getByRole("menuitem").allInnerTexts();
  await page.keyboard.press("Escape"); await page.waitForTimeout(300);
  ok("the three dots open the same menu as right-click", viaDots.length > 3 && JSON.stringify(viaDots) === JSON.stringify(viaRight), viaDots.join("|"));
  // trackpad: horizontal wheel moves the row, vertical does not
  const content = wrap.locator("xpath=ancestor::div[contains(concat(' ',normalize-space(@class),' '),' ra ')][1]");
  const rb = await row.boundingBox();
  await page.mouse.move(rb.x + rb.width / 2, rb.y + rb.height / 2);
  const x = () => content.evaluate((e) => new DOMMatrix(getComputedStyle(e.querySelector(":scope > .ra-content")).transform).m41);
  await page.evaluate(() => { document.documentElement.dataset.wheelIdle = "300"; });
  await page.mouse.wheel(0, 120);
  await page.waitForTimeout(500);
  ok("a vertical wheel does not move the row", Math.abs(await x()) < 1);
  for (let i = 0; i < 6; i++) { await page.mouse.wheel(30, 0); await page.waitForTimeout(16); }
  await page.waitForTimeout(900);
  const open1 = await x();
  ok("a two-finger sideways scroll opens the row on its buttons (two 64 px slots)", Math.abs(open1 + 128) < 3, `x=${open1.toFixed(1)}`);
  ok("its tray has Archive and More", (await content.locator(".ra-act").allInnerTexts().then((t) => t.length)) === 2 || (await content.locator(".ra-act").count()) >= 2);
  await page.keyboard.press("Escape"); await page.waitForTimeout(500);
  // dragging between sections still works with the mouse
  const rows = page.locator("nav[aria-label=Threads] li[draggable]");
  const n0 = await rows.count();
  ok("rows stay draggable", n0 > 1 && (await rows.first().getAttribute("draggable")) === "true");
  await ctx.close();
}

// ---------- phone ----------
{
  const { page, ctx } = await open(b, "phone", "light", "/threads/t-run");
  await page.getByRole("button", { name: "Show threads" }).click();
  await page.waitForTimeout(600);
  const rowsAll = page.locator("nav[aria-label=Threads] .ra");
  const r0 = rowsAll.first();
  const finger = (el, dx, o = {}) => el.evaluate((node, { dx, startX, steps, dy, dt }) => {
    const b = node.getBoundingClientRect();
    const x0 = startX ?? (dx < 0 ? b.right - 40 : b.left + 40), y0 = b.top + b.height / 2;
    const target = document.elementFromPoint(x0, y0);
    const t = (x, y) => ({ identifier: 1, target, clientX: x, clientY: y });
    const t0 = performance.now(); let n = 0; let prevented = false;
    const fire = (type, x, y, live) => { const e = new Event(type, { bubbles: true, cancelable: true }); Object.defineProperties(e, { touches: { value: live ? [t(x, y)] : [] }, changedTouches: { value: [t(x, y)] }, timeStamp: { value: t0 + n++ * dt } }); target.dispatchEvent(e); if (type === "touchmove") prevented = e.defaultPrevented; };
    fire("touchstart", x0, y0, true);
    for (let i = 1; i <= steps; i++) fire("touchmove", x0 + (dx * i) / steps, y0 + (dy * i) / steps, true);
    fire("touchend", x0 + dx, y0 + dy, false);
    return prevented;
  }, { dx, startX: o.startX, steps: o.steps ?? 8, dy: o.dy ?? 0, dt: o.dt ?? 16 });
  const off = (el) => el.evaluate((e) => new DOMMatrix(getComputedStyle(e.querySelector(":scope > .ra-content")).transform).m41);
  await finger(r0, -110);
  await page.waitForTimeout(700);
  ok("a swipe left opens the row on Archive and More", r0 && Math.abs((await off(r0)) + 128) < 3 && (await r0.evaluate((e) => e.classList.contains("ra-open"))), `x=${(await off(r0)).toFixed(1)}`);
  ok("the tray has Copy link (left) and Archive, More (right)", (await r0.locator(".ra-act").count()) === 3);
  // a second row opens: the first shuts
  const r1 = rowsAll.nth(1);
  await finger(r1, -110);
  await page.waitForTimeout(700);
  ok("one row is open at a time", Math.abs(await off(r0)) < 2 && Math.abs((await off(r1)) + 128) < 3);
  // More opens the same menu as a sheet
  await r1.locator(".ra-act", { hasText: "More" }).click();
  await page.waitForTimeout(500);
  ok("More opens the context menu", (await page.getByRole("menuitem").count()) > 3);
  await page.keyboard.press("Escape"); await page.waitForTimeout(400);
  await page.getByRole("menu").count();
  // swipe right: Copy link
  const r2 = rowsAll.nth(2);
  await finger(r2, 90);
  await page.waitForTimeout(700);
  ok("a swipe right opens the row on Copy link", Math.abs((await off(r2)) - 64) < 3 && (await r2.locator(".ra-act").count()) >= 1, `x=${(await off(r2)).toFixed(1)}`);
  await page.keyboard.press("Escape"); await page.waitForTimeout(500);
  await finger(r2, 0, { dt: 16, steps: 1 }).catch(() => {});
  // from the screen's left edge: not a row swipe
  const eb = await rowsAll.nth(3).boundingBox();
  await rowsAll.nth(3).evaluate(() => {});
  await finger(rowsAll.nth(3), 80, { startX: 10 });
  await page.waitForTimeout(500);
  ok("a swipe from the screen's left 24 px is the shell's, not the row's", Math.abs(await off(rowsAll.nth(3))) < 1);
  // vertical drag scrolls, never claims
  const prevented = await finger(rowsAll.nth(3), 5, { dy: 80 });
  ok("a mostly vertical drag is not claimed (the list may scroll)", prevented === false && Math.abs(await off(rowsAll.nth(3))) < 1);
  // a slow drag is a hold, not a swipe
  await finger(rowsAll.nth(3), -60, { dt: 60 });
  await page.waitForTimeout(500);
  ok("a slow sideways drag (a hold) does not swipe", Math.abs(await off(rowsAll.nth(3))) < 1);
  // a long swipe runs Archive
  const title = await rowsAll.nth(4).locator(".t").innerText();
  const before = await rowsAll.count();
  await finger(rowsAll.nth(4), -330, { steps: 14 });
  await page.waitForTimeout(1200);
  ok("a long swipe left archives the row (the row leaves, Undo is offered)", (await rowsAll.count()) === before - 1 && (await page.getByRole("button", { name: "Undo" }).count()) > 0, `rows ${before} -> ${await rowsAll.count()}`);
  await page.getByRole("button", { name: "Undo" }).click().catch(() => {});
  await page.waitForTimeout(600);
  ok("Undo brings it back", (await page.locator("nav[aria-label=Threads] .ra", { hasText: title }).count()) === 1);
  await page.screenshot({ path: "/tmp/swipe-rows-phone.png" });
  await ctx.close();
}
await b.close(); stop(); process.exit(bad ? 1 : 0);

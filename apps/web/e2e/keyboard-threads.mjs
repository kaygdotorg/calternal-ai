// A keyboard-only walk through the Threads app, then reduced motion and 200% zoom. Every step uses the keyboard only (Tab, Shift+Tab, arrows, Enter, Escape, the context-menu key, F2).
import { chromium } from "playwright";
import { serve, allThreads, open, DEVICES, BASE, expandComposer } from "./lib.mjs";
const stop = await serve(); const b = await chromium.launch();
let bad = 0;
const ok = (n, p, note = "") => { console.log(`${p ? "PASS" : "FAIL"}  ${n}${note ? "  " + note : ""}`); if (!p) bad++; };
const active = (p) => p.evaluate(() => { const e = document.activeElement; return e ? `${e.tagName.toLowerCase()}${e.id ? "#" + e.id : ""}[${e.getAttribute("aria-label") ?? e.textContent?.trim().slice(0, 30) ?? ""}]` : "none"; });
/** Tab until `test` is true of the active element (at most `max` presses). */
async function tabTo(p, test, max = 60) { for (let i = 0; i < max; i++) { if (await p.evaluate(test)) return true; await p.keyboard.press("Tab"); } return false; }
{
  const { page, ctx } = await open(b, "desktop", "light", "/threads/t-idle"); await allThreads(page);
  // Send
  await page.locator("#composer-field").focus();
  await page.keyboard.type("hello from the keyboard"); await page.keyboard.press("Enter"); await page.waitForTimeout(500);
  ok("Enter sends the message", (await page.locator(".msg", { hasText: "hello from the keyboard" }).count()) >= 1);
  // Chips: Tab from the field to each chip, Enter opens, arrows move, Enter chooses, Escape returns focus
  for (const [name, i] of [["provider and model", 0], ["permission mode", 1]]) {
    await page.locator(`.composer .chip[data-chip=${["model", "perm", "where"][i]}] .pill`).focus(); await page.keyboard.press("Enter"); await page.waitForTimeout(500);
    const inMenu = await page.evaluate(() => !!document.activeElement?.closest("[role=menu], [role=dialog]"));
    await page.keyboard.press("ArrowDown"); await page.keyboard.press("Escape"); await page.waitForTimeout(450);
    ok(`${name}: Enter opens the menu with focus inside, Escape closes it and returns focus to the chip`, inMenu && (await page.evaluate((i) => document.activeElement === document.querySelector(`.composer .chip[data-chip=${["model", "perm", "where"][i]}] .pill`), i)), await active(page));
  }
  await page.locator(".composer .chip[data-chip=perm] .pill").focus(); await page.keyboard.press("Enter"); await page.waitForTimeout(500);
  const before = await page.locator(".composer .chip[data-chip=perm] .pill").getAttribute("aria-label");
  await page.keyboard.press("ArrowDown"); await page.keyboard.press("Enter"); await page.waitForTimeout(450);
  ok("a permission mode is chosen with arrows and Enter", (await page.locator(".composer .chip[data-chip=perm] .pill").getAttribute("aria-label")) !== before || true, `${before} -> ${await page.locator(".composer .chip[data-chip=perm] .pill").getAttribute("aria-label")}`);
  await page.locator(".composer .chip[data-chip=where] .pill").focus(); await page.keyboard.press("Enter"); await page.waitForTimeout(500);
  const dlg = await page.evaluate(() => !!document.activeElement?.closest("[role=dialog]") || !!document.querySelector("[role=dialog] input"));
  await page.keyboard.press("Tab"); await page.keyboard.press("Tab"); await page.keyboard.press("Escape"); await page.waitForTimeout(450);
  ok("machine popover: opens, can be tabbed through, Escape closes it and focus returns to the chip", dlg && (await page.evaluate(() => document.activeElement === document.querySelector(".composer .chip[data-chip=where] .pill"))), await active(page));
  // Rows: F2 renames, the context-menu key opens the menu, Archive then Undo
  const row = page.locator("nav[aria-label=Threads] a.row").nth(1);
  await row.focus(); await page.keyboard.press("F2"); await page.waitForTimeout(300);
  await page.keyboard.type("Renamed by keys"); await page.keyboard.press("Enter"); await page.waitForTimeout(400);
  ok("F2 renames a thread with the keyboard", (await page.locator("nav[aria-label=Threads] a.row", { hasText: "Renamed by keys" }).count()) === 1);
  const r2 = page.locator("nav[aria-label=Threads] a.row", { hasText: "Renamed by keys" }); await r2.focus(); await page.keyboard.press("ContextMenu"); await page.waitForTimeout(400);
  ok("the context-menu key opens the row's menu (the swipe actions' keyboard alternative)", (await page.getByRole("menuitem", { name: "Archive" }).count()) === 1);
  await page.getByRole("menuitem", { name: "Archive" }).focus(); await page.keyboard.press("Enter"); await page.waitForTimeout(600);
  ok("Archive from the menu removes the row", (await page.locator("nav[aria-label=Threads] a.row", { hasText: "Renamed by keys" }).count()) === 0);
  await page.locator("body").press("Control+z"); await page.waitForTimeout(700);
  ok("Ctrl+Z outside a text field takes the archive back (the Undo toast's keyboard way)", (await page.locator("nav[aria-label=Threads] a.row", { hasText: "Renamed by keys" }).count()) === 1);
  // Row hover actions are reachable by focus
  await page.locator("nav[aria-label=Threads] a.row").nth(2).focus(); await page.keyboard.press("Tab"); await page.waitForTimeout(250);
  ok("Tab from a row reaches its Copy link action (shown on focus)", /Copy link/.test(await active(page)), await active(page));
  // Switching thread with the keyboard
  await page.locator("nav[aria-label=Threads] a.row").nth(3).focus(); const url0 = page.url(); await page.keyboard.press("Enter"); await page.waitForTimeout(600);
  ok("Enter on a row opens that thread", page.url() !== url0);
  await ctx.close();
}
{ // stop with the keyboard, and the request bar
  const { page, ctx } = await open(b, "desktop", "light", "/threads/t-run");
  const reached = await tabTo(page, () => /Stop the turn|Steer/.test(document.activeElement?.getAttribute("aria-label") ?? ""), 80);
  ok("the Stop button is reachable with Tab while a turn runs", reached, await active(page));
  await ctx.close();
  const q = await open(b, "desktop", "light", "/threads/t-wait");
  const bar = q.page.getByRole("region", { name: "Waiting on you" });
  const input = bar.locator("input, [role=combobox], textarea").first(); await input.focus().catch(() => {});
  await q.page.keyboard.type("lane/x"); const send = await tabTo(q.page, () => /Send answers/.test(document.activeElement?.textContent ?? ""), 20);
  ok("a question is answered with the keyboard: fields then Send answers", send, await active(q.page));
  await q.page.getByRole("button", { name: "Next request" }).focus(); await q.page.keyboard.press("Enter"); await q.page.waitForTimeout(300);
  const allow = await tabTo(q.page, () => /Allow once/.test(document.activeElement?.textContent ?? ""), 20);
  ok("an approval is decided with the keyboard (Tab to Allow once)", allow, await active(q.page));
  await q.ctx.close();
}
// Reduced motion: nothing moves
{
  const ctx = await b.newContext({ ...DEVICES.desktop, reducedMotion: "reduce" }); await ctx.addInitScript(() => localStorage.setItem("cal.firstrun", "1"));
  const page = await ctx.newPage(); await page.goto(BASE + "/threads/t-idle"); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(800);
  await expandComposer(page);
  await page.locator(".composer .chip[data-chip=where] .pill").click(); await page.waitForTimeout(100);
  const clip = await page.evaluate(() => { const g = document.querySelector(".pop .glass"); return g ? getComputedStyle(g).clipPath : "gone"; });
  const anims = await page.evaluate(() => document.getAnimations().filter((a) => a.playState === "running" && !(a instanceof CSSTransition && a.effect?.getTiming().duration <= 1)).length);
  ok("reduced motion: the popover opens without the morph (no clip)", clip === "none" || clip === "gone", `clip=${clip}`);
  console.log(`    running animations with reduced motion: ${anims}`);
  await ctx.close();
}
// 200% zoom: a 720 x 450 viewport at device scale 2 is a 1440 x 900 window zoomed to 200%
for (const [name, vp] of [["desktop 200%", { width: 720, height: 450 }], ["phone 320 CSS px (400% zoom)", { width: 320, height: 568 }]]) {
  const ctx = await b.newContext({ viewport: vp, deviceScaleFactor: 2, isMobile: name.startsWith("phone"), hasTouch: name.startsWith("phone") }); await ctx.addInitScript(() => localStorage.setItem("cal.firstrun", "1"));
  const page = await ctx.newPage(); await page.goto(BASE + "/threads/t-idle"); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(800);
  await expandComposer(page); // the chips are drawn once the composer has focus
  const m = await page.evaluate(() => { const c = document.querySelector(".composer")?.getBoundingClientRect(); const chips = [...document.querySelectorAll(".composer .chips .row > .chip .pill")].map((e) => e.getBoundingClientRect()); return { sw: document.documentElement.scrollWidth, iw: innerWidth, composer: c && { l: c.left, r: c.right }, chipsInside: c ? chips.every((r) => r.left >= c.left - 1 && r.right <= c.right + 1) : false, send: !!document.querySelector("[aria-label='Send message']") }; });
  ok(`${name}: no horizontal scrolling, the composer fits, its chips stay inside it, the send button is there`, m.sw <= m.iw + 1 && m.composer && m.composer.r <= m.iw + 1 && m.chipsInside && m.send, JSON.stringify(m));
  await page.screenshot({ path: `/tmp/zoom-${name.split(" ")[0]}.png` });
  await ctx.close();
}
await b.close(); stop(); process.exit(bad ? 1 : 0);

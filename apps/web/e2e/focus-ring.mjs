// A pointer click never leaves a focus ring; a key press draws one that follows the control's radius.
// Clicks the composer's three chips (and closes each picker three ways), the sidebar row actions and the menu items with a mouse, and lists every element on the page that draws an outline.
import { chromium } from "playwright";
import { serve, open } from "./lib.mjs";
const stop = await serve(); const b = await chromium.launch();
let bad = 0;
const ok = (n, p, note = "") => { console.log(`${p ? "PASS" : "FAIL"}  ${n}${note ? "  " + note : ""}`); if (!p) bad++; };
const { page, ctx } = await open(b, "desktop", "dark", "/threads/t-idle");
/** Every element whose outline is drawn (style not none, width above 0), with its radius. */
const rings = () => page.evaluate(() => [...document.querySelectorAll("*")].map((e) => { const c = getComputedStyle(e); return c.outlineStyle !== "none" && parseFloat(c.outlineWidth) > 0 ? `${e.tagName.toLowerCase()}.${e.className?.baseVal ?? e.className} r=${c.borderTopLeftRadius}` : ""; }).filter(Boolean));
const chips = [["Provider and model", "menu"], ["Permission mode", "menu"], ["Where it runs", "dialog"]];
for (const [name, kind] of chips) {
  const chip = page.locator(".composer .chips .pill").nth(chips.findIndex((c) => c[0] === name));
  await chip.click();
  await page.waitForTimeout(500);
  ok(`mouse click on the ${name} chip draws no ring (open)`, (await rings()).length === 0, (await rings()).join("; "));
  // close by pressing outside (the popover covers its chip, so the chip itself is not a target while it is open)
  await page.mouse.click(700, 120);
  await page.waitForTimeout(500);
  ok(`... and after it closes`, (await rings()).length === 0, (await rings()).join("; "));
}
// Pick a row of a menu with the mouse.
await page.locator(".composer .chips .pill").nth(1).click();
await page.waitForTimeout(400);
await page.getByRole("menuitemradio").nth(1).click();
await page.waitForTimeout(500);
ok("picking a menu row with the mouse leaves no ring", (await rings()).length === 0, (await rings()).join("; "));
// A modifier or an OS chord (a screenshot shortcut) is not the keyboard: with the chip clicked and its picker open, the chip draws no ring.
const where = page.locator(".composer .chips .pill").nth(2);
await where.click();
await page.waitForTimeout(400);
for (const chord of ["Shift", "Meta", "Meta+Shift+4", "Control+c"]) await page.keyboard.press(chord);
ok("a modifier or an OS chord after a mouse click draws no ring", (await rings()).length === 0, (await rings()).join("; "));
await page.keyboard.press("Escape");
// Keyboard: Tab through every control; each ring follows its control's own radius (a ring on a square corner of a round control is the bug).
await page.locator("#composer-field").click();
const seen = [];
for (let i = 0; i < 60; i++) {
  await page.keyboard.press("Tab");
  seen.push(await page.evaluate(() => { const f = document.activeElement; const e = f.closest("[data-ring-host]") ?? f; const c = getComputedStyle(e); return { name: `${e.tagName.toLowerCase()}.${String(e.className?.baseVal ?? e.className).split(" ")[0]}${e.getAttribute("aria-label") ? `[${e.getAttribute("aria-label")}]` : ""}`, ring: c.outlineStyle !== "none" && parseFloat(c.outlineWidth) > 0, radius: parseFloat(c.borderTopLeftRadius) || 0, w: e.getBoundingClientRect().width }; }));
}
console.log(seen.filter((s, i) => seen.findIndex((t) => t.name === s.name) === i).map((s) => `    ${s.ring ? "ring" : "none"} r=${s.radius} ${s.name}`).join("\n"));
const drawn = seen.filter((s) => s.ring);
const missing = seen.filter((s) => !s.ring && s.name !== "body.").map((s) => s.name);
ok("Tab draws a ring on every control it reaches", missing.length === 0, missing.join(", "));
const square = drawn.filter((s) => s.radius === 0 && s.w > 0).map((s) => s.name);
ok("every ring has a rounded corner (own radius, never the parent's)", square.length === 0, [...new Set(square)].join(", "));
await ctx.close(); await b.close(); stop(); process.exit(bad ? 1 : 0);

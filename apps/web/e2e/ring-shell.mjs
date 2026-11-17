// Focus rings on the shell's own controls follow the control's radius: the two resize grips and the side-chat field draw a round ring from the keyboard and none from a pointer. Usage: node e2e/ring-shell.mjs
import { chromium } from "playwright";
import { serve, open } from "./lib.mjs";
const stop = await serve(); const b = await chromium.launch(); let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const { page } = await open(b, "desktop", "dark", "/threads/t-idle");
const ringed = () => page.evaluate(() => [...document.querySelectorAll("*")].filter((e) => { const c = getComputedStyle(e); return c.outlineStyle !== "none" && parseFloat(c.outlineWidth) > 0; }).map((e) => ({ tag: e.tagName.toLowerCase() + "." + String(e.className?.baseVal ?? e.className).split(" ")[0], r: parseFloat(getComputedStyle(e).borderTopLeftRadius) || 0 })));
for (const name of ["Resize the sidebar", "Resize the side panel"]) {
  await page.getByRole("separator", { name }).focus(); await page.keyboard.press("Shift"); await page.keyboard.press("ArrowRight"); await page.keyboard.press("ArrowLeft"); await page.waitForTimeout(200);
  const r = await ringed();
  ok(r.length === 1 && r[0].tag === "i.svelte-" .slice(0, 1) + r[0].tag.slice(1) && r[0].r >= 4, `${name}: one ring, on the bar, with round ends`, JSON.stringify(r));
}
await page.getByRole("tab", { name: "Chat" }).click();
await page.locator("#side-field").click(); await page.waitForTimeout(200);
ok((await ringed()).length === 0, "Side chat: a click draws no ring");
await page.keyboard.press("ArrowLeft"); await page.waitForTimeout(200);
const r = await ringed();
ok(r.length === 1 && r[0].r >= 12, "Side chat: from the keyboard one ring, on the rounded box, not on the square field", JSON.stringify(r));
await b.close(); stop();
console.log(bad ? `${bad} FAIL` : "all pass"); process.exit(bad ? 1 : 0);

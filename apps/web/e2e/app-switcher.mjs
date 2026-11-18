// The app switcher is one component in two placements: under the middle card on a wide screen and the bottom bar on a phone. Text only (no icons), three names in view, the rest reached by scrolling the capsule sideways (no "More"
// list), the person's order, the chosen app always in view, arrow keys move and choose.
import { chromium } from "playwright";
import { BASE, serve } from "./lib.mjs";
const stop = await serve(); const b = await chromium.launch(); let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const open = async (phone, order) => { const ctx = await b.newContext(phone ? { viewport: { width: 390, height: 844 }, deviceScaleFactor: 2, isMobile: true, hasTouch: true } : { viewport: { width: 1440, height: 900 } }); await ctx.addInitScript((o) => { localStorage.setItem("cal.firstrun", "1"); if (o) localStorage.setItem("cal.appearance", JSON.stringify({ appOrder: o })); }, order); const page = await ctx.newPage(); await page.goto(`${BASE}/threads/t-run`); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(700); return { ctx, page }; };
for (const [name, phone, cls] of [["wide screen", false, "card"], ["phone", true, "bar"]]) {
  const { ctx, page } = await open(phone);
  ok((await page.locator(`nav.switcher.${cls}`).count()) === 1 && (await page.locator("nav.switcher").count()) === 1, `${name}: the one switcher component, placed as '${cls}'`);
  const t = await page.locator("nav.switcher [role=tab]").allInnerTexts();
  ok(t.join(",") === "Threads,Notes,Settings" && (await page.locator("nav.switcher svg, nav.switcher img").count()) === 0, `${name}: the apps' names, text only`, t.join(","));
  const r = await page.evaluate(() => { const bb = document.querySelector("nav.switcher .sw").getBoundingClientRect(), mid = document.querySelector(".mid-card").getBoundingClientRect(); return { bottom: bb.bottom, midBottom: mid.bottom, ih: innerHeight }; });
  ok(phone ? r.ih - r.bottom < 120 : Math.abs(r.midBottom - r.bottom) < 40, `${name}: at the ${phone ? "bottom of the screen" : "bottom of the card"}`);
  // more apps than three: the capsule scrolls sideways, three names wide, with no "More" entry
  const scroll = await page.evaluate(() => { const track = document.querySelector("nav.switcher .track"); for (const n of ["Tasks", "Files", "Maps"]) { const x = document.createElement("button"); x.type = "button"; x.setAttribute("role", "tab"); x.className = track.querySelector(".app").className; x.textContent = n; track.append(x); } return { w: track.querySelector(".app").getBoundingClientRect().width, tw: track.clientWidth, scrollable: track.scrollWidth > track.clientWidth + 4, more: [...track.querySelectorAll("button")].some((x) => /more/i.test(x.textContent)), snap: getComputedStyle(track).scrollSnapType }; });
  ok(Math.abs(scroll.w * 3 - scroll.tw) < scroll.tw * 0.12 && scroll.scrollable && !scroll.more && /x/.test(scroll.snap), `${name}: three names in view, the others reached by scrolling, snapping, no More`, `name ${Math.round(scroll.w)} px of ${scroll.tw}`);
  await page.locator("nav.switcher").getByRole("tab", { name: "Threads" }).focus(); await page.keyboard.press("ArrowRight"); await page.waitForURL(/\/notes/, { waitUntil: "commit" });
  ok(/\/notes/.test(page.url()), `${name}: the right arrow moves to the next app and opens it`);
  await ctx.close();
}
{
  const { ctx, page } = await open(false, ["settings", "threads"]);
  const t = await page.locator("nav.switcher [role=tab]").allInnerTexts();
  ok(t.join(",") === "Settings,Threads,Notes", "the person's order is the order shown", t.join(","));
  await ctx.close();
}
{
  const { ctx, page } = await open(false); await page.goto(`${BASE}/settings/appearance`); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(500);
  await page.getByRole("button", { name: "Move Notes up" }).click(); await page.waitForTimeout(300);
  const t = await page.locator("nav.switcher [role=tab]").allInnerTexts();
  ok(t.join(",") === "Notes,Threads,Settings", "Settings, Appearance, Apps: Move Notes up changes the switcher", t.join(","));
  await ctx.close();
}
await b.close(); stop(); process.exit(bad ? 1 : 0);

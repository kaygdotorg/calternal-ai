// Switching from an open thread (or note) to Settings through the app capsule shows the Settings page in the middle, and every Settings item then opens its own page there: with the card (canvas off) and with the canvas
// (canvas on, which is the default against the real server). A window for Settings used to be refused, so the sidebar followed the capsule and the middle kept the thread; direct loads of /settings/... never exercised the switch.
// Also back: the capsule's Threads returns to the thread that had focus. Usage: node e2e/settings-switch.mjs (after `npm run build`, through with-preview).
import { chromium } from "playwright";
import { BASE, serve } from "./lib.mjs";
const stop = await serve(); const b = await chromium.launch(); let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
for (const [mode, q] of [["card", ""], ["canvas", "?canvas=1"]]) for (const [from, start] of [["a thread", "/threads/t-run"], ["a note", "/notes/n-1"]]) {
  const ctx = await b.newContext({ viewport: { width: 1440, height: 900 } }); await ctx.addInitScript(() => localStorage.setItem("cal.firstrun", "1"));
  const page = await ctx.newPage(); await page.goto(`${BASE}${start}${q}`); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(1200);
  const heading = () => page.evaluate(() => { const m = document.querySelector("main.mid"); const h = [...m.querySelectorAll("h1, h2")].find((e) => e.getClientRects().length); return h ? h.textContent.trim() : ""; });
  await page.locator('aside.side footer nav[aria-label="Apps"]').getByRole("link", { name: "Settings" }).click(); await page.waitForTimeout(1200);
  ok(/\/settings/.test(page.url()) && (await page.locator("aside.side nav a").count()) > 8, `${mode}, from ${from}: the capsule's Settings opens the Settings sidebar`, page.url());
  const first = await heading();
  ok(!(await page.locator("main.mid [data-thread], main.mid .thread").count()) && first.length > 0, `${mode}, from ${from}: the middle shows a Settings page, not the ${start.split("/")[1].slice(0, -1)}`, JSON.stringify(first));
  const links = await page.locator("aside.side nav a").evaluateAll((a) => a.map((e) => ({ name: e.textContent.trim(), href: e.getAttribute("href") })));
  let wrong = [];
  for (const l of links) {
    await page.locator("aside.side nav a", { hasText: l.name }).first().click(); await page.waitForTimeout(450);
    const h = await heading(); const here = new URL(page.url()).pathname;
    if (!here.endsWith(l.href?.replace(/^.*\/settings/, "/settings") ?? "") || !h || !(h === l.name || h.includes(l.name) || l.name.includes(h) || h.length > 0)) wrong.push(`${l.name}: ${h || "(nothing)"} ${here}`);
    if (await page.locator("main.mid [data-thread], main.mid .thread").count()) wrong.push(`${l.name}: a thread is still shown`);
  }
  ok(wrong.length === 0, `${mode}, from ${from}: each of the ${links.length} Settings items shows its own page in the middle`, wrong.slice(0, 3).join(" | "));
  if (mode === "canvas") ok((await page.locator("[data-win][data-kind=settings]").count()) === 1, "canvas: Settings is one window, however many pages are opened");
  await page.locator('aside.side footer nav[aria-label="Apps"]').getByRole("link", { name: start.includes("threads") ? "Threads" : "Notes" }).click(); await page.waitForTimeout(1000);
  ok(page.url().includes(start.split("/").slice(0, 3).join("/")), `${mode}: choosing ${start.includes("threads") ? "Threads" : "Notes"} again goes back to the ${start.includes("threads") ? "thread" : "note"} you were in`, page.url());
  await ctx.close();
}
await b.close(); stop(); console.log(bad ? `${bad} FAIL` : "all pass"); process.exit(bad ? 1 : 0);

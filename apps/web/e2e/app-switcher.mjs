// The app switcher is an icon-only capsule in the sidebar's footer (account picture, capsule, search): three whole icons at a time, the outer icon smaller and fainter when more apps exist on that side, the current app
// centred, real links with names, one tab stop, arrow keys. The collapsed sidebar has a two-part pill [sidebar | current app] with a dropdown of apps. Spaces are in the account menu (a search field above five).
// Checked at 1, 3, 4 and 20 apps and 1, 3, 4 and 50 spaces, desktop and phone (fixtures: `?apps=N`, `?spaces=N`).
import { chromium } from "playwright";
import { BASE, serve } from "./lib.mjs";
const stop = await serve(); const b = await chromium.launch(); let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const open = async (phone, path, order) => {
  const ctx = await b.newContext(phone ? { viewport: { width: 390, height: 844 }, deviceScaleFactor: 2, isMobile: true, hasTouch: true } : { viewport: { width: 1440, height: 900 } });
  await ctx.addInitScript((o) => { if (o) localStorage.setItem("cal.appearance", JSON.stringify({ appOrder: o })); }, order);
  const page = await ctx.newPage(); await page.goto(`${BASE}${path}`); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(600);
  if (phone) { await page.getByRole("button", { name: /Show (the sidebar|threads|notes)/ }).first().click(); await page.waitForTimeout(800); }
  return { ctx, page };
};
const geom = (page) => page.evaluate(() => {
  const nav = document.querySelector('aside.side footer nav[aria-label="Apps"]'), track = nav.querySelector(".selector"), tr = track.getBoundingClientRect();
  const icons = [...nav.querySelectorAll(".opt")], r = (e) => e.getBoundingClientRect();
  const whole = icons.filter((a) => r(a).left >= tr.left - 1 && r(a).right <= tr.right + 1 && r(a).width > 0);
  const cur = icons.find((a) => a.getAttribute("aria-selected") === "true");
  const c = cur ? r(cur) : null;
  return { total: icons.length, whole: whole.length, outer: nav.querySelectorAll(".ic.outer").length, scrolls: track.scrollWidth > track.clientWidth + 1, centred: c ? Math.abs(c.left + c.width / 2 - (tr.left + tr.width / 2)) : 999, labels: icons.map((a) => a.getAttribute("aria-label")), current: icons.filter((a) => a.getAttribute("aria-selected") === "true").length, text: nav.innerText.trim(), tabbable: icons.filter((a) => a.tabIndex === 0).length };
});
for (const phone of [false, true]) {
  const where = phone ? "phone" : "wide screen";
  // ---- app counts
  for (const [n, route, outerWant] of [[1, "/threads", 0], [3, "/threads", 0], [4, "/threads", 1], [4, "/notes", 1], [20, "/settings/appearance", 2], [20, "/threads", 1]]) {
    const { ctx, page } = await open(phone, `${route}?apps=${n}&firstrun=0`);
    const g = await geom(page);
    if (n === 3) { const looks = await page.evaluate(() => [...document.querySelectorAll('aside.side footer nav[aria-label="Apps"] .ic')].map((a) => { const c = getComputedStyle(a); return `${c.opacity}/${c.scale}`; })); ok(looks.every((l) => l === "1/none"), `${where}, exactly 3 apps: no icon is faded or shrunk (nothing is hidden on either side)`, looks.join(" ")); }
    const wholeWant = Math.min(3, n);
    ok(g.total === n && g.whole === wholeWant, `${where}, ${n} apps (on ${route}): ${wholeWant} whole icon${wholeWant > 1 ? "s" : ""} in view, none clipped`, `${g.whole} of ${g.total}`);
    ok(g.outer === outerWant, `${where}, ${n} apps on ${route}: ${outerWant ? "the outer icon on the side with more is smaller and fainter" : "all icons full size"}`, `${g.outer} outer`);
    ok(n <= 3 ? !g.scrolls : g.scrolls, `${where}, ${n} apps: ${n <= 3 ? "nothing scrolls" : "the rest scroll sideways"}`);
    const interior = n > 3 && !(route === "/threads") && !(route === "/settings/appearance" && n === 4);
    ok(!interior || g.centred < 3, `${where}, ${n} apps on ${route}: ${interior ? "the current app is centred in the capsule" : "the current app is in view (at an end the strip rests flush)"}`, `${g.centred.toFixed(1)} px off`);
    ok(g.current === 1 && g.tabbable === 1 && g.labels.every((l) => !!l) && g.text === "", `${where}, ${n} apps: every icon is a named tab, one chosen, one tab stop, no words`);
    await ctx.close();
  }
  // ---- the glide lasts the motion token (a custom property reads back as ".22s"; read as a bare number it was 0.22 ms and the glide was instant)
  {
    const { ctx, page } = await open(phone, "/threads?apps=20&firstrun=0");
    const nav = page.locator('aside.side footer nav[aria-label="Apps"]');
    const token = await page.evaluate(() => { const e = document.createElement("i"); e.style.transitionDuration = "var(--dur-base)"; document.body.append(e); const s = getComputedStyle(e).transitionDuration; e.remove(); return parseFloat(s) * (s.endsWith("ms") ? 1 : 1000); });
    await nav.getByRole("tab", { name: "Notes" }).click(); await page.waitForTimeout(100);
    const ms = Number(await nav.locator(".selector").getAttribute("data-glide-ms"));
    ok(token >= 100 && Math.abs(ms - token) < 1, `${where}: the capsule glide lasts the --dur-base token`, `${ms} ms, token ${token} ms`);
    await ctx.close();
  }
  // ---- keys, links, current app after navigation
  {
    const { ctx, page } = await open(phone, "/threads?apps=20&firstrun=0");
    const nav = page.locator('aside.side footer nav[aria-label="Apps"]');
    await nav.getByRole("tab", { name: "Threads" }).focus(); await page.keyboard.press("ArrowRight"); await page.waitForTimeout(500);
    ok((await page.evaluate(() => document.activeElement?.getAttribute("aria-label"))) === "Notes", `${where}: the right arrow moves focus to the next app`);
    await page.keyboard.press("End"); await page.waitForTimeout(500);
    const e = await geom(page);
    ok((await page.evaluate(() => document.activeElement?.getAttribute("aria-label"))) === "App 17" && e.whole >= 1, `${where}: End goes to the last app and scrolls it into the middle`);
    await page.keyboard.press("Home"); await page.waitForTimeout(500); await page.keyboard.press("ArrowRight"); await page.waitForTimeout(400);
    await page.keyboard.press("Enter"); await page.waitForURL(/\/notes/, { waitUntil: "commit" });
    await page.waitForTimeout(700);
    const g = await geom(page);
    ok(/\/notes/.test(page.url()) && g.centred < 3, `${where}: Enter opens it and the capsule keeps the current app centred`, `${g.centred.toFixed(1)} px off`);
    await ctx.close();
  }
  // ---- spaces are in the account menu
  for (const n of [1, 3, 4, 50]) {
    const { ctx, page } = await open(phone, `/threads?spaces=${n}&firstrun=0`);
    ok((await page.locator("aside.side footer [aria-label='Spaces']").count()) === 0, `${where}, ${n} spaces: no space dots in the footer`);
    await page.getByRole("button", { name: /Account, spaces and settings/ }).click(); await page.waitForTimeout(500);
    const rows = await page.getByRole("menuitemradio").count(), search = await page.locator('input[type=search]').count();
    ok(rows === n && (await page.getByRole("menuitem", { name: "Settings" }).count()) === 1 && (await page.getByRole("menuitemradio", { checked: true }).count()) === 1, `${where}, ${n} spaces: the account menu lists them (the current one checked) and Settings`, `${rows} rows`);
    ok(n > 5 ? search === 1 : search === 0, `${where}, ${n} spaces: ${n > 5 ? "a search field" : "no search field"}`);
    if (n === 50) { await page.locator("input[type=search]").fill("Space 49"); await page.waitForTimeout(300); ok((await page.getByRole("menuitemradio").count()) === 1, `${where}, 50 spaces: the search finds one`); }
    await ctx.close();
  }
}
// ---- the person's order
{
  const { ctx, page } = await open(false, "/threads?firstrun=0", ["settings", "threads"]);
  const g = await geom(page);
  ok(g.labels.join(",") === "Settings,Threads,Notes", "the person's order is the order shown", g.labels.join(","));
  await ctx.close();
}
// ---- the collapsed sidebar: a two-part pill with a dropdown of apps
{
  const { ctx, page } = await open(false, "/threads/t-run?firstrun=0");
  await page.getByRole("button", { name: "Hide the sidebar" }).click(); await page.waitForTimeout(500);
  const pill = page.getByRole("group", { name: "Sidebar and apps" });
  ok((await pill.count()) === 1 && (await pill.getByRole("button").count()) === 2, "collapsed: one pill with two parts, the sidebar and the current app");
  await pill.getByRole("button", { name: "Apps: Threads" }).click(); await page.waitForTimeout(500);
  const rows = await page.getByRole("menuitemradio").evaluateAll((e) => e.map((x) => `${x.textContent.trim()}${x.getAttribute("aria-checked") === "true" ? "*" : ""}`));
  ok(rows.join(",") === "Threads*,Notes,Settings", "…its dropdown lists the apps with their names, the current one checked", rows.join(","));
  await page.getByRole("menuitemradio", { name: "Notes" }).click(); await page.waitForURL(/\/notes/, { waitUntil: "commit" });
  ok(/\/notes/.test(page.url()), "…and choosing one opens it");
  await pill.getByRole("button", { name: "Show the sidebar" }).click(); await page.waitForTimeout(500);
  ok((await page.getByRole("button", { name: "Hide the sidebar" }).count()) === 1, "…and the first part shows the sidebar again");
  await ctx.close();
}
// ---- Settings: Move Notes up changes the capsule
{
  const { ctx, page } = await open(false, "/settings/appearance/appearance.app-order?firstrun=0");
  await page.getByRole("button", { name: "Move Notes up" }).click(); await page.waitForTimeout(300);
  const g = await geom(page);
  ok(g.labels.join(",") === "Notes,Threads,Settings", "Settings, Appearance, Apps: Move Notes up changes the capsule", g.labels.join(","));
  await ctx.close();
}
await b.close(); stop(); process.exit(bad ? 1 : 0);

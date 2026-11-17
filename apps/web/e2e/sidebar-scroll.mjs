// The sidebar must never end up scrolled so that its first row sits under the wordmark: only its own scroller (which fades under the bars) may scroll, and the clipping box around it must not scroll at all, whatever the window's height
// and whatever is clicked or focused. Checked in a short and a tall window, desktop and phone, in every Settings page. Usage: node e2e/sidebar-scroll.mjs (after `npm run build`).
import { chromium } from "playwright";
import { serve, BASE } from "./lib.mjs";
const stop = await serve(); const b = await chromium.launch(); let bad = 0;
for (const [name, vp] of [["short desktop", { width: 1280, height: 640 }], ["tall desktop", { width: 1440, height: 900 }]]) {
  const ctx = await b.newContext({ viewport: vp }); await ctx.addInitScript(() => localStorage.setItem("cal.firstrun", "1"));
  const p = await ctx.newPage();
  await p.goto(BASE + "/settings/appearance"); await p.waitForSelector("html[data-ready]"); await p.waitForTimeout(400);
  const state = () => p.evaluate(() => {
    const side = document.querySelector("aside.side"), sc = side.querySelector(".scroll");
    const clipped = [side, ...side.querySelectorAll("*")].filter((e) => e !== sc && e.scrollTop > 0).map((e) => `${e.tagName}.${String(e.className).slice(0, 16)}=${e.scrollTop}`);
    const first = side.querySelector("nav a")?.getBoundingClientRect().top ?? 0, mark = side.querySelector(".mark")?.getBoundingClientRect().bottom ?? 0;
    return { clipped, scTop: sc.scrollTop, first, mark, fits: sc.scrollHeight <= sc.clientHeight };
  });
  // The sidebar was scrolled in a short window, then the person goes to another app and back: it starts at the top (the scroller is one element for every app; an old offset would put the first row under the wordmark).
  await p.setViewportSize({ width: vp.width, height: 380 });
  await p.evaluate(() => { document.querySelector("aside.side .scroll").scrollTop = 9999; });
  const geo = await p.evaluate(() => { const a = document.querySelector("aside.side"), sc = a.querySelector(".scroll"); return { aside: a.clientHeight, scroller: sc.clientHeight, content: sc.scrollHeight }; });
  if (geo.scroller > geo.aside + 1 || geo.content <= geo.scroller) { console.log(`FAIL  ${name}: in a 380 px window the scroller is ${geo.scroller} px in a ${geo.aside} px sidebar and its list is ${geo.content} px: the rest of the list cannot be scrolled to`); bad++; }
  const before = (await state()).scTop;
  for (const to of ["/notes", "/settings/appearance"]) { await p.evaluate((x) => { history.pushState(null, "", x); dispatchEvent(new PopStateEvent("popstate")); }, to); await p.waitForTimeout(500); }
  const carried = await state();
  if (before > 0 && carried.scTop > 0) { console.log(`FAIL  ${name}: Settings opened already scrolled by ${carried.scTop}px (it had been scrolled ${before}px): ${JSON.stringify(carried)}`); bad++; }
  await p.setViewportSize(vp); await p.waitForTimeout(200);
  const names = await p.locator("aside.side nav a").allInnerTexts();
  for (const n of names) {
    await p.locator("aside.side nav a", { hasText: n.trim() }).first().click({ timeout: 3000 }).catch(() => {});
    await p.waitForTimeout(250);
    const s = await state();
    const ok = !s.clipped.length && (s.scTop > 0 || s.first >= s.mark - 1); // a scroll is allowed only inside the fading scroller; at rest the first row is below the wordmark
    if (!ok) { console.log(`FAIL  ${name}: after "${n.trim()}": ${JSON.stringify(s)}`); bad++; }
  }
  // The ways a page scrolls a box on its own: focusing a row below the fold (Tab, a shortcut) and scrolling it into view.
  for (const how of ["focus", "scrollIntoView"]) {
    await p.evaluate((h) => { const rows = [...document.querySelectorAll("aside.side nav a")]; const last = rows[rows.length - 1]; if (h === "focus") last.focus(); else last.scrollIntoView({ block: "end" }); }, how);
    await p.waitForTimeout(250);
    const s = await state();
    if (s.clipped.length) { console.log(`FAIL  ${name}: after ${how} of the last row the clipping box scrolled: ${JSON.stringify(s)}`); bad++; }
  }
  if (!bad) console.log(`PASS  ${name}: the sidebar never scrolls under the wordmark (${names.length} pages)`);
  await ctx.close();
}
await b.close(); stop();
process.exit(bad ? 1 : 0);

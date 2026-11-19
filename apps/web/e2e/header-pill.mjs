// The header pill (packages/ui header/track.ts): as a Settings page's title and then each section heading pass the header line they appear in the pill beside the sidebar button, and leave when
// scrolled back; the page's own title and the pill never show together. Phone and desktop. Usage: node e2e/with-preview.mjs e2e/header-pill.mjs
import { launch, open, serve } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const state = (p) => p.evaluate(() => { const h = document.querySelector(".hdr"); const t = document.querySelector("[data-chrome-title]"); return { on: h?.hasAttribute("data-on") ?? false, text: h?.textContent?.trim() ?? "", title: t ? getComputedStyle(t).opacity : "", pillBox: h?.getBoundingClientRect().toJSON() }; });
try {
  for (const device of ["phone", "desktop"]) {
    const { page: p, ctx } = await open(browser, device, "light", "/settings/appearance?firstrun=0"); if (device === "desktop") { await p.setViewportSize({ width: 600, height: 700 }); await p.waitForTimeout(500); }
    const sc = p.locator(".scroller").first();
    let s = await state(p);
    ok(!s.on && s.title === "1", `${device}: at the top the title is readable and the pill is away`);
    await sc.evaluate((e) => { e.scrollTop = 120; }); await p.waitForTimeout(500);
    s = await state(p);
    ok(s.on && s.text === "Appearance" && s.title === "0", `${device}: the title has passed the line: the pill says it and the page's own title fades (never both)`, `${s.text} / ${s.title}`);
    await sc.evaluate((e) => { const h = [...document.querySelectorAll("[data-chrome-heading]")][3]; e.scrollTop = h.getBoundingClientRect().top - e.getBoundingClientRect().top + e.scrollTop + h.getBoundingClientRect().height + 80; }); await p.waitForTimeout(500);
    const want = await p.evaluate(() => [...document.querySelectorAll("[data-chrome-heading]")][3]?.textContent?.trim());
    s = await state(p);
    ok(s.on && s.text === want, `${device}: a section heading that has passed the line is in the pill`, `${s.text} (want ${want})`);
    const clear = await p.evaluate(() => { const h = document.querySelector(".hdr")?.getBoundingClientRect(), b = document.querySelector(".edge.l")?.getBoundingClientRect(); return !h || !b || h.left >= b.right - 1 || h.top >= b.bottom || h.bottom <= b.top; });
    ok(clear, `${device}: the pill does not overlap the sidebar button`);
    const mid = await p.evaluate(() => { const h = (document.querySelector("[data-win][data-focused] .mid-t") ?? document.querySelector(".hdr .hp"))?.getBoundingClientRect(), c = (document.querySelector("[data-win][data-focused]") ?? document.querySelector(".mid-card"))?.getBoundingClientRect(); return h && c ? Math.abs(h.left + h.width / 2 - (c.left + c.width / 2)) : 999; });
    ok(mid <= 2, `${device}: the pill is centred at the top of the card`, `${mid.toFixed(1)} px off`);
    await sc.evaluate((e) => { e.scrollTop = 0; }); await p.waitForTimeout(500);
    s = await state(p);
    ok(!s.on && s.title === "1", `${device}: scrolled back the pill leaves and the title returns`);
    await ctx.close();
  }
} catch (err) { console.log("FAIL  " + err.message); bad++; }
await browser.close(); await stop();
process.exit(bad ? 1 : 0);

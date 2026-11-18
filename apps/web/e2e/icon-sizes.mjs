// Every icon on the Threads surfaces is one of the two icon sizes (`--glyph` for controls, `--glyph-inline` beside text) and nothing is hand-set: the window pills, thread rows, message actions, menus, composer chips, the tabs and the Inbox marks, on a desktop and a
// phone. The page's computed sizes are read against the two tokens. Usage: CAL_URL=... node e2e/icon-sizes.mjs
import { launch, open, serve, expandComposer } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
/** The svg elements whose size is neither token (a deliberately larger illustration is marked data-art). */
const stray = (p) => p.evaluate(() => {
  const px = (v) => { const e = document.createElement("i"); e.style.cssText = `position:absolute;width:${v}`; document.body.append(e); const w = e.getBoundingClientRect().width; e.remove(); return w; };
  const ok = new Set([px("var(--glyph)"), px("var(--glyph-inline)")].map((w) => Math.round(w * 10) / 10));
  const out = [];
  for (const s of document.querySelectorAll("svg")) {
    if (s.closest(".pv, [data-art], .md img, [aria-hidden=true] > svg.art") || s.hasAttribute("data-art")) continue;
    const r = s.getBoundingClientRect(); if (!r.width) continue;
    const w = Math.round(r.width * 10) / 10;
    if (!ok.has(w)) { let e = s.parentElement; while (e && !(typeof e.className === "string" && e.className)) e = e.parentElement; out.push(`${w}x${Math.round(r.height * 10) / 10} in ${(e?.className ?? "").slice(0, 40)} (${(s.closest("[role^=menuitem]")?.innerText ?? s.parentElement?.innerText ?? "").slice(0, 30)})`); }
  }
  return [...new Set(out)];
});
try {
  for (const device of ["desktop", "phone"]) {
    const { page: p, ctx } = await open(browser, device, "light", "/threads/t-idle?canvas=1");
    await p.waitForTimeout(2500);
    if (device === "phone") await p.locator("aside.side").waitFor({ state: "attached" });
    let s = await stray(p);
    ok(s.length === 0, `${device}: every icon in the canvas windows and the sidebar is a token size`, s.join("; "));
    // menus
    await p.locator("[data-win]").first().getByRole("button", { name: /^(Thread actions|Note actions|More)$/ }).click(); await p.waitForTimeout(900);
    s = await stray(p); ok(s.length === 0, `${device}: and in the window menu`, s.join("; "));
    await p.keyboard.press("Escape");
    await p.locator("[data-win]").first().getByRole("button", { name: "Share" }).click(); await p.waitForTimeout(900);
    s = await stray(p); ok(s.length === 0, `${device}: and in the share menu`, s.join("; "));
    await p.keyboard.press("Escape");
    // composer chips
    await expandComposer(p).catch(() => {});
    s = await stray(p); ok(s.length === 0, `${device}: and in the composer with its chips`, s.join("; "));
    await ctx.close();
  }
  { const { page: p, ctx } = await open(browser, "desktop", "light", "/threads/t-idle");
    await p.waitForTimeout(800);
    const m = p.locator(".msg.user").last(); await m.scrollIntoViewIfNeeded(); await m.hover(); await p.waitForTimeout(400);
    let s = await stray(p); ok(s.length === 0, "desktop: message actions, the Inbox marks and the state marks are token sizes", s.join("; "));
    await p.getByRole("button", { name: "Thread actions" }).click(); await p.waitForTimeout(900);
    s = await stray(p); ok(s.length === 0, "desktop: and the thread menu", s.join("; "));
    await ctx.close(); }
  // The shell surfaces: sidebar footer capsule, account menu, settings list and pages, right sidebar tabs.
  for (const device of ["desktop", "phone"]) {
    const { page: p, ctx } = await open(browser, device, "light", "/settings/appearance");
    await p.waitForTimeout(1500);
    let s = await stray(p); ok(s.length === 0, `${device}: Settings (list, sections, rows) are token sizes`, s.join("; "));
    await p.goto(p.url().replace(/\/settings.*/, "/notes")); await p.waitForTimeout(1200);
    s = await stray(p); ok(s.length === 0, `${device}: the app capsule and sidebar rows of Notes are token sizes`, s.join("; "));
    const caps = await p.evaluate(() => [...document.querySelectorAll("[aria-label=Apps] svg")].map((v) => `${v.getAttribute("stroke-width") ?? getComputedStyle(v).strokeWidth}/${Math.round(v.getBoundingClientRect().width)}`));
    ok(new Set(caps).size <= 1, `${device}: capsule icons share one stroke weight and size`, caps.join(" "));
    await p.getByRole("button", { name: /Account/ }).first().click().catch(() => {}); await p.waitForTimeout(700);
    s = await stray(p); ok(s.length === 0, `${device}: and in the account menu`, s.join("; "));
    await ctx.close();
  }
} catch (err) { console.log("FAIL  " + err.message); bad++; }
await browser.close(); await stop();
process.exit(bad ? 1 : 0);

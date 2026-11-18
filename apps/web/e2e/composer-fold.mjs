// The composer expands on focus and collapses on blur (docs/threads-ui.md, "Composer expand and collapse"): the context and chips rows leave the layout unfocused and the box takes a wider inset on a phone;
// the layout flips at once and the glass, field, rows and the list's content are played from the old look over --dur-morph on --ease-morph (transform, opacity and the glass layer's edges: no layout
// per frame); the last message stays just above the composer in both states and moves with it; keyboard focus and reduced motion change at once; the morph is interruptible. Usage: CAL_URL=... node e2e/composer-fold.mjs
import { launch, open, serve } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const box = (p) => p.locator(".composer").evaluate((e) => { const r = e.getBoundingClientRect(); return { h: r.height, w: r.width, x: r.x, collapsed: e.hasAttribute("data-collapsed") }; });
const rows = (p) => p.locator(".composer .chips").evaluate((e) => ({ vis: getComputedStyle(e).display === "none" ? "hidden" : "visible", op: +getComputedStyle(e).opacity }));
/** The visible bottom of the last message and the top of the composer's glass, as drawn now (animations included). */
const gap = (p) => p.evaluate(() => { const m = [...document.querySelectorAll(".msg")].at(-1).getBoundingClientRect(); const g = document.querySelector(".composer > .glass").getBoundingClientRect(); return { msgBottom: m.bottom, glassTop: g.top, glassH: g.height, gap: g.top - m.bottom }; });
try {
  for (const device of ["desktop", "phone"]) {
    console.log(`-- ${device}`);
    const { page: p, ctx } = await open(browser, device, "dark", "/threads/t-idle");
    await p.waitForTimeout(600);
    const rest = await box(p); const r0 = await rows(p);
    ok(rest.collapsed && r0.vis === "hidden", "unfocused, the context and chips rows are folded away and out of the tab order", `h=${rest.h.toFixed(0)}`);
    const g0 = await gap(p);
    // pointer focus: sample the layout height and what is drawn (the glass, the last message) every frame
    await p.evaluate(() => { window.__s = []; const c = document.querySelector(".composer"); const t0 = performance.now(); const f = () => { const m = [...document.querySelectorAll(".msg")].at(-1).getBoundingClientRect(); const g = c.querySelector(":scope > .glass").getBoundingClientRect(); window.__s.push({ t: performance.now() - t0, layoutH: c.getBoundingClientRect().height, glassH: g.height, glassTop: g.top, msgBottom: m.bottom }); if (performance.now() - t0 < 700) requestAnimationFrame(f); }; requestAnimationFrame(f); });
    await p.locator("#composer-field").click();
    await p.waitForTimeout(800);
    const h1 = await p.evaluate(() => window.__s);
    const open1 = await box(p); const r1 = await rows(p);
    ok(!open1.collapsed && r1.vis === "visible", "focus expands it", `h ${rest.h.toFixed(0)} → ${open1.h.toFixed(0)}`);
    const flip = h1.findIndex((s) => s.layoutH > rest.h + 1);
    ok(flip >= 0 && h1.slice(flip).every((s) => Math.abs(s.layoutH - open1.h) < 1), "the layout is in its new form from the first frame that changed (one layout, not one per frame)");
    const done = h1.find((s) => Math.abs(s.glassH - open1.h) < 0.6)?.t ?? -1;
    console.log(`   expand: ${h1.length} frames, the glass reached its final height at ${done.toFixed(0)} ms (from ${h1[0].glassH.toFixed(0)})`);
    ok(done > 150 && done < 450, "the drawn expansion runs about --dur-morph (350 ms), not instant, not slow");
    const g1 = await gap(p);
    ok(Math.abs(g1.gap - g0.gap) < 2, "the last message sits just above the composer in both states (no gap left)", `gap ${g0.gap.toFixed(0)} → ${g1.gap.toFixed(0)}`);
    const dev = Math.max(...h1.map((s) => Math.abs((s.glassTop - s.msgBottom) - g0.gap)));
    ok(dev < 12, "and they move together through the morph", `largest deviation ${dev.toFixed(1)} px`);
    ok(h1[0].msgBottom > g1.msgBottom + 1 || Math.abs(open1.h - rest.h) < 2, "the last message starts where it was and slides up with the composer");
    // blur: click on the list
    await p.mouse.click(device === "phone" ? 200 : 900, 200);
    await p.waitForTimeout(700);
    const back = await box(p); const g2 = await gap(p);
    ok(back.collapsed && Math.abs(back.h - rest.h) < 1 && Math.abs(g2.gap - g0.gap) < 2, "blur collapses it to the same height, the message back just above it", `h=${back.h.toFixed(0)} gap ${g2.gap.toFixed(0)}`);
    // interrupt: focus, then blur 120 ms in; what is drawn must never jump
    await p.evaluate(() => { window.__s = []; const c = document.querySelector(".composer"); const t0 = performance.now(); const f = () => { const g = c.querySelector(":scope > .glass").getBoundingClientRect(); const m = [...document.querySelectorAll(".msg")].at(-1).getBoundingClientRect(); window.__s.push([g.height, m.bottom]); if (performance.now() - t0 < 900) requestAnimationFrame(f); }; requestAnimationFrame(f); });
    await p.locator("#composer-field").click(); await p.waitForTimeout(120); await p.mouse.click(device === "phone" ? 200 : 900, 200); await p.waitForTimeout(900);
    const hs = await p.evaluate(() => window.__s); let jump = 0, mj = 0; for (let i = 1; i < hs.length; i++) { jump = Math.max(jump, Math.abs(hs[i][0] - hs[i - 1][0])); mj = Math.max(mj, Math.abs(hs[i][1] - hs[i - 1][1])); }
    ok(jump < 40 && mj < 40, "an interrupted expansion reverses without a jump", `biggest step glass ${jump.toFixed(0)} px, message ${mj.toFixed(0)} px over ${hs.length} frames`);
    // keyboard focus: Tab into the composer from outside; the change is instant (keyboard use never animates)
    await p.mouse.click(device === "phone" ? 200 : 900, 200); await p.waitForTimeout(500);
    for (let i = 0; i < 60 && (await p.evaluate(() => document.activeElement?.id)) !== "composer-field"; i++) await p.keyboard.press(i === 0 ? "Tab" : "Tab");
    await p.waitForTimeout(40);
    const kb = await box(p);
    ok((await p.evaluate(() => document.activeElement?.id)) === "composer-field" && !kb.collapsed && Math.abs(kb.h - open1.h) < 1.5, "Tab into the composer expands it at once (no animation for the keyboard)", `h=${kb.h.toFixed(0)}`);
    await p.keyboard.press("Shift+Tab"); await p.keyboard.press("Shift+Tab"); await p.waitForTimeout(40);
    console.log(`   Shift+Tab out: collapsed=${(await box(p)).collapsed}`);
    await ctx.close();
  }
  // reduced motion: a change at once
  const { page: q, ctx: c2 } = await open(browser, "desktop", "dark", "/threads/t-idle");
  await q.emulateMedia({ reducedMotion: "reduce" }); await q.reload(); await q.waitForSelector("html[data-ready]"); await q.waitForTimeout(500);
  await q.locator("#composer-field").click(); await q.waitForTimeout(60);
  const rm = await box(q); ok(!rm.collapsed && (await rows(q)).vis === "visible", "reduced motion: expanded within a few frames", `h=${rm.h.toFixed(0)}`);
  await c2.close();
} finally { await browser.close(); stop(); }
process.exit(bad ? 1 : 0);

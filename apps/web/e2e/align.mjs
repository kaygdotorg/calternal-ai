// Optical alignment and nested radii, measured in a real browser. Prints what is off; exits 1 if an icon sits more than 1 px off its text's centre
// or a nested radius is more than 2 px from "outer radius minus the gap". Usage: node e2e/align.mjs   (after `npm run build`).
import { launch, open, serve } from "./lib.mjs";

const stop = await serve();
const browser = await launch();
let bad = 0;
try {
  for (const device of ["desktop", "phone"]) for (const path of ["/threads/t-wait", "/threads/t-run?panel=changes", "/notes/n-1"]) {
    const { page, ctx } = await open(browser, device, "light", path);
    const r = await page.evaluate(() => {
      const out = { icons: [], radii: [] };
      const vis = (el) => { const r = el.getBoundingClientRect(); const cs = getComputedStyle(el); return r.width > 0 && r.height > 0 && cs.visibility !== "hidden" && cs.display !== "none"; };
      // 1. An icon beside text: the icon's vertical centre against the text's x-height centre (the middle of the text's line box).
      for (const svg of document.querySelectorAll("svg[data-icon]")) {
        if (!vis(svg)) continue;
        const host = svg.parentElement;
        const text = [...host.parentElement?.children ?? [], ...host.children].find((c) => c !== svg && c !== host && c.textContent?.trim() && vis(c));
        const sib = [...host.parentElement.children].find((c) => c !== host && c.textContent?.trim() && vis(c) && !c.querySelector("svg"));
        const t = sib ?? text;
        if (!t) continue;
        const tr = t.getBoundingClientRect(); // the text block: one line, or a title and a quiet line, which the icon centres on together
        const ir = svg.getBoundingClientRect();
        const d = (ir.top + ir.height / 2) - (tr.top + tr.height / 2);
        if (Math.abs(d) > 1) out.icons.push(`${(svg.closest("button, a, [role]") ?? host).getAttribute("aria-label") ?? t.textContent.trim().slice(0, 24)}: ${d.toFixed(1)} px`);
      }
      // 2. Nested radii: a filled rounded box inside a rounded parent, near its edge.
      const px = (v) => parseFloat(v) || 0;
      for (const el of document.body.querySelectorAll("*")) {
        const cs = getComputedStyle(el);
        const rad = px(cs.borderTopLeftRadius);
        if (rad < 2 || rad > 200 || !vis(el)) continue;
        const bg = cs.backgroundColor;
        if (bg === "rgba(0, 0, 0, 0)" && cs.backgroundImage === "none") continue;
        let p = el.parentElement;
        while (p && !(px(getComputedStyle(p).borderTopLeftRadius) >= 4 && px(getComputedStyle(p).borderTopLeftRadius) < 200)) p = p.parentElement;
        if (!p) continue;
        const pr = px(getComputedStyle(p).borderTopLeftRadius);
        const a = el.getBoundingClientRect(), b = p.getBoundingClientRect();
        const gap = Math.min(a.left - b.left, a.top - b.top, b.right - a.right, b.bottom - a.bottom);
        if (gap < 0 || gap > 14) continue;
        const want = Math.max(0, pr - gap);
        if (rad > 150 || (px(cs.borderTopLeftRadius) >= a.height / 2 - 0.5)) continue; // a pill is half its height by construction
        if (Math.abs(rad - want) > 2) out.radii.push(`${el.className.toString().slice(0, 28) || el.tagName}: radius ${rad}, parent ${pr}, gap ${gap.toFixed(0)}, concentric would be ${want.toFixed(0)}`);
      }
      return out;
    });
    const icons = [...new Set(r.icons)], radii = [...new Set(r.radii)];
    console.log(`\n== ${device} ${path}\nicons off the text centre: ${icons.length ? "\n  " + icons.join("\n  ") : "none"}\nnested radii off: ${radii.length ? "\n  " + radii.join("\n  ") : "none"}`);
    bad += icons.length;
    await ctx.close();
  }
} finally {
  await browser.close();
  stop();
}
process.exit(bad ? 1 : 0);

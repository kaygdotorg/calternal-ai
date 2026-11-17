// Optical alignment and nested radii, measured in a real browser. Prints what is off; exits 1 if an icon sits more than 1 px off its text's centre
// or a nested radius is more than 2 px from "outer radius minus the gap". Usage: node e2e/align.mjs   (after `npm run build`).
import { launch, open, serve } from "./lib.mjs";

const measure = () => {
  const out = { icons: [], radii: [] };
  const vis = (el) => { const r = el.getBoundingClientRect(); const cs = getComputedStyle(el); return r.width > 0 && r.height > 0 && cs.visibility !== "hidden" && cs.display !== "none"; };
  // 1. An icon beside text: the icon's vertical centre against the text's x-height centre (the middle of the text's line box).
  for (const svg of document.querySelectorAll("svg[data-icon]")) {
    if (!vis(svg)) continue;
    const ir = svg.getBoundingClientRect();
    // The row the icon belongs to: the nearest control or list item around it. Its text is the leaf text elements that share the icon's line
    // (their vertical span overlaps the icon's); the nearest one sideways is the label the icon is read with. Text on another line is ignored.
    const row = svg.closest("button, a, [role=menuitem], [role=tab], li, label, summary") ?? svg.parentElement;
    if (!row) continue;
    let best = null, dist = Infinity;
    const walker = document.createTreeWalker(row, NodeFilter.SHOW_TEXT);
    for (let n = walker.nextNode(); n; n = walker.nextNode()) {
      if (!n.textContent.trim()) continue;
      const el = n.parentElement;
      if (!el || el.closest("svg") || !vis(el)) continue;
      const tr = el.getBoundingClientRect();
      const overlap = Math.min(tr.bottom, ir.bottom) - Math.max(tr.top, ir.top);
      if (overlap < Math.min(tr.height, ir.height) * 0.5) continue; // not on the icon's line
      const dx = Math.max(0, tr.left - ir.right, ir.left - tr.right);
      if (dx < dist) { dist = dx; best = el; }
    }
    if (!best || dist > 24) continue;
    // A title with a quiet line under it is one block: the icon centres on the stack (lines with the same left edge, touching the first).
    let top = best.getBoundingClientRect().top, bottom = best.getBoundingClientRect().bottom;
    const left = best.getBoundingClientRect().left;
    for (let grew = true; grew;) {
      grew = false;
      const w2 = document.createTreeWalker(row, NodeFilter.SHOW_TEXT);
      for (let n = w2.nextNode(); n; n = w2.nextNode()) {
        const el = n.parentElement;
        if (!n.textContent.trim() || !el || !vis(el)) continue;
        const r2 = el.getBoundingClientRect();
        if (Math.abs(r2.left - left) > 2) continue;
        if (r2.top >= top - 4 && r2.bottom <= bottom + 8 + r2.height && (r2.bottom > bottom + 0.5 || r2.top < top - 0.5)) { top = Math.min(top, r2.top); bottom = Math.max(bottom, r2.bottom); grew = true; }
      }
    }
    const d = (ir.top + ir.height / 2) - (top + bottom) / 2;
    if (Math.abs(d) > 1) out.icons.push(`${(row.getAttribute("aria-label") ?? best.textContent.trim().slice(0, 24))}: ${d.toFixed(1)} px`);
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
    if (gap < 2 || gap > 14) continue; // under 2 px is a border or ring, not an inset
    const want = Math.max(0, pr - gap);
    if (rad > 150 || (px(cs.borderTopLeftRadius) >= a.height / 2 - 0.5)) continue; // a pill is half its height by construction
    if (Math.abs(rad - want) > 2) out.radii.push(`${el.className.toString().slice(0, 28) || el.tagName}: radius ${rad}, parent ${pr}, gap ${gap.toFixed(0)}, concentric would be ${want.toFixed(0)}`);
  }
  return out;
};

const stop = await serve();
const browser = await launch();
let bad = 0;
try {
// Self-test: a deliberately 3 px off icon must be reported, a centred one must not, or the findings below mean nothing.
{
  const { page, ctx } = await open(browser, "desktop", "light", "/threads/t-run");
  await page.evaluate(() => {
    document.body.insertAdjacentHTML("beforeend", '<div id="st" style="position:fixed;top:5px;left:5px;z-index:99999;background:#fff"><button style="display:flex;align-items:center;height:40px"><svg data-icon width="16" height="16" style="position:relative;top:3px"></svg><span>Off by three</span></button><button style="display:flex;align-items:center;height:40px"><svg data-icon width="16" height="16"></svg><span>Centred</span></button></div>');
  });
  const r = await page.evaluate(measure);
  const hit = r.icons.filter((x) => x.startsWith("Off by three")).length, miss = r.icons.filter((x) => x.startsWith("Centred")).length;
  if (hit !== 1 || miss !== 0) { console.error(`align self-test failed: reported off=${hit}, centred=${miss}`); process.exit(2); }
  console.log("self-test: ok (a 3 px offset is caught, a centred icon is not)");
  await ctx.close();
}

  for (const device of ["desktop", "phone"]) for (const path of ["/threads/t-wait", "/threads/t-run?panel=changes", "/notes/n-1", "/settings/appearance", "/threads/t-long"]) {
    const { page, ctx } = await open(browser, device, "light", path);
    const r = await page.evaluate(measure);
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

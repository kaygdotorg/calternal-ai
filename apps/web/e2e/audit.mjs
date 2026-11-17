// Design audit in a real browser: fonts, type scale, weights, spacing scale and (on a phone) hit targets. Prints what is off-scale.
// Usage: node e2e/audit.mjs   (after `npm run build`). Exits 1 if any family other than the two allowed ones is drawn, or a phone hit target is under 44 px.
import { launch, open, serve } from "./lib.mjs";

const stop = await serve();
const browser = await launch();
let bad = 0;
const paths = ["/threads/t-wait", "/threads/t-run", "/notes/n-1"];
try {
  for (const device of ["desktop", "phone"]) {
    const seen = { fonts: new Set(), sizes: new Map(), weights: new Set(), spacing: new Map(), small: [], icons: new Map() };
    for (const path of paths) for (const scheme of ["light", "dark"]) {
      const { page, ctx } = await open(browser, device, scheme, path);
      const r = await page.evaluate((isPhone) => {
        const root = getComputedStyle(document.documentElement);
        const scale = ["--s-1", "--s-2", "--s-3", "--s-4", "--s-5", "--s-6", "--s-7", "--s-8", "--s-9", "--s-10"].map((n) => parseFloat(root.getPropertyValue(n)));
        const out = { fonts: [], sizes: {}, weights: [], spacing: {}, small: [], scale, icons: {} };
        for (const v of document.querySelectorAll("svg[data-icon]")) { const r = v.getBoundingClientRect(); if (!r.width) continue; const sw = parseFloat(getComputedStyle(v).strokeWidth) * r.width / 24; const k = `${Math.round(r.width)}px / ${sw.toFixed(1)}px line`; out.icons[k] = (out.icons[k] ?? 0) + 1; }
        const px = (v) => (v.endsWith("px") ? parseFloat(v) : NaN);
        for (const el of document.body.querySelectorAll("*")) {
          const cs = getComputedStyle(el);
          const r = el.getBoundingClientRect();
          if (!r.width || !r.height || cs.visibility === "hidden" || cs.display === "none") continue;
          if (el.closest("svg")) { continue; }
          if ([...el.childNodes].some((n) => n.nodeType === 3 && n.textContent.trim())) {
            out.fonts.push(cs.fontFamily.split(",")[0].replace(/"/g, ""));
            out.sizes[cs.fontSize] = (out.sizes[cs.fontSize] ?? 0) + 1;
            out.weights.push(cs.fontWeight);
          }
          for (const p of ["paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "rowGap", "columnGap"]) {
            const v = px(cs[p]);
            if (v > 0) out.spacing[v] = (out.spacing[v] ?? 0) + 1;
          }
          if (isPhone && (el.matches("button, a[href], input, select, textarea, [role=tab], [role=radio], [role=switch]")) && !el.closest(".md, p") && (r.width < 43.5 || r.height < 43.5) && !el.classList.contains("visually-hidden")) out.small.push(`${el.tagName.toLowerCase()}[${el.getAttribute("aria-label") ?? el.textContent.trim().slice(0, 24)}] ${Math.round(r.width)}x${Math.round(r.height)}`);
        }
        return out;
      }, device === "phone");
      r.fonts.forEach((f) => seen.fonts.add(f));
      for (const [k, n] of Object.entries(r.sizes)) seen.sizes.set(k, (seen.sizes.get(k) ?? 0) + n);
      r.weights.forEach((w) => seen.weights.add(w));
      for (const [k, n] of Object.entries(r.spacing)) seen.spacing.set(+k, (seen.spacing.get(+k) ?? 0) + n);
      for (const [k, n] of Object.entries(r.icons)) seen.icons.set(k, (seen.icons.get(k) ?? 0) + n);
      seen.scale = r.scale;
      for (const s of r.small) seen.small.push(`${path} ${scheme} ${s}`);
      await ctx.close();
    }
    const allowedFonts = new Set(["Google Sans Flex", "Google Sans Flex Dark", "Google Sans Flex Rounded", "Maple Mono"]);
    const wrongFonts = [...seen.fonts].filter((f) => !allowedFonts.has(f));
    const off = [...seen.spacing].filter(([v]) => !seen.scale.some((s) => Math.abs(s - v) < 0.01)).sort((a, b) => b[1] - a[1]);
    console.log(`\n== ${device}`);
    console.log("fonts drawn:", [...seen.fonts].join(", "), wrongFonts.length ? `   OFF-LIST: ${wrongFonts.join(", ")}` : "");
    console.log("font sizes (px:count):", [...seen.sizes].sort((a, b) => parseFloat(a[0]) - parseFloat(b[0])).map(([k, n]) => `${parseFloat(k).toFixed(1)}:${n}`).join("  "));
    console.log("weights:", [...seen.weights].sort().join(", "));
    console.log("icon sizes / rendered line (count):", [...seen.icons].map(([k, n]) => `${k} (${n})`).join("   "));
    console.log("spacing off the s-1..s-10 scale (px:count):", off.map(([v, n]) => `${v}:${n}`).join("  ") || "none");
    if (device === "phone") console.log("hit targets under 44 px:", seen.small.length ? "\n  " + [...new Set(seen.small)].join("\n  ") : "none");
    bad += wrongFonts.length + (device === "phone" ? new Set(seen.small).size : 0);
  }
} finally {
  await browser.close();
  stop();
}
process.exit(bad ? 1 : 0);

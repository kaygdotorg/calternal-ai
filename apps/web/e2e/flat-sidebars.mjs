// The sidebars are flat: with the text taken away, no column between the middle card and a sidebar differs from the sidebar's own surface (no strip of bare wallpaper in the gap beside the
// card, no hard edge where a veil starts). Measured on rendered pixels with bright wallpapers in both schemes. The check is proven able to fail: with the veil cut back to the sidebar's own
// width (how it used to be) it finds the edge. Usage: node e2e/flat-sidebars.mjs (after `npm run build`).
import { launch, open, serve } from "./lib.mjs";
const IDS = (process.env.IDS ?? "fbV03MdVtJA,BnOdWOMW7Is,Nkwrpa_v0WA,EX_STserBZM").split(",");
const STEP = 3; // the largest change of the mean colour from one pixel column to the next, in 0..255, that still counts as one continuous surface
const stop = await serve(); const browser = await launch(); let bad = 0;
const measure = async (page) => {
  const rects = await page.evaluate(() => { document.head.insertAdjacentHTML("beforeend", "<style>aside.side > *, aside.right > *, .mid-card > * { visibility: hidden !important } .toasts, .edge { display: none }</style>"); const r = (s) => document.querySelector(s).getBoundingClientRect().toJSON(); return { card: r(".mid-card"), left: r("aside.side"), right: r("aside.right"), w: innerWidth, h: innerHeight }; });
  await page.waitForTimeout(200);
  const png = (await page.screenshot()).toString("base64");
  return page.evaluate(async ([b64, R]) => {
    const img = new Image(); img.src = "data:image/png;base64," + b64; await img.decode();
    const cv = document.createElement("canvas"); cv.width = img.width; cv.height = img.height; const ctx = cv.getContext("2d", { willReadFrequently: true }); ctx.drawImage(img, 0, 0);
    const sc = img.width / R.w, y0 = Math.round(120 * sc), hh = Math.round((R.h - 300) * sc);
    const col = (x) => { const d = ctx.getImageData(Math.round(x * sc), y0, 1, hh).data; let a = 0; for (let i = 0; i < d.length; i += 4) a += (d[i] + d[i + 1] + d[i + 2]) / 3; return a / (d.length / 4); };
    // The span from just outside the card (the card casts its own shadow in its first few pixels, which belongs to the card) across the gap and into the sidebar, both sides.
    const span = (from, to) => { let worst = 0, at = from; let prev = col(from); for (let x = from + 1 / sc; x <= to; x += 1 / sc) { const c = col(x); if (Math.abs(c - prev) > worst) { worst = Math.abs(c - prev); at = x; } prev = c; } return { worst, at }; };
    return { right: span(R.card.right + 6, R.right.left + 40), left: span(R.left.right - 40, R.card.left - 6) };
  }, [png, rects]);
};
try {
  for (const scheme of ["light", "dark"]) for (const id of IDS) {
    const { page, ctx } = await open(browser, "desktop", scheme, "/threads/t-run?panel=changes");
    await page.evaluate((w) => localStorage.setItem("cal.appearance", JSON.stringify({ wallpaperId: w })), id);
    await page.reload(); await page.waitForSelector("html[data-ready]");
    await page.waitForFunction(() => getComputedStyle(document.documentElement).getPropertyValue("--c-dim-a").trim() !== "" && getComputedStyle(document.documentElement).getPropertyValue("--wall-card").includes("blob"), null, { timeout: 15000 });
    await page.waitForTimeout(500);
    const m = await measure(page);
    const good = m.right.worst <= STEP && m.left.worst <= STEP;
    console.log(`${good ? "PASS" : "FAIL"}  ${scheme.padEnd(5)} ${id.padEnd(12)} largest step right ${m.right.worst.toFixed(2)} (x ${m.right.at.toFixed(0)}), left ${m.left.worst.toFixed(2)} (x ${m.left.at.toFixed(0)})`);
    if (!good) bad++;
    await ctx.close();
  }
  // The check can fail: cut the veils back to the sidebars' own widths and it must see the edge.
  const { page, ctx } = await open(browser, "desktop", "light", "/threads/t-run?panel=changes");
  await page.evaluate(() => localStorage.setItem("cal.appearance", JSON.stringify({ wallpaperId: "fbV03MdVtJA" }))); await page.reload(); await page.waitForSelector("html[data-ready]");
  await page.waitForFunction(() => getComputedStyle(document.documentElement).getPropertyValue("--c-dim-a").trim() !== "", null, { timeout: 15000 }); await page.waitForTimeout(500);
  await page.addStyleTag({ content: ".wash.l { width: var(--side-w) !important } .wash.r { width: var(--right-w) !important }" });
  const m = await measure(page);
  const sees = m.right.worst > STEP || m.left.worst > STEP;
  console.log(`${sees ? "PASS" : "FAIL"}  the check finds the old edge (largest step ${Math.max(m.right.worst, m.left.worst).toFixed(2)})`);
  if (!sees) bad++;
  await ctx.close();
} finally { await browser.close(); stop(); }
console.log(bad ? `${bad} FAIL` : "all pass"); process.exit(bad ? 1 : 0);

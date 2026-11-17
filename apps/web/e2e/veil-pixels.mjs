// Does the veil deliver what it promises, measured on RENDERED pixels (not the model)? For each of the eight photographs in both schemes: hide the text, screenshot,
// then measure the first percentile of the contrast of each surface's real ink colours against the pixels the browser actually drew under it
// (backdrop, veil layers, grain and all). Targets from docs/ui-craft-and-settings-study.md: primary 7, ink-2 5.5, ink-3 4.5, with 0.3 of slack for the
// rendering (jpeg, grain, the 48 by 48 sampling the solver does). Usage: node e2e/veil-pixels.mjs   (after `npm run build`). Exits 1 on any miss.
import { launch, open, serve } from "./lib.mjs";

const ONLY = process.env.ONLY?.split(",");
const SHOTS = process.env.SHOTS; // a directory: keeps the text-less screenshots
const IDS = ["fbV03MdVtJA", "BnOdWOMW7Is", "Nkwrpa_v0WA", "B8zfbse7mOc", "_zDcx4dD7L0", "EX_STserBZM", "ngWVYP9NIYM", "I_xBR4NmDhU"];
const SLACK = 0.3, TARGET = { ink: 7, ink2: 5.5, ink3: 4.5 };
const stop = await serve();
const browser = await launch();
let bad = 0;
const rows = [];
try {
  for (const scheme of ["light", "dark"]) for (const id of IDS) {
    if (ONLY && !ONLY.includes(`${scheme}:${id}`)) continue;
    const { page, ctx } = await open(browser, "desktop", scheme, "/threads/t-run?panel=changes");
    await page.evaluate((w) => localStorage.setItem("cal.appearance", JSON.stringify({ wallpaperId: w })), id);
    await page.reload();
    await page.waitForSelector("html[data-ready]");
    await page.waitForFunction(() => getComputedStyle(document.documentElement).getPropertyValue("--c-dim-a").trim() !== "" && getComputedStyle(document.documentElement).getPropertyValue("--wall-card").includes("blob"), null, { timeout: 15000 });
    await page.waitForTimeout(500);
    // The inks each surface really has, read where they apply; then the text goes (the grounds stay).
    const inks = await page.evaluate(() => {
      const probe = (host, v) => { const e = document.createElement("span"); e.style.cssText = `color:var(${v});position:absolute`; host.append(e); const c = getComputedStyle(e).color; e.remove(); return c.match(/[\d.]+/g).slice(0, 3).map(Number); };
      const out = {};
      for (const [k, sel] of [["left", "aside.side"], ["right", "aside.right"], ["card", "main.mid"]]) { const h = document.querySelector(sel); out[k] = { ink: probe(h, "--ink"), ink2: probe(h, "--ink-2"), ink3: probe(h, "--ink-3"), flip: h.getAttribute("data-ink") }; out[k].rect = (k === "card" ? document.querySelector(".mid-card") : h).getBoundingClientRect().toJSON(); }
      document.head.insertAdjacentHTML("beforeend", "<style>aside.side > *, aside.right > *, .mid-card > * { visibility: hidden !important } .toasts { display: none }</style>");
      return out;
    });
    await page.waitForTimeout(150);
    const shot = await page.screenshot();
    if (SHOTS) (await import("node:fs")).mkdirSync(SHOTS, { recursive: true }), (await import("node:fs")).writeFileSync(`${SHOTS}/${scheme}-${id}.png`, shot);
    const png = shot.toString("base64");
    const res = await page.evaluate(async ([b64, inks]) => {
      const img = new Image(); img.src = "data:image/png;base64," + b64; await img.decode();
      const cv = document.createElement("canvas"); cv.width = img.width; cv.height = img.height;
      const ctx = cv.getContext("2d", { willReadFrequently: true }); ctx.drawImage(img, 0, 0);
      const lin = (v) => ((v /= 255) <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);
      const lum = (c) => 0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2]);
      const ratio = (a, b) => (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
      const out = {};
      for (const [k, v] of Object.entries(inks)) {
        const r = v.rect, inset = 12, sc = img.width / innerWidth; // away from the rounded corners and the card's shadow; the screenshot is at device pixels
        const x0 = Math.round((r.left + inset) * sc), y0 = Math.round((r.top + inset) * sc), w = Math.round((r.width - 2 * inset) * sc), h = Math.round((r.height - 2 * inset) * sc);
        const d = ctx.getImageData(x0, y0, w, h).data;
        const lums = [];
        for (let i = 0; i < d.length; i += 4 * 3) lums.push(lum([d[i], d[i + 1], d[i + 2]]));
        const p01 = (c) => { const rs = lums.map((l) => ratio(l, lum(c))).sort((a, b) => a - b); return rs[Math.floor(0.01 * (rs.length - 1))]; };
        out[k] = { ink: p01(v.ink), ink2: p01(v.ink2), ink3: p01(v.ink3), flip: v.flip };
      }
      return out;
    }, [png, inks]);
    const miss = [];
    for (const [k, v] of Object.entries(res)) for (const t of ["ink", "ink2", "ink3"]) if (v[t] < TARGET[t] - SLACK) miss.push(`${k} ${t} ${v[t].toFixed(2)} < ${TARGET[t]}`);
    const f = (k) => `${res[k].ink.toFixed(1)}/${res[k].ink2.toFixed(1)}/${res[k].ink3.toFixed(1)}${res[k].flip ? " flip:" + res[k].flip : ""}`;
    console.log(`${miss.length ? "FAIL" : "ok  "} ${scheme.padEnd(5)} ${id.padEnd(12)} left ${f("left")}  card ${f("card")}  right ${f("right")}${miss.length ? "\n   " + miss.join("; ") : ""}`);
    bad += miss.length;
    await ctx.close();
  }
} finally {
  await browser.close();
  stop();
}
process.exit(bad ? 1 : 0);

// One continuous background (docs/ui-shell.md, "The background"): the wallpaper under ONE veil spans the whole window, both sidebars are drawn on it, and the middle card is the only thing on top. Nothing may paint in the gap
// beside the card or behind its rounded corners, so every pixel outside the card must be exactly what it is with no card at all. Measured directly on pixels: the window is captured twice, with the card and with the card
// invisible (`visibility: hidden`, which also takes away its shadow), text, icons and the sidebars' own content hidden in both; every pixel in the 40 px band outside the card's rounded edge (more than 2 px away, so the
// card's own antialiased edge is not counted) is compared. A seam is any difference: a shadow, a second veil ending at the card, a backdrop of the card's own, a clip. Both schemes, the eight wallpapers, 1x and 2x
// (the gate runs fewer: SEAM_FULL=1 for all). Allowed: SEAM_MAX levels of 255 (default 1: the capture is deterministic, so the two should be equal).
import { chromium } from "playwright";
import { readFileSync } from "node:fs";
import { BASE, serve } from "./lib.mjs";
const IDS = JSON.parse(readFileSync("wallpapers.json", "utf8")).map((w) => w.id);
const FULL = !!process.env.SEAM_FULL, MAX = Number(process.env.SEAM_MAX ?? 1);
const stop = await serve(); const b = await chromium.launch(); let bad = 0, worstAll = 0;
const cases = [];
for (const scheme of ["light", "dark"]) for (const id of FULL ? IDS : [IDS[0], IDS[3]]) for (const dpr of FULL ? [1, 2] : id === IDS[0] ? [1, 2] : [1]) cases.push({ scheme, id, dpr });
for (const { scheme, id, dpr } of cases) {
  const ctx = await b.newContext({ viewport: { width: 1500, height: 970 }, deviceScaleFactor: dpr, colorScheme: scheme });
  await ctx.addInitScript(([id]) => { localStorage.setItem("cal.firstrun", "1"); localStorage.setItem("cal.appearance", JSON.stringify({ wallpaperId: id })); }, [id]);
  const page = await ctx.newPage(); await page.goto(`${BASE}/threads/t-run?panel=changes`); await page.waitForSelector("html[data-ready]");
  await page.waitForFunction(() => getComputedStyle(document.documentElement).getPropertyValue("--c-dim-a").trim() !== "" && document.documentElement.style.getPropertyValue("--wall-side").includes("blob"), null, { timeout: 15000 }).catch(() => {});
  await page.waitForTimeout(1200);
  await page.addStyleTag({ content: "aside > *, .edge, .mid-card > * { visibility: hidden !important; } *, *::before, *::after { caret-color: transparent !important; animation: none !important; transition: none !important; }" });
  await page.waitForTimeout(400);
  const g = await page.evaluate(() => { const c = document.querySelector(".mid-card"), r = c.getBoundingClientRect(); return { x0: r.left, x1: r.right, y0: r.top, y1: r.bottom, radius: parseFloat(getComputedStyle(c).borderTopLeftRadius) || 0, iw: innerWidth, ih: innerHeight }; });
  const withCard = (await page.screenshot()).toString("base64");
  await page.addStyleTag({ content: ".mid-card { visibility: hidden !important; }" }); await page.waitForTimeout(300);
  const without = (await page.screenshot()).toString("base64");
  const res = await page.evaluate(async ([a, b, g, dpr]) => {
    const load = async (png) => { const img = await createImageBitmap(await (await fetch(`data:image/png;base64,${png}`)).blob()); const c = document.createElement("canvas"); c.width = img.width; c.height = img.height; const x = c.getContext("2d", { willReadFrequently: true }); x.drawImage(img, 0, 0); return x.getImageData(0, 0, img.width, img.height); };
    const A = await load(a), B = await load(b), W = A.width, H = A.height, R = g.radius;
    const cx = (g.x0 + g.x1) / 2, cy = (g.y0 + g.y1) / 2, hx = (g.x1 - g.x0) / 2, hy = (g.y1 - g.y0) / 2;
    const sdf = (px, py) => { const qx = Math.abs(px - cx) - (hx - R), qy = Math.abs(py - cy) - (hy - R); return Math.hypot(Math.max(qx, 0), Math.max(qy, 0)) + Math.min(Math.max(qx, qy), 0) - R; };
    const regions = { "top-left": 0, "top-right": 0, "bottom-left": 0, "bottom-right": 0, "left gap": 0, "right gap": 0 }, where = {};
    for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
      const px = (x + 0.5) / dpr, py = (y + 0.5) / dpr, d = sdf(px, py);
      if (d < 2 || d > 40) continue;
      const i = (y * W + x) * 4; const df = Math.max(Math.abs(A.data[i] - B.data[i]), Math.abs(A.data[i + 1] - B.data[i + 1]), Math.abs(A.data[i + 2] - B.data[i + 2]));
      const nearTop = py < g.y0 + R, nearBottom = py > g.y1 - R, left = px < cx;
      const name = nearTop ? (left ? "top-left" : "top-right") : nearBottom ? (left ? "bottom-left" : "bottom-right") : left ? "left gap" : "right gap";
      if (df > regions[name]) { regions[name] = df; where[name] = [Math.round(px), Math.round(py)]; }
    }
    // And the background itself, with no card in the way, must be continuous across the lines where the card's edges are: the old build had a veil per sidebar that ended exactly there (a step of 30 levels or more); a
    // photograph changes a level or two over 6 px. The 90th percentile of the step over the card's height, at both vertical edges.
    const mean = (x, y) => { const X = Math.round(x * dpr) - 1, Y = Math.round(y * dpr) - 1; let s = [0, 0, 0]; for (let j = 0; j < 3; j++) for (let i = 0; i < 3; i++) { const k = ((Y + j) * W + X + i) * 4; s[0] += B.data[k]; s[1] += B.data[k + 1]; s[2] += B.data[k + 2]; } return s.map((v) => v / 9); };
    const steps = [];
    for (const xe of [g.x0, g.x1]) for (let y = 8; y < g.ih - 8; y += 6) { const a1 = mean(xe - 3, y), b1 = mean(xe + 3, y); steps.push(Math.max(Math.abs(a1[0] - b1[0]), Math.abs(a1[1] - b1[1]), Math.abs(a1[2] - b1[2]))); }
    steps.sort((p, q) => p - q);
    return [...Object.entries(regions).map(([k, v]) => [k, v, where[k] ?? null]), ["step across the card's edge lines (p90)", +steps[Math.floor(steps.length * 0.9)].toFixed(1), null]];
  }, [withCard, without, g, dpr]);
  const worst = Math.max(...res.filter((r) => !r[0].startsWith("step")).map((r) => r[1])); worstAll = Math.max(worstAll, worst);
  const fail = res.filter((r) => r[1] > (r[0].startsWith("step") ? 10 : MAX));
  console.log(`${fail.length ? "FAIL" : "PASS"}  ${scheme} ${id.slice(0, 6)} ${dpr}x   ${res.map((r) => `${r[0]} ${r[1]}${r[1] > MAX && r[2] ? ` at ${r[2]}` : ""}`).join(", ")}`);
  if (fail.length) bad++;
  await ctx.close();
}
console.log(`worst difference (outside the card, with and without it): ${worstAll} levels of 255 (allowed ${MAX})`);
await b.close(); stop(); process.exit(bad ? 1 : 0);

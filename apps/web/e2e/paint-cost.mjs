// Paint cost, measured from a Chromium trace while the 10,000 message thread scrolls (120 steps of 900 px). Sums the duration of the trace events that
// are the cost of drawing (Paint, RasterTask, Layerize, Commit, UpdateLayoutTree, Layout) and counts the live backdrop-filter surfaces with their area.
// Variants: the shipped build; the same with every small glass surface's live blur removed; the old approach (a live 75 px blur over the whole card).
// Development-host numbers (headless Chromium, software compositing). Usage: node e2e/paint-cost.mjs   (after `npm run build`).
import { launch, open, serve } from "./lib.mjs";

const COST = new Set(["Paint", "RasterTask", "ImageDecodeTask", "Layerize", "Commit", "UpdateLayoutTree", "Layout", "PrePaint", "CompositeLayers"]);
const VARIANTS = {
  "shipped (pre-blurred wallpaper; live blur on small glass)": "",
  "no live blur anywhere": ".glass, [data-radius] .glass { backdrop-filter: none !important; -webkit-backdrop-filter: none !important; }",
  "old approach: live 75 px blur over the whole card": ".mid-card { backdrop-filter: blur(75px) saturate(140%); -webkit-backdrop-filter: blur(75px) saturate(140%); } .mid-card::before { background: none !important; }",
};
const stop = await serve();
const browser = await launch();
const out = [];
try {
  for (const [name, css] of Object.entries(VARIANTS)) {
    const { page, ctx } = await open(browser, "desktop", "light", "/threads/t-long");
    if (css) await page.addStyleTag({ content: css });
    await page.waitForSelector('[role="feed"] [role="article"]');
    const live = await page.evaluate(() => {
      let n = 0, area = 0;
      for (const el of document.querySelectorAll("*")) {
        const cs = getComputedStyle(el);
        if (cs.backdropFilter !== "none" && cs.backdropFilter) { const r = el.getBoundingClientRect(); n++; area += r.width * r.height; }
      }
      return { surfaces: n, percentOfViewport: +((100 * area) / (innerWidth * innerHeight)).toFixed(1) };
    });
    const cdp = await ctx.newCDPSession(page);
    const events = [];
    cdp.on("Tracing.dataCollected", (d) => events.push(...d.value));
    await cdp.send("Tracing.start", { categories: "devtools.timeline,disabled-by-default-devtools.timeline", transferMode: "ReportEvents" });
    await page.evaluate(async () => {
      const sc = document.querySelector('[role="feed"]');
      for (let i = 0; i < 120; i++) { sc.scrollTop -= 900; await new Promise((r) => requestAnimationFrame(r)); }
    });
    const done = new Promise((r) => cdp.once("Tracing.tracingComplete", r));
    await cdp.send("Tracing.end");
    await done;
    const by = {};
    for (const e of events) if (e.ph === "X" && COST.has(e.name) && e.dur) by[e.name] = (by[e.name] ?? 0) + e.dur / 1000;
    const total = Object.values(by).reduce((a, b) => a + b, 0);
    out.push({ variant: name, live, total_ms: +total.toFixed(0), per_frame_ms: +(total / 120).toFixed(2), by_event_ms: Object.fromEntries(Object.entries(by).map(([k, v]) => [k, +v.toFixed(0)])) });
    await ctx.close();
  }
} finally {
  await browser.close();
  stop();
}
console.log(JSON.stringify(out, null, 1));

// Scroll and streaming performance of the thread view on the 10,000 message fixture (docs/ui-performance.md).
// Usage: node e2e/perf.mjs   (after `npm run build`). Numbers are development-host numbers (headless Chromium, software compositing).
import { launch, open, serve } from "./lib.mjs";

const stop = await serve();
const browser = await launch();
const result = {};
try {
  const { page, ctx } = await open(browser, "desktop", "light", "/threads/t-long");
  if (process.env.LIVE_BLUR) await page.addStyleTag({ content: ".mid-card { -webkit-backdrop-filter: blur(75px) saturate(140%); backdrop-filter: blur(75px) saturate(140%); } .mid-card::before { background: none !important; }" }); // the approach this replaces: a live blur over the whole card
  await page.waitForSelector('[role="feed"] [role="article"]');
  const count = () => page.evaluate(() => ({ dom: document.getElementsByTagName("*").length, rows: document.querySelectorAll('[role="feed"] [role="article"]').length }));
  result.dom_at_end = await count();
  result.total_messages = await page.evaluate(() => Number(document.querySelector('[role="feed"] [role="article"]')?.getAttribute("aria-setsize")));

  // Frame times in three phases: a steady scroll up from the end (120 steps of 900 px), 40 random jumps anywhere in the thread, a steady scroll down.
  const phases = await page.evaluate(async () => {
    const sc = document.querySelector('[role="feed"]');
    const raf = () => new Promise((r) => requestAnimationFrame(r));
    const run = async (n, step) => {
      const times = [];
      let last = await new Promise((r) => requestAnimationFrame(r));
      for (let i = 0; i < n; i++) { step(i); const t = await new Promise((r) => requestAnimationFrame(r)); times.push(t - last); last = t; }
      return times;
    };
    const up = await run(120, () => { sc.scrollTop -= 900; });
    const jump = await run(40, () => { sc.scrollTop = Math.random() * (sc.scrollHeight - sc.clientHeight); });
    const down = await run(120, () => { sc.scrollTop += 700; });
    return { up, jump, down };
  });
  const stats = (frames) => { const s = [...frames].sort((a, b) => a - b); const q = (p) => s[Math.min(s.length - 1, Math.floor(p * s.length))]; return { frames: frames.length, median_ms: +q(0.5).toFixed(1), p95_ms: +q(0.95).toFixed(1), max_ms: +Math.max(...frames).toFixed(1), over_20ms: frames.filter((f) => f > 20).length }; };
  result.scroll = { steady_up: stats(phases.up), random_jumps: stats(phases.jump), steady_down: stats(phases.down) };
  result.dom_after_scroll = await count();

  // Jump to the top and read the dom count there too (the unmeasured, estimated region).
  await page.evaluate(() => { document.querySelector('[role="feed"]').scrollTop = 0; });
  await page.waitForTimeout(300);
  result.dom_at_top = await count();
  await ctx.close();

  // Choosing a wallpaper: from the click to the new blurred copies being in place (two canvas renders, one 24 by 14 read, the wash): the one-off cost
  // that replaces a per-frame blur. Five picks.
  const w = await open(browser, "desktop", "light", "/settings/appearance");
  await w.page.waitForSelector('[role="radiogroup"][aria-label="Wallpaper"] [role="radio"]');
  await w.page.waitForFunction(() => getComputedStyle(document.documentElement).getPropertyValue("--wall-side").length > 0);
  const picks = [];
  for (const name of ["Aurora", "Ember", "Moss", "Slate", "Lagoon"]) {
    const before = await w.page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue("--wash"));
    const t0 = Date.now();
    await w.page.getByRole("radio", { name, exact: true }).click();
    await w.page.waitForFunction((b) => getComputedStyle(document.documentElement).getPropertyValue("--wash") !== b, before, { polling: "raf", timeout: 5000 });
    picks.push(Date.now() - t0);
  }
  result.wallpaper_pick_ms = { runs: picks, note: "click to applied, includes the 60 ms debounce and Playwright round trips" };
  await w.ctx.close();

  // Streaming: how many elements change while a message streams (the list must not re-render).
  const s = await open(browser, "desktop", "light", "/threads/t-run");
  await s.page.waitForSelector('[role="feed"] [role="article"]');
  await s.page.waitForTimeout(1500);
  const stream = await s.page.evaluate(async () => {
    const root = document.querySelector('[role="feed"]');
    let mutations = 0, nodesTouched = new Set();
    const mo = new MutationObserver((ms) => { for (const m of ms) { mutations++; nodesTouched.add(m.target.parentElement?.closest("[data-i]")?.dataset.i ?? "?"); } });
    mo.observe(root, { subtree: true, childList: true, characterData: true, attributes: true });
    const t0 = performance.now(); const longTasks = [];
    const po = new PerformanceObserver((l) => longTasks.push(...l.getEntries().map((e) => e.duration)));
    try { po.observe({ type: "longtask", buffered: false }); } catch {}
    await new Promise((r) => setTimeout(r, 4000));
    mo.disconnect(); po.disconnect();
    return { seconds: (performance.now() - t0) / 1000, mutations, rowsTouched: [...nodesTouched], longTasks };
  });
  result.streaming = stream;
  await s.ctx.close();

  // Streaming at a faster pace on the long thread: rows other than the stream row must not be touched.
  console.log(JSON.stringify(result, null, 2));
} finally {
  await browser.close();
  stop();
}

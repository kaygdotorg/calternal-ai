// Profiles the first Clarity step (Settings > Appearance, "Clear to tinted", Home key at 4x CPU) with a Chrome trace, for the default path (canvasPrefs.enabled true in the build) and the override path
// (sessionStorage cal.canvas.on with enabled false), and prints each one's top native events by SELF time with counts. Needs a build with VITE_CANVAS_DEFAULT unset after the flip, or any build for the override path.
// Usage: node e2e/with-preview.mjs e2e/trace-first-step.mjs   (CAL_TRACE_VARIANTS=default,override)
import { launch, serve, BASE } from "./lib.mjs";
const stop = await serve(); const b = await launch();
const variants = (process.env.CAL_TRACE_VARIANTS ?? "default,override").split(",");
for (const variant of variants) {
  const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, colorScheme: "light" });
  await ctx.addInitScript((v) => { localStorage.setItem("cal.firstrun", "1"); if (v === "override") { sessionStorage.setItem("cal.canvas.on", "1"); localStorage.setItem("cal.canvas.prefs", JSON.stringify({ look: "card", width: 1, enabled: false })); } if (v === "default") localStorage.setItem("cal.canvas.prefs", JSON.stringify({ look: "card", width: null, enabled: true })); }, variant);
  const page = await ctx.newPage();
  await page.goto(`${BASE}/settings/appearance/appearance.clarity?firstrun=0`); await page.waitForSelector("html[data-ready]");
  await page.waitForFunction(() => getComputedStyle(document.documentElement).getPropertyValue("--wall-side").includes("blob"), null, { timeout: 20000 }); await page.waitForTimeout(800);
  const info = await page.evaluate(() => ({ wins: document.querySelectorAll("[data-win]").length, pv: document.querySelectorAll(".pv").length, els: document.querySelectorAll("*").length, canvasOn: !!document.querySelector("[data-win]") }));
  const slider = page.getByRole("slider", { name: /Clear to tinted/ }); await slider.scrollIntoViewIfNeeded(); await slider.focus();
  const cdp = await ctx.newCDPSession(page); await cdp.send("Emulation.setCPUThrottlingRate", { rate: 4 });
  const events = []; cdp.on("Tracing.dataCollected", (e) => events.push(...e.value));
  await cdp.send("Tracing.start", { categories: "devtools.timeline,disabled-by-default-devtools.timeline,cc,gpu", transferMode: "ReportEvents" });
  await page.keyboard.press("Home"); await page.waitForTimeout(1200);
  const done = new Promise((r) => cdp.once("Tracing.tracingComplete", r)); await cdp.send("Tracing.end"); await done;
  // Self time per event name on each thread: durations of complete events minus those of their children (nesting by start time).
  const byThread = new Map();
  for (const e of events) if (e.ph === "X" && e.dur != null) { const k = `${e.pid}:${e.tid}`; (byThread.get(k) ?? byThread.set(k, []).get(k)).push(e); }
  const self = new Map(), stats = {};
  for (const list of byThread.values()) {
    list.sort((a, c) => a.ts - c.ts || c.dur - a.dur);
    const stack = [];
    for (const e of list) {
      while (stack.length && stack.at(-1).end <= e.ts) stack.pop();
      const top = stack.at(-1); if (top) top.child += e.dur;
      stack.push({ e, end: e.ts + e.dur, child: 0 }); e.__n = stack.at(-1);
    }
    for (const e of list) { const s = Math.max(0, e.dur - e.__n.child); const a = self.get(e.name) ?? [0, 0]; a[0] += s; a[1]++; self.set(e.name, a);
      if (e.name === "UpdateLayoutTree" && e.dur > 5000) stats.style = [...(stats.style ?? []), `${(e.dur / 1000).toFixed(0)}ms elements=${e.args?.elementCount ?? "?"}`];
      if (e.name === "Layout" && e.dur > 5000) stats.layout = [...(stats.layout ?? []), `${(e.dur / 1000).toFixed(0)}ms dirty=${e.args?.beginData?.dirtyObjects ?? e.args?.dirtyObjects ?? "?"}/${e.args?.beginData?.totalObjects ?? e.args?.totalObjects ?? "?"}`]; }
  }
  const skip = /^(ThreadControllerImpl|RunMicrotasks|HandlePostMessage)/;
  const top = [...self].filter(([n]) => !skip.test(n)).sort((a, c) => c[1][0] - a[1][0]).slice(0, 12).map(([n, v]) => `${(v[0] / 1000).toFixed(0).padStart(5)} ms  x${String(v[1]).padEnd(4)} ${n}`);
  const long = events.filter((e) => e.ph === "X" && e.name === "RunTask" && e.dur > 50000).map((e) => (e.dur / 1000).toFixed(0)).join(", ");
  console.log(`\n== ${variant}: ${JSON.stringify(info)}  tasks>50ms: [${long}]\n${top.join("\n")}\n  style: ${(stats.style ?? []).join(" | ")}\n  layout: ${(stats.layout ?? []).join(" | ")}`);
  await ctx.close();
}
await b.close(); await stop();

// What the composer's 350 ms morph costs the main thread (4x CPU): a trace of the window from the click that focuses the field, summed by kind of work (layout, style, paint, script, commit),
// plus a CPU profile's top script functions. Usage: CAL_URL=... node e2e/profile-fold.mjs [desktop|phone]
import { chromium } from "playwright";
import { BASE } from "./lib.mjs";
const device = process.argv[2] ?? "phone";
const b = await chromium.launch();
const ctx = await b.newContext({ viewport: device === "phone" ? { width: 390, height: 844 } : { width: 1500, height: 970 }, deviceScaleFactor: 2, isMobile: device === "phone", hasTouch: device === "phone" });
await ctx.addInitScript(() => localStorage.setItem("cal.firstrun", "1"));
const page = await ctx.newPage(); const cdp = await ctx.newCDPSession(page);
await page.goto(BASE + "/threads/t-idle"); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(1500);
await cdp.send("Emulation.setCPUThrottlingRate", { rate: 4 });
const fb = await page.locator("#composer-field").boundingBox();
await cdp.send("Profiler.enable"); await cdp.send("Profiler.setSamplingInterval", { interval: 200 });
await b.startTracing(page, { categories: ["devtools.timeline", "disabled-by-default-devtools.timeline", "blink", "cc", "viz"] });
await cdp.send("Profiler.start");
await page.mouse.click(fb.x + 20, fb.y + 10);
await page.waitForTimeout(700);
const { profile } = await cdp.send("Profiler.stop");
const buf = await b.stopTracing(); const trace = JSON.parse(buf.toString());
const sums = {}; for (const e of trace.traceEvents ?? trace) { if (e.ph !== "X" || !e.dur) continue; if (!["Layout", "UpdateLayoutTree", "PrePaint", "Paint", "Layerize", "Commit", "FunctionCall", "FireAnimationFrame", "RunTask", "ResizeObserver", "HitTest", "CompositeLayers", "UpdateLayer"].includes(e.name)) continue; sums[e.name] = (sums[e.name] ?? 0) + e.dur / 1000; }
console.log(`${device} morph window 700 ms at 4x: ` + Object.entries(sums).map(([k, v]) => `${k} ${v.toFixed(1)} ms`).join(", "));
const self = new Map(); const byId = new Map(profile.nodes.map((n) => [n.id, n])); const dt = profile.timeDeltas;
profile.samples.forEach((id, i) => { const n = byId.get(id); const k = `${n.callFrame.functionName || "(anon)"} ${n.callFrame.url.split("/").slice(-1)[0]}:${n.callFrame.lineNumber + 1}`; self.set(k, (self.get(k) ?? 0) + (dt[i] ?? 0)); });
for (const [k, v] of [...self].sort((a, c) => c[1] - a[1]).filter(([k]) => !/idle|program/.test(k)).slice(0, 10)) console.log(`${(v / 1000).toFixed(1).padStart(7)} ms  ${k}`);
await b.close();

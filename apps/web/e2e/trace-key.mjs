// Main-thread work by kind (style, layout, paint, script) for one key in the composer at 4x, from a Chrome trace. Usage: node e2e/trace-key.mjs <first|second|at>
import { chromium } from "playwright";
import { serve, open } from "./lib.mjs";
const WHAT = process.argv[2] ?? "first";
const stop = await serve(); const b = await chromium.launch();
const { page, ctx } = await open(b, "desktop", "light", "/threads/t-idle?test=1");
await page.waitForSelector("#composer-field");
const cdp = await ctx.newCDPSession(page); await cdp.send("Emulation.setCPUThrottlingRate", { rate: 4 });
const f = page.locator("#composer-field"); await f.click();
if (WHAT !== "first") { await f.pressSequentially(WHAT === "at" ? "hello there " : WHAT === "atp" ? "hello there @" : "hel", { delay: 60 }); await page.waitForTimeout(300); }
await page.waitForTimeout(400);
const events = []; cdp.on("Tracing.dataCollected", (d) => events.push(...d.value));
await cdp.send("Tracing.start", { categories: "devtools.timeline,disabled-by-default-devtools.timeline", transferMode: "ReportEvents" });
await page.keyboard.type(WHAT === "at" ? "@" : WHAT === "atp" ? "P" : "t", { delay: 80 }); await page.waitForTimeout(300);
const done = new Promise((r) => cdp.once("Tracing.tracingComplete", r)); await cdp.send("Tracing.end"); await done;
const sum = new Map();
for (const e of events) if (e.ph === "X" && e.dur && /^(UpdateLayoutTree|Layout|PrePaint|Paint|Layerize|Commit|HitTest|FunctionCall|EventDispatch|RunMicrotasks|TimerFire|FireAnimationFrame|UpdateLayer|ScheduleStyleRecalculation|StyleRecalc|IntersectionObserverController::computeIntersections)$/.test(e.name)) sum.set(e.name, (sum.get(e.name) ?? 0) + e.dur / 1000);
console.log([...sum.entries()].sort((a, c) => c[1] - a[1]).map(([k, v]) => `${v.toFixed(2).padStart(7)} ms  ${k}`).join("\n"));
const ul = events.filter((e) => e.name === "UpdateLayoutTree" && e.dur > 500).map((e) => `${(e.dur / 1000).toFixed(1)}ms elements=${e.args?.elementCount ?? "?"} ${e.args?.beginData?.stackTrace?.[0]?.functionName ?? ""}`);
console.log("slow style recalcs:", ul.join(" | "));
await b.close(); stop();

// Where composer typing time goes: a CPU profile of typing a line in the thread composer at 4x, self time per function per key, and main-thread time by kind (script, style, layout, paint)
// from a trace over the same keys. Usage: node e2e/profile-composer.mjs <distDir> [topN]  (run from apps/web; serves the given build on port 4397)
import { spawn } from "node:child_process";
import { launch } from "./lib.mjs";
const [DIR = "dist", TOP = "25"] = process.argv.slice(2);
const base = "http://127.0.0.1:4397";
const srv = spawn("npx", ["vite", "preview", "--outDir", DIR, "--port", "4397", "--strictPort", "--host", "127.0.0.1"], { stdio: "ignore", detached: true });
for (let i = 0; i < 60; i++) { try { if ((await fetch(base)).ok) break; } catch { /* not yet */ } await new Promise((r) => setTimeout(r, 250)); }
const b = await launch();
const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });
await ctx.addInitScript(() => { try { localStorage.setItem("cal.firstrun", "1"); } catch {} });
const page = await ctx.newPage(); const cdp = await ctx.newCDPSession(page);
await page.goto(base + "/threads/t-idle?test=1"); await page.waitForSelector("#composer-field"); await page.waitForTimeout(1500);
await cdp.send("Emulation.setCPUThrottlingRate", { rate: 4 });
await page.locator("#composer-field").click(); await page.waitForTimeout(300);
const TEXT = "the quick brown fox jumps over the lazy dog, then naps friday five. ".repeat(2);
await cdp.send("Profiler.enable"); await cdp.send("Profiler.setSamplingInterval", { interval: 100 }); await cdp.send("Profiler.start");
await page.keyboard.type(TEXT, { delay: 50 }); await page.waitForTimeout(300);
const { profile } = await cdp.send("Profiler.stop");
const self = new Map(); const byId = new Map(profile.nodes.map((n) => [n.id, n]));
const dt = profile.timeDeltas; let total = 0;
profile.samples.forEach((id, i) => { const n = byId.get(id); const k = `${n.callFrame.functionName || "(anon)"} ${n.callFrame.url.split("/").pop()}:${n.callFrame.lineNumber}`; self.set(k, (self.get(k) ?? 0) + (dt[i] ?? 0) / 1000); total += (dt[i] ?? 0) / 1000; });
const keys = TEXT.length;
const rows = [...self.entries()].sort((a, c) => c[1] - a[1]).slice(0, +TOP).map(([k, ms]) => `${(ms / keys).toFixed(3).padStart(7)} ms/key  ${k}`);
console.log(`total sampled ${(total / keys).toFixed(2)} ms/key (idle included)\n` + rows.join("\n"));
// By kind, from a trace over a second line.
await cdp.send("Tracing.start", { categories: "devtools.timeline,disabled-by-default-devtools.timeline", transferMode: "ReturnAsStream" });
await page.keyboard.type(TEXT.slice(0, 30), { delay: 50 }); await page.waitForTimeout(300);
const done = new Promise((r) => cdp.once("Tracing.tracingComplete", r));
await cdp.send("Tracing.end"); const { stream } = await done;
let text = ""; for (;;) { const c = await cdp.send("IO.read", { handle: stream }); text += c.data; if (c.eof) break; }
const ev = JSON.parse(text).traceEvents ?? JSON.parse(text);
const kinds = { script: /^(FunctionCall|EvaluateScript|EventDispatch|TimerFire|FireAnimationFrame|RunMicrotasks|v8\.callFunction)$/, style: /^(UpdateLayoutTree|RecalculateStyles)$/, layout: /^(Layout|IntersectionObserverController::computeIntersections)$/, paint: /^(Paint|PrePaint|Layerize|Commit|UpdateLayer|CompositeLayers)$/ };
const sum = Object.fromEntries(Object.keys(kinds).map((k) => [k, 0]));
for (const e of ev) if (e.ph === "X" && e.dur) for (const [k, re] of Object.entries(kinds)) if (re.test(e.name)) sum[k] += e.dur / 1000;
console.log("main thread by kind, ms per key (events can nest, script includes what it calls): " + Object.entries(sum).map(([k, v]) => `${k} ${(v / 30).toFixed(2)}`).join(", "));
await b.close(); try { process.kill(-srv.pid); } catch { /* gone */ }

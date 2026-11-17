// What the first key (or the key that opens a picker) costs: a CPU profile of just those keys in the composer at 4x. Usage: node e2e/profile-key.mjs <first|at> [topN]
import { chromium } from "playwright";
import { serve, open } from "./lib.mjs";
const WHAT = process.argv[2] ?? "first", TOP = +(process.argv[3] ?? 25);
const stop = await serve(); const b = await chromium.launch();
const { page, ctx } = await open(b, "desktop", "light", "/threads/t-idle?test=1");
await page.waitForSelector("#composer-field");
const cdp = await ctx.newCDPSession(page); await cdp.send("Emulation.setCPUThrottlingRate", { rate: 4 });
const f = page.locator("#composer-field"); await f.click();
if (WHAT !== "first") { await f.pressSequentially(WHAT === "atp" ? "hello there @" : "hello there ", { delay: 60 }); await page.waitForTimeout(500); }
await cdp.send("Profiler.enable"); await cdp.send("Profiler.setSamplingInterval", { interval: 50 }); await cdp.send("Profiler.start");
await page.keyboard.type(WHAT === "at" ? "@" : WHAT === "atp" ? "P" : "t", { delay: 80 }); await page.waitForTimeout(250);
const { profile } = await cdp.send("Profiler.stop");
const byId = new Map(profile.nodes.map((n) => [n.id, n])); const parent = new Map(); for (const n of profile.nodes) for (const c of n.children ?? []) parent.set(c, n.id);
const self = new Map(), incl = new Map(); const dt = profile.timeDeltas;
const nm = (n) => `${n.callFrame.functionName || "(anon)"} ${n.callFrame.url.split("/").pop()}:${n.callFrame.lineNumber}`;
profile.samples.forEach((id, i) => { const ms = (dt[i] ?? 0) / 1000; const n = byId.get(id); self.set(nm(n), (self.get(nm(n)) ?? 0) + ms); const seen = new Set(); for (let p = id; p; p = parent.get(p)) { const k = nm(byId.get(p)); if (!seen.has(k)) { seen.add(k); incl.set(k, (incl.get(k) ?? 0) + ms); } } });
const top = (m) => [...m.entries()].filter(([k]) => !/^\((idle|program|root)\)/.test(k)).sort((a, c) => c[1] - a[1]).slice(0, TOP).map(([k, ms]) => `${ms.toFixed(2).padStart(7)} ms  ${k}`).join("\n");
console.log(`-- self time\n${top(self)}\n-- inclusive\n${top(incl)}`);
await b.close(); stop();

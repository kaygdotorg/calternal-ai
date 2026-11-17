import { spawn } from "node:child_process";
import { chromium } from "playwright";
import { DEVICES } from "./lib.mjs";
const srv = spawn("npx", ["vite", "preview", "--port", "4196", "--strictPort", "--host", "127.0.0.1", "--outDir", "/tmp/dist-nomin"], { stdio: "ignore", detached: true });
const BASE = "http://127.0.0.1:4196"; for (let i = 0; i < 60; i++) { try { if ((await fetch(BASE)).ok) break; } catch {} await new Promise((r) => setTimeout(r, 250)); }
const b = await chromium.launch();
const ctx = await b.newContext({ ...DEVICES.desktop }); const page = await ctx.newPage(); const cdp = await ctx.newCDPSession(page);
await page.goto(BASE + "/threads/t-idle?firstrun=0"); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(3000);
await cdp.send("Emulation.setCPUThrottlingRate", { rate: 4 });
await cdp.send("Profiler.enable"); await cdp.send("Profiler.setSamplingInterval", { interval: 200 }); await cdp.send("Profiler.start");
await page.getByRole("button", { name: /Account and settings/ }).click(); await page.waitForSelector(".page h1"); await page.waitForTimeout(600);
const { profile } = await cdp.send("Profiler.stop");
const byId = new Map(profile.nodes.map((n) => [n.id, n])); const parent = new Map(); for (const n of profile.nodes) for (const c of n.children ?? []) parent.set(c, n.id);
const self = new Map(), incl = new Map(); const dt = profile.timeDeltas; const nm = (n) => `${n.callFrame.functionName || "(anon)"} ${n.callFrame.url.split("/").pop()}:${n.callFrame.lineNumber}`;
profile.samples.forEach((id, i) => { const ms = (dt[i] ?? 0) / 1000; self.set(nm(byId.get(id)), (self.get(nm(byId.get(id))) ?? 0) + ms); const seen = new Set(); for (let p = id; p; p = parent.get(p)) { const k = nm(byId.get(p)); if (!seen.has(k)) { seen.add(k); incl.set(k, (incl.get(k) ?? 0) + ms); } } });
const top = (m, n) => [...m.entries()].filter(([k]) => !/idle|root|program/.test(k)).sort((a, c) => c[1] - a[1]).slice(0, n).map(([k, v]) => `${v.toFixed(1).padStart(7)} ms ${k}`).join("\n");
console.log("-- self\n" + top(self, 8) + "\n-- inclusive\n" + top(incl, 22));
await b.close(); try { process.kill(-srv.pid); } catch {}

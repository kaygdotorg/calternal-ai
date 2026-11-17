// Where typing time goes: CPU profile of typing in a note at 4x, self time per function per key. Usage: node e2e/profile-typing.mjs <appDir> [path] [topN]
import { spawn } from "node:child_process";
import { launch } from "./lib.mjs";
const [DIR, PATH = "/notes/n-long", TOP = "22"] = process.argv.slice(2);
const base = "http://127.0.0.1:4193";
const srv = spawn("npx", ["vite", "preview", "--port", "4193", "--strictPort", "--host", "127.0.0.1"], { cwd: DIR, stdio: "ignore", detached: true });
for (let i = 0; i < 60; i++) { try { if ((await fetch(base)).ok) break; } catch { /* not yet */ } await new Promise((r) => setTimeout(r, 250)); }
const b = await launch();
const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });
await ctx.addInitScript(() => { try { localStorage.setItem("cal.firstrun", "1"); } catch {} });
const page = await ctx.newPage(); const cdp = await ctx.newCDPSession(page);
await page.goto(base + PATH + "?test=1"); await page.waitForSelector(".cm-content"); await page.waitForTimeout(1500);
await cdp.send("Emulation.setCPUThrottlingRate", { rate: 4 });
await page.locator(".cm-content").click(); await page.keyboard.press("Control+End"); await page.waitForTimeout(500);
await cdp.send("Profiler.enable"); await cdp.send("Profiler.setSamplingInterval", { interval: 100 }); await cdp.send("Profiler.start");
const TEXT = "the quick brown fox jumps over the lazy dog, then naps. ".repeat(2);
await page.keyboard.type(TEXT, { delay: 50 }); await page.waitForTimeout(300);
const { profile } = await cdp.send("Profiler.stop");
const self = new Map(); const byId = new Map(profile.nodes.map((n) => [n.id, n]));
const dt = profile.timeDeltas; let total = 0;
profile.samples.forEach((id, i) => { const n = byId.get(id); const k = `${n.callFrame.functionName || "(anon)"} ${n.callFrame.url.split("/").pop()}:${n.callFrame.lineNumber}`; self.set(k, (self.get(k) ?? 0) + (dt[i] ?? 0) / 1000); total += (dt[i] ?? 0) / 1000; });
const keys = TEXT.length;
const rows = [...self.entries()].sort((a, c) => c[1] - a[1]).slice(0, +TOP).map(([k, ms]) => `${(ms / keys).toFixed(3).padStart(7)} ms/key  ${k}`);
console.log(`total sampled ${(total / keys).toFixed(2)} ms/key (idle included)\n` + rows.join("\n"));
await b.close(); try { process.kill(-srv.pid); } catch { /* gone */ }

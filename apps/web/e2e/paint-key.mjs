// What paints when a key is typed in the thread composer: a trace over 30 keys at 4x, every Paint event with its clip rectangle (the area repainted), the layer it painted, and the raster and layout totals.
// A rectangle the size of the composer card means the whole card repaints per key; one the size of a line of text means only the text does.
// Usage: node e2e/paint-key.mjs <distDir>   (run from apps/web; serves the build on port 4399)
import { spawn } from "node:child_process";
import { launch } from "./lib.mjs";
const DIR = process.argv[2] ?? "dist";
const base = "http://127.0.0.1:4399";
const srv = spawn("npx", ["vite", "preview", "--outDir", DIR, "--port", "4399", "--strictPort", "--host", "127.0.0.1"], { stdio: "ignore", detached: true });
for (let i = 0; i < 60; i++) { try { if ((await fetch(base)).ok) break; } catch { /* not yet */ } await new Promise((r) => setTimeout(r, 250)); }
const b = await launch();
const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });
await ctx.addInitScript(() => { try { localStorage.setItem("cal.firstrun", "1"); } catch {} });
const page = await ctx.newPage(); const cdp = await ctx.newCDPSession(page);
await page.goto(base + "/threads/t-idle"); await page.waitForSelector("#composer-field"); await page.waitForTimeout(1500);
const box = await page.locator(".composer").boundingBox(); const mirror = await page.locator(".composer .mirror").boundingBox().catch(() => null);
console.log(`composer card ${Math.round(box.width)}x${Math.round(box.height)} css px at (${Math.round(box.x)},${Math.round(box.y)}); mirror ${mirror ? Math.round(mirror.width) + "x" + Math.round(mirror.height) : "none"}`);
await cdp.send("Emulation.setCPUThrottlingRate", { rate: 4 });
await page.locator("#composer-field").click(); await page.waitForTimeout(500);
await cdp.send("Tracing.start", { categories: "devtools.timeline,disabled-by-default-devtools.timeline,cc,viz", transferMode: "ReturnAsStream" });
const TEXT = "the quick brown fox jumps over the lazy dog";
await page.keyboard.type(TEXT.slice(0, 30), { delay: 60 }); await page.waitForTimeout(400);
const done = new Promise((r) => cdp.once("Tracing.tracingComplete", r));
await cdp.send("Tracing.end"); const { stream } = await done;
let text = ""; for (;;) { const c = await cdp.send("IO.read", { handle: stream }); text += c.data; if (c.eof) break; }
const ev = JSON.parse(text).traceEvents ?? JSON.parse(text);
const rects = new Map(); let paints = 0, paintMs = 0, raster = 0, layout = 0, layoutN = 0, style = 0, prepaint = 0;
for (const e of ev) {
  if (e.ph !== "X") continue;
  if (e.name === "Paint") { paints++; paintMs += (e.dur ?? 0) / 1000; const c = e.args?.data?.clip; if (c) { const w = Math.round((c[2] - c[0]) / 2), h = Math.round((c[5] - c[1]) / 2); const k = `${w}x${h} css px, ${e.args.data.nodeName ?? "?"}`; rects.set(k, (rects.get(k) ?? 0) + 1); } }
  else if (e.name === "RasterTask") raster += (e.dur ?? 0) / 1000;
  else if (e.name === "Layout") { layout += (e.dur ?? 0) / 1000; layoutN++; }
  else if (e.name === "UpdateLayoutTree") style += (e.dur ?? 0) / 1000;
  else if (e.name === "PrePaint") prepaint += (e.dur ?? 0) / 1000;
}
const n = 30;
console.log(`per key: Paint ${(paintMs / n).toFixed(2)} ms (${(paints / n).toFixed(1)} paint events), PrePaint ${(prepaint / n).toFixed(2)}, raster ${(raster / n).toFixed(2)}, layout ${(layout / n).toFixed(2)} (${(layoutN / n).toFixed(1)} runs), style ${(style / n).toFixed(2)}`);
console.log("repainted areas (count over 30 keys):\n" + [...rects.entries()].sort((a, c) => c[1] - a[1]).slice(0, 12).map(([k, v]) => `  ${String(v).padStart(4)}  ${k}`).join("\n"));
await b.close(); try { process.kill(-srv.pid); } catch { /* gone */ }

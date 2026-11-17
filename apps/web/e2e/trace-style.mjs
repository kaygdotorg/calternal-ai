// Which elements are restyled by a key in the composer, and why: a trace with invalidation tracking over the first key and the second. Prints each UpdateLayoutTree (how many elements it
// recalculated) and the invalidation reasons grouped by reason and node (an attribute change on a high ancestor, a :has() or :focus-within selector, a class toggle).
// Usage: node e2e/trace-style.mjs <distDir>   (run from apps/web; port 4400)
import { spawn } from "node:child_process";
import { launch } from "./lib.mjs";
const DIR = process.argv[2] ?? "dist";
const PRE = process.argv[3], KEYS = process.argv[4]; // optional: type PRE first (untraced), then trace KEYS only
const base = "http://127.0.0.1:4400";
const srv = spawn("npx", ["vite", "preview", "--outDir", DIR, "--port", "4400", "--strictPort", "--host", "127.0.0.1"], { stdio: "ignore", detached: true });
for (let i = 0; i < 60; i++) { try { if ((await fetch(base)).ok) break; } catch { /* not yet */ } await new Promise((r) => setTimeout(r, 250)); }
const b = await launch();
const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });
await ctx.addInitScript(() => { try { localStorage.setItem("cal.firstrun", "1"); } catch {} });
const page = await ctx.newPage(); const cdp = await ctx.newCDPSession(page);
await page.goto(base + "/threads/t-idle"); await page.waitForSelector("#composer-field"); await page.waitForTimeout(1500);
await cdp.send("Emulation.setCPUThrottlingRate", { rate: 4 });
await page.locator("#composer-field").click(); await page.waitForTimeout(500);
async function trace(label, keys) {
  await cdp.send("Tracing.start", { categories: "devtools.timeline,disabled-by-default-devtools.timeline,disabled-by-default-devtools.timeline.invalidationTracking", transferMode: "ReturnAsStream" });
  await page.keyboard.type(keys, { delay: 60 }); await page.waitForTimeout(500);
  const done = new Promise((r) => cdp.once("Tracing.tracingComplete", r));
  await cdp.send("Tracing.end"); const { stream } = await done;
  let text = ""; for (;;) { const c = await cdp.send("IO.read", { handle: stream }); text += c.data; if (c.eof) break; }
  const ev = JSON.parse(text).traceEvents ?? JSON.parse(text);
  console.log(`\n=== ${label}`);
  for (const e of ev) if (e.name === "UpdateLayoutTree" && e.ph === "X" && e.dur > 200) console.log(`  UpdateLayoutTree ${(e.dur / 1000).toFixed(2)} ms, ${e.args?.elementCount ?? "?"} elements`);
  for (const e of ev) if (e.name === "Layout" && e.ph === "X" && e.dur > 200) { const bd = e.args?.beginData ?? {}; console.log(`  Layout ${(e.dur / 1000).toFixed(2)} ms, ${bd.dirtyObjects ?? "?"} dirty of ${bd.totalObjects ?? "?"} objects${bd.partialLayout ? " (partial)" : ""}, root ${e.args?.endData?.root?.slice?.(0, 40) ?? ""}`); }
  const lwhy = new Map();
  for (const e of ev) if (e.name === "LayoutInvalidationTracking") { const d = e.args?.data ?? {}; const k = `${d.reason ?? "?"} | ${d.nodeName ?? ""}`; lwhy.set(k, (lwhy.get(k) ?? 0) + 1); }
  console.log("  layout invalidations:\n" + ([...lwhy.entries()].sort((a, c) => c[1] - a[1]).slice(0, 10).map(([k, v]) => `   ${String(v).padStart(4)}  ${k}`).join("\n") || "   (none recorded)"));
  const why = new Map();
  for (const e of ev) if (e.name === "StyleRecalcInvalidationTracking") { const d = e.args?.data ?? {}; const k = `${d.reason ?? "?"} | ${d.nodeName ?? ""}${d.extraData ? " | " + String(d.extraData).slice(0, 60) : ""}`; why.set(k, (why.get(k) ?? 0) + 1); }
  console.log("  invalidations:\n" + [...why.entries()].sort((a, c) => c[1] - a[1]).slice(0, 14).map(([k, v]) => `   ${String(v).padStart(4)}  ${k}`).join("\n"));
}
if (PRE !== undefined) { await page.keyboard.type(PRE, { delay: 60 }); await page.waitForTimeout(400); await trace(`keys "${KEYS}" after "${PRE.slice(-12)}"`, KEYS); }
else { await trace("first key", "t"); await trace("keys 2 to 5", "he q"); }
await b.close(); try { process.kill(-srv.pid); } catch { /* gone */ }

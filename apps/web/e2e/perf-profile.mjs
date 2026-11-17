// Where the time goes: a CPU profile (V8 sampling, 4x slowdown) of the first load and of the palette, summed by function. Prints the top self-time entries.
// Usage: node e2e/perf-profile.mjs [startup|palette|scroll|switch|sheet]   (after `npm run build`).
import { chromium } from "playwright";
import { serve, open, DEVICES, BASE } from "./lib.mjs";
const what = process.argv[2] ?? "startup";
const stop = await serve();
const b = await chromium.launch();
const ctx = await b.newContext({ ...DEVICES.desktop });
await ctx.addInitScript(() => { try { localStorage.setItem("cal.firstrun", "1"); } catch {} });
const page = await ctx.newPage();
const cdp = await ctx.newCDPSession(page);
await cdp.send("Emulation.setCPUThrottlingRate", { rate: 4 });
await cdp.send("Profiler.enable");
await cdp.send("Profiler.setSamplingInterval", { interval: 200 });
const start = () => cdp.send("Profiler.start");
const report = async () => {
  const { profile } = await cdp.send("Profiler.stop");
  const dt = profile.timeDeltas; const self = new Map(); const byId = new Map(profile.nodes.map((n) => [n.id, n]));
  profile.samples.forEach((id, i) => { const n = byId.get(id); const f = n.callFrame; const key = `${f.functionName || "(anon)"} ${f.url.split("/").slice(-2).join("/")}:${f.lineNumber}`; self.set(key, (self.get(key) ?? 0) + (dt[i] ?? 0)); });
  const total = [...self.values()].reduce((a, c) => a + c, 0);
  // Who calls the natives that stand out: the nearest named ancestors of getPropertyValue, remove, before, getComputedStyle.
  const parent = new Map(); for (const n of profile.nodes) for (const c of n.children ?? []) parent.set(c, n.id);
  const callers = new Map();
  profile.samples.forEach((id, i) => { const f = byId.get(id).callFrame; if (!["getPropertyValue", "remove", "before", "getComputedStyle", "elementsFromPoint"].includes(f.functionName)) return; let p = parent.get(id), chain = []; while (p && chain.length < 4) { const pf = byId.get(p).callFrame; if (pf.url) chain.push(`${pf.functionName || "(anon)"}@${pf.url.split("/").pop()}:${pf.lineNumber}:${pf.columnNumber}`); p = parent.get(p); } const k = `${f.functionName} <- ${chain.join(" <- ")}`; callers.set(k, (callers.get(k) ?? 0) + (dt[i] ?? 0)); });
  for (const [k, v] of [...callers.entries()].sort((a, c) => c[1] - a[1]).slice(0, 6)) console.log(`   caller ${(v / 1000).toFixed(1)} ms ${k}`);
  console.log(`== ${what} (4x CPU), ${(total / 1000).toFixed(0)} ms sampled`);
  for (const [k, v] of [...self.entries()].sort((a, c) => c[1] - a[1]).slice(0, 14)) console.log(`${(v / 1000).toFixed(1).padStart(7)} ms  ${k}`);
};
if (what === "startup") {
  await start();
  await page.goto(BASE + "/threads/t-run");
  await page.waitForSelector("html[data-ready]"); await page.waitForSelector("h1"); await page.waitForTimeout(2500);
  await report();
} else {
  await page.goto(BASE + "/threads/" + (what === "scroll" ? "t-long" : what === "typing" ? "t-idle" : "t-run"));
  await page.waitForSelector("html[data-ready]"); await page.waitForSelector("h1"); await page.waitForTimeout(3000);
  await start();
  if (what === "typing") { await page.locator("#composer-field").click(); await page.locator("#composer-field").pressSequentially("the quick brown fox jumps over the lazy dog, then naps @Priya friday 5pm. ", { delay: 60 }); }
  if (what === "palette") { await page.keyboard.press("Control+k"); await page.waitForSelector(".pal-input"); await page.locator(".pal-input").pressSequentially("call plumber friday 5pm about journal", { delay: 30 }); }
  if (what === "scroll") { const box = await page.getByRole("feed").boundingBox(); await page.mouse.move(box.x + 300, box.y + 300); for (let i = 0; i < 80; i++) { await page.mouse.wheel(0, -700); await page.waitForTimeout(16); } }
  if (what === "switch") for (let i = 1; i < 7; i++) { await page.locator("a.row").nth(i).click(); await page.waitForTimeout(250); }
  if (what === "sheet") for (let i = 0; i < 4; i++) { await page.getByRole("button", { name: "Search" }).click(); await page.waitForTimeout(500); await page.keyboard.press("Escape"); await page.waitForTimeout(400); }
  await page.waitForTimeout(500);
  await report();
}
await b.close(); stop();

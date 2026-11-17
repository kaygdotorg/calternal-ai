// What the first paint of a page downloads: scripts, styles, fonts, images, uncompressed and gzipped, from the preview of a build. Usage: node e2e/first-load.mjs <appDir> [path]
import { spawn } from "node:child_process";
import { gzipSync } from "node:zlib";
import { launch } from "./lib.mjs";
const [DIR, PATH = "/threads/t-idle"] = process.argv.slice(2);
const base = "http://127.0.0.1:4194";
const srv = spawn("npx", ["vite", "preview", "--port", "4194", "--strictPort", "--host", "127.0.0.1"], { cwd: DIR, stdio: "ignore", detached: true });
for (let i = 0; i < 60; i++) { try { if ((await fetch(base)).ok) break; } catch { /* not yet */ } await new Promise((r) => setTimeout(r, 250)); }
const b = await launch();
const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });
await ctx.addInitScript(() => { try { localStorage.setItem("cal.firstrun", "1"); } catch {} });
const page = await ctx.newPage();
const got = [];
page.on("response", async (r) => { try { const buf = await r.body(); got.push({ url: r.url().replace(base, ""), type: r.request().resourceType(), raw: buf.length, gz: gzipSync(buf).length }); } catch { /* aborted */ } });
await page.goto(base + PATH); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(2500);
// Scripts that had finished downloading by the first paint (the ones the paint waited for) and the ones fetched after it.
const timing = await page.evaluate(() => { const fcp = performance.getEntriesByName("first-contentful-paint")[0]?.startTime ?? Infinity; return performance.getEntriesByType("resource").filter((r) => r.name.endsWith(".js")).map((r) => ({ name: r.name.split("/").pop(), before: r.responseEnd <= fcp })); });
const by = {};
for (const g of got) { const t = /\.(woff2?)$/.test(g.url) ? "font" : g.type; by[t] ??= { files: 0, raw: 0, gz: 0 }; by[t].files++; by[t].raw += g.raw; by[t].gz += g.gz; }
const kb = (n) => (n / 1024).toFixed(1) + " KB";
console.log(Object.entries(by).map(([t, v]) => `${t.padEnd(10)} ${String(v.files).padStart(2)} files  ${kb(v.raw).padStart(10)} raw  ${kb(v.gz).padStart(10)} gzip`).join("\n"));
console.log(got.filter((g) => g.type === "script" || g.type === "stylesheet").sort((a, c) => c.raw - a.raw).map((g) => `  ${kb(g.raw).padStart(9)} ${kb(g.gz).padStart(9)}  ${g.url}`).join("\n"));
const isBefore = (g) => timing.find((t) => g.url.endsWith(t.name))?.before;
const sumOf = (xs) => xs.reduce((a, g) => a + g.raw, 0);
const js = got.filter((g) => g.type === "script");
console.log(`scripts finished before first paint: ${kb(sumOf(js.filter(isBefore)))} raw; fetched after: ${kb(sumOf(js.filter((g) => !isBefore(g))))} raw`);
await b.close(); try { process.kill(-srv.pid); } catch { /* gone */ }

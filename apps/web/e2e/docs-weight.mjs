// The documentation site's first load: bytes by kind (raw and gzip) and the paints, for the landing page and a long page. Usage: node e2e/docs-weight.mjs [baseUrl]  (default http://127.0.0.1/docs)
import { gzipSync } from "node:zlib";
import { chromium } from "playwright";
const BASE = process.argv[2] ?? "http://127.0.0.1/docs";
const b = await chromium.launch();
for (const [name, path] of [["landing", "/"], ["a doc page", "/user/permissions-modes/"]]) {
  for (const scheme of ["light", "dark"]) {
    const ctx = await b.newContext({ viewport: { width: 1280, height: 800 }, colorScheme: scheme }); const p = await ctx.newPage();
    const got = [];
    p.on("response", async (r) => { try { const buf = await r.body(); got.push({ t: /\.(woff2?)$/.test(r.url()) ? "font" : /\.(jpe?g|png|webp|avif|svg)$/.test(r.url()) ? "image" : r.request().resourceType(), raw: buf.length, gz: gzipSync(buf).length }); } catch {} });
    await p.goto(BASE + path, { waitUntil: "load" }); await p.waitForTimeout(1500);
    const paint = await p.evaluate(() => ({ fcp: Math.round(performance.getEntriesByName("first-contentful-paint")[0]?.startTime ?? -1), lcp: 0 }));
    const by = {}; for (const g of got) { by[g.t] ??= { n: 0, raw: 0, gz: 0 }; by[g.t].n++; by[g.t].raw += g.raw; by[g.t].gz += g.gz; }
    const kb = (n) => (n / 1024).toFixed(1);
    console.log(`${name.padEnd(12)} ${scheme.padEnd(5)} FCP ${String(paint.fcp).padStart(4)} ms  ` + Object.entries(by).map(([t, v]) => `${t} ${v.n}x ${kb(v.raw)}K/${kb(v.gz)}Kgz`).join("  "));
    await ctx.close();
  }
}
await b.close();

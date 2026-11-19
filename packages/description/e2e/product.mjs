// The reading-list view of the PRODUCT route (window-engine lane, docs/reviews/window-engine-2026-11-19.md) drawn by the same renderer, through a proxy that adds the session bearer (PRODUCT_VIEW, PRODUCT_TOKEN_FILE):
// `GET base` describes the view, `POST base` opens it, windows run under X-Calternal-View-Lease and X-Calternal-Cache-Partition. Checks the items list (First/Second/Third article in the host's order, exact count 3, the read
// mark), that the reminders bind's refusal (422 unsupported_query_plan) becomes the error state, and prints every difference from the step 0 host as INFO lines. Usage: node e2e/product.mjs [screenshot.png]
import { createServer } from "node:http";
import { spawn } from "node:child_process";
import { launch, open, serve } from "./lib.mjs";
const shot = process.argv[2];
if (!process.env.PRODUCT_VIEW || !process.env.PRODUCT_TOKEN_FILE) { console.error("Set PRODUCT_VIEW and PRODUCT_TOKEN_FILE."); process.exit(2); }
const PORT = 4391;
const srv = spawn("node", ["demo/server.mjs"], { env: { ...process.env, DEMO_PORT: String(PORT) }, stdio: "ignore" });
for (let i = 0; i < 40; i++) { try { if ((await fetch(`http://127.0.0.1:${PORT}/`)).ok) break; } catch { /* not yet */ } await new Promise((r) => setTimeout(r, 150)); }
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const ctx = await browser.newContext({ viewport: { width: 1200, height: 800 } }); const p = await ctx.newPage();
const seen = []; p.on("response", async (r) => { if (/\/product/.test(r.url())) seen.push({ url: new URL(r.url()).pathname, status: r.status(), method: r.request().method(), headers: r.request().headers(), body: await r.text().catch(() => "") }); });
const errors = []; p.on("pageerror", (e) => errors.push(String(e)));
await p.goto(`http://127.0.0.1:${PORT}/app/?product=/product&mockless=1`);
await p.waitForSelector("[data-i]", { timeout: 20000 }).catch(() => {});
await p.waitForTimeout(2500);
const titles = await p.evaluate(() => [...document.querySelectorAll("[data-node=list] .list[data-bind=items] [data-i] .t")].map((e) => e.textContent));
const marks = await p.evaluate(() => [...document.querySelectorAll("[data-node=list] .list[data-bind=items] [data-i]")].map((e) => !!e.querySelector("[data-mark=icon]")));
const count = await p.evaluate(() => document.querySelector("[data-node=list] .list[data-bind=items]")?.getAttribute("data-count"));
ok(titles.join("|") === "Second article|Third article|First article", "the items list draws the host's order", titles.join("|"));
ok(marks.join() === "true,false,false", "only the read item carries the read mark", marks.join());
ok(count === "3", "exact count 3", String(count));
const states = await p.evaluate(() => [...document.querySelectorAll(".list[data-state]")].map((e) => e.getAttribute("data-bind") + ":" + e.getAttribute("data-state")));
ok(states.includes("reminders:error"), "the reminders bind (refused with 422 unsupported_query_plan) shows its error state", states.join(","));
const win = seen.find((s) => s.url.endsWith("/window") && s.method === "POST");
ok(!!win?.headers["x-calternal-view-lease"] && !!win?.headers["x-calternal-cache-partition"], "windows carry the lease and cache-partition headers");
const open_ = seen.find((s) => s.method === "POST" && s.url === "/product");
const lease = JSON.parse(open_?.body || "{}").lease ?? {};
console.log(`INFO  view.open reply keys: ${Object.keys(JSON.parse(open_?.body || "{}")).join(",")}; lease keys: ${Object.keys(lease).join(",")}`);
const w0 = JSON.parse(win?.body || "{}");
console.log(`INFO  window reply keys: ${Object.keys(w0).join(",")}; audience ${w0.audience}; row keys ${Object.keys(w0.rows?.[0] ?? {}).join(",")}; field names ${Object.keys(w0.rows?.[0]?.fields ?? {}).join(",")}; contexts per row ${w0.activation_contexts?.[0] ? Object.keys(w0.activation_contexts[0].contexts).join(",") : "none"}`);
console.log(`INFO  routes seen: ${[...new Set(seen.map((s) => `${s.method} ${s.url} ${s.status}`))].join("; ")}`);
ok(errors.length === 0, "no script error", errors[0]);
if (shot) await p.screenshot({ path: shot });
await browser.close(); stop(); srv.kill();
process.exit(bad ? 1 : 0);

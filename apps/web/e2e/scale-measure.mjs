// Server mode at scale, against the server seeded by e2e/scale-seed.mjs (10,000 notes, 2,000 tags, 200 members, 100 open invitations, 50 spaces, 30 machines): for each screen the time to usable, the requests
// and bytes it made, the worst and 95th-percentile frame while it scrolls, the JS heap and the DOM size. Development-host numbers (dev-02, debug server, headless Chromium). Env: CAL_SCALE_PORT, CAL_SCALE_DIR, SHOTS=<dir>.
import { readFileSync } from "node:fs";
import { chromium } from "playwright";

const PORT = Number(process.env.CAL_SCALE_PORT ?? 3400), DIR = process.env.CAL_SCALE_DIR ?? "/tmp/cal-scale";
const url = `http://localhost:${PORT}`, state = JSON.parse(readFileSync(`${DIR}/state.json`, "utf8"));
const b = await chromium.launch();
const pct = (a, p) => [...a].sort((x, y) => x - y)[Math.min(a.length - 1, Math.floor(p * (a.length - 1)))] ?? 0;
const rows = [];
async function scenario(name, path, ready, o = {}) {
  const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, storageState: { cookies: state.cookies, origins: [] } });
  await ctx.addInitScript(() => { localStorage.setItem("cal.firstrun", "1"); localStorage.setItem("cal.server", "1"); });
  const p = await ctx.newPage(); const cdp = await ctx.newCDPSession(p); await cdp.send("Performance.enable");
  let n = 0, bytes = 0; p.on("response", async (r) => { if (r.url().includes("/api/v1/") && !r.url().includes("/live")) { n++; try { bytes += (await r.body()).length; } catch {} } });
  const t0 = Date.now();
  await p.goto(`${url}${path}`, { waitUntil: "commit" });
  await p.waitForSelector("html[data-ready]", { timeout: 30000 });
  let usable = "timeout";
  try { await ready(p); usable = Date.now() - t0; } catch { /* reported as a timeout */ }
  await p.waitForTimeout(600);
  let frames = null;
  if (o.scroll && usable !== "timeout") {
    frames = await p.evaluate(async (sel) => { const el = document.querySelector(sel); if (!el) return null; const d = []; let last = performance.now(); let top = el.scrollTop; await new Promise((res) => { let k = 0; const f = (t) => { d.push(t - last); last = t; el.scrollTop = top += 60; if (++k < 90) requestAnimationFrame(f); else res(); }; requestAnimationFrame(f); }); return d.slice(2); }, o.scroll);
  }
  const m = Object.fromEntries((await cdp.send("Performance.getMetrics")).metrics.map((x) => [x.name, x.value]));
  const extra = o.extra ? await o.extra(p) : "";
  if (process.env.SHOTS && o.shot) await p.screenshot({ path: `${process.env.SHOTS}/${o.shot}.png` });
  rows.push({ name, usable, n, kb: Math.round(bytes / 1024), worst: frames ? Math.max(...frames) : null, p95: frames ? pct(frames, 0.95) : null, heap: Math.round(m.JSHeapUsedSize / 1048576), dom: m.Nodes, extra });
  await ctx.close();
}
const count = (sel) => (p) => p.locator(sel).count();
await scenario("Notes: list opens", "/notes", (p) => p.locator('nav[aria-label="Notes"] a.row').first().waitFor({ timeout: 20000 }), { scroll: "aside.side .scroll", extra: async (p) => `${await p.locator('nav[aria-label="Notes"] a.row').count()} rows listed, ${await p.locator(".tg").count()} tag chips` });
await scenario("Notes: tag filter", "/notes", async (p) => { await p.locator(".tg").nth(3).waitFor({ timeout: 20000 }); const t = Date.now(); await p.locator(".tg").nth(3).evaluate((e) => e.click()); await p.waitForFunction((x) => document.querySelectorAll('nav[aria-label="Notes"] a.row').length <= 6, null, { timeout: 8000 }); globalThis.__filterMs = Date.now() - t; }, { extra: async (p) => `filter applied in ${globalThis.__filterMs} ms, ${await p.locator('nav[aria-label="Notes"] a.row').count()} notes under the tag` });
await scenario("Notes: [[ title picker", "/notes", async (p) => { await p.locator('nav[aria-label="Notes"] a.row').first().click(); await p.waitForSelector(".cm-content"); await p.locator(".cm-content").click(); await p.keyboard.press("Control+End"); const t = Date.now(); await p.keyboard.type("\n[[Note 004"); await p.getByRole("option").first().waitFor({ timeout: 8000 }); globalThis.__pickMs = Date.now() - t; }, { extra: async (p) => `${await p.getByRole("option").count()} options, first after ${globalThis.__pickMs} ms from the last key` });
await scenario("Space dots: 50 spaces", "/threads", (p) => p.locator('[aria-label="Spaces"]').waitFor({ timeout: 20000 }), { shot: "dots", extra: async (p) => p.evaluate(() => { const g = document.querySelector('[aria-label="Spaces"]'), s = document.querySelector("aside.side"); const r = g.getBoundingClientRect(), a = s.getBoundingClientRect(); return `${g.children.length} dots; row ${Math.round(r.width)} px wide in a ${Math.round(a.width)} px sidebar; right edge ${Math.round(r.right)} vs ${Math.round(a.right)} (${r.right > a.right + 1 ? "overflows and is clipped" : "fits"})`; }) });
const toEnd = async (p, sel, rowSel, want) => { const t = Date.now(); let n = 0; for (let i = 0; i < 900; i++) { await p.evaluate((s) => { const el = document.querySelector(s); el.scrollTop = el.scrollHeight; }, sel); await p.waitForTimeout(120); n = await p.locator(rowSel).count(); if (n >= want) break; } return `${n} rows after scrolling to the end (${Date.now() - t} ms, pages fetched on the way)`; };
await scenario("Notes: list scrolled to the end (10,000)", "/notes", (p) => p.locator('nav[aria-label="Notes"] a.row').first().waitFor({ timeout: 20000 }), { extra: async (p) => { const first = await p.locator('nav[aria-label="Notes"] a.row').count(); return `${first} rows at first; ${await toEnd(p, "aside.side .scroll", 'nav[aria-label="Notes"] a.row', 10000)}; ${await p.evaluate(() => document.querySelectorAll("*").length)} DOM nodes, heap ${await p.evaluate(() => Math.round(performance.memory.usedJSHeapSize / 1048576))} MB; ${await p.evaluate(async () => { const el = document.querySelector("aside.side .scroll"); const d = []; let last = performance.now(); el.scrollTop = 0; await new Promise((r) => { let i = 0; const f = (t) => { d.push(t - last); last = t; el.scrollTop += el.scrollHeight / 120; if (++i < 120) requestAnimationFrame(f); else r(); }; requestAnimationFrame(f); }); d.sort((x, y) => x - y); return `scrolling the whole list: worst frame ${Math.round(d.at(-1))} ms, p95 ${Math.round(d[Math.floor(d.length * 0.95)])} ms`; })}` } });
await scenario("Trash: scrolled to the end (1,500)", "/notes?view=trash", (p) => p.getByRole("heading", { name: "Trash" }).waitFor({ timeout: 20000 }), { extra: async (p) => { const rows = ".nc-article .r"; const first = await p.locator(rows).count(); const sel = await p.evaluate(() => { let e = document.querySelector(".nc-article .r"); while (e && e.scrollHeight <= e.clientHeight + 1) e = e.parentElement; e?.setAttribute("data-end", ""); return "[data-end]"; }); return `${first} rows at first; ${await toEnd(p, sel, rows, 1500)}`; } });
await scenario("Members: 200 people", "/settings/members", (p) => p.locator('[data-setting^="space.member."]').nth(40).waitFor({ timeout: 30000 }), { scroll: ".scroller", shot: "members", extra: async (p) => { const first = await p.locator('[data-setting^="space.member."]').count(); return `${first} rows first; ${await toEnd(p, ".scroller", '[data-setting^="space.member."]', 200)}`; } });
await scenario("Invites: 100 open", "/settings/invites", (p) => p.locator('[data-setting^="space.invite."]').nth(40).waitFor({ timeout: 30000 }), { scroll: ".scroller", extra: async (p) => `${await p.locator('[data-setting^="space.invite."]').count()} rows first; ${await toEnd(p, ".scroller", '[data-setting^="space.invite."]', 100)} (open ones first)` });
await scenario("Machines: 30", "/settings/machines", (p) => p.locator('[data-setting^="machines."]').nth(25).waitFor({ timeout: 30000 }), { scroll: ".scroller", extra: async (p) => `${await p.locator('[data-setting^="machines."]').count()} rows` });
await scenario("Admin: people (all accounts)", "/settings/people", (p) => p.locator('[data-setting^="server.person."]').nth(40).waitFor({ timeout: 30000 }), { scroll: ".scroller", extra: async (p) => `${await p.locator('[data-setting^="server.person."]').count()} rows first; ${await toEnd(p, ".scroller", '[data-setting^="server.person."]', 200)}` });
await b.close();
const w = (s, n) => String(s).padEnd(n);
console.log(w("screen", 30) + w("usable", 9) + w("requests", 10) + w("KB", 8) + w("worst", 8) + w("p95", 7) + w("heap MB", 9) + w("DOM", 8) + "notes");
for (const r of rows) console.log(w(r.name, 30) + w(typeof r.usable === "number" ? r.usable + " ms" : r.usable, 9) + w(r.n, 10) + w(r.kb, 8) + w(r.worst === null ? "-" : r.worst.toFixed(0) + " ms", 8) + w(r.p95 === null ? "-" : r.p95.toFixed(0), 7) + w(r.heap, 9) + w(r.dom, 8) + r.extra);

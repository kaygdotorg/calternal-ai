// A note removed on another device leaves a list that already holds it, even scrolled past the first page: one session has the Notes list open and scrolled several pages down; another session
// (same account) moves one of the rows it shows to the Trash; the first list drops that row without a reload. Runs against the server seeded by e2e/scale-seed.mjs. Env: CAL_SCALE_PORT, CAL_SCALE_DIR.
import { readFileSync } from "node:fs";
import { chromium } from "playwright";
const PORT = Number(process.env.CAL_SCALE_PORT ?? 3400), DIR = process.env.CAL_SCALE_DIR ?? "/tmp/cal-scale";
const url = `http://localhost:${PORT}`, state = JSON.parse(readFileSync(`${DIR}/state.json`, "utf8"));
const b = await chromium.launch(); let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, storageState: { cookies: state.cookies, origins: [] } });
const page = await ctx.newPage();
await page.goto(`${url}/notes?server=1`); await page.waitForSelector("html[data-ready]");
await page.locator('nav[aria-label="Notes"] a.row').first().waitFor({ timeout: 20000 });
// Scroll down until at least four pages (400 rows) are loaded, then park on a row far from the top.
for (let i = 0; i < 60; i++) { await page.evaluate(() => { const e = document.querySelector("aside.side .scroll"); e.scrollTop = e.scrollHeight; }); await page.waitForTimeout(150); if (await page.evaluate(() => Math.max(0, ...[...document.querySelectorAll("[aria-setsize]")].map((e) => +e.getAttribute("aria-setsize"))) >= 400)) break; }
await page.evaluate(() => { const e = document.querySelector("aside.side .scroll"); e.scrollTop = e.scrollHeight * 0.03; });
await page.waitForTimeout(500);
const victim = await page.evaluate(() => { const a = [...document.querySelectorAll('nav[aria-label="Notes"] a.row')]; const r = a[Math.floor(a.length / 2)]; return { href: r.getAttribute("href"), title: r.textContent.trim().slice(0, 40) }; });
const id = victim.href.split("/").pop();
const rows = () => page.locator(`nav[aria-label="Notes"] a.row[href$="/${id}"]`).count();
ok((await rows()) === 1, "the row is on screen, scrolled well past the first page", victim.title);
// Another session of the same account trashes it.
const res = await ctx.request.post(`${url}/api/v1/operations`, { data: { request_id: crypto.randomUUID(), operation: "notes.delete", input: { id } } });
ok((await res.json()).ok === true, "another session moved the note to the Trash");
let gone = false; for (let i = 0; i < 40 && !gone; i++) { await page.waitForTimeout(250); gone = (await rows()) === 0; }
ok(gone, "the list drops the row without a reload");
await ctx.request.post(`${url}/api/v1/operations`, { data: { request_id: crypto.randomUUID(), operation: "notes.restore", input: { id } } });
await b.close(); process.exit(bad ? 1 : 0);

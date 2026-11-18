// One spacing rule for a section heading (packages/ui base.css `.section-heading`, tokens section-above, section-below, section-h): the distance from the last item above to the heading's text line, and from its text line to the first
// item below, is the same in every sidebar and list page that has headings: Settings (User, Admin, Space), Threads (a section), Notes (Waiting to send). Measured on the rendered boxes at desktop size; SHOTS=<dir> saves 2x
// pictures of each to compare side by side.
import { chromium } from "playwright";
import { mkdirSync } from "node:fs";
import { BASE, serve } from "./lib.mjs";
const stop = await serve(); const b = await chromium.launch(); let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const dir = process.env.SHOTS; if (dir) mkdirSync(dir, { recursive: true });
const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 }); await ctx.addInitScript(() => localStorage.setItem("cal.firstrun", "1"));
const page = await ctx.newPage();
// For a heading element: the gap above (previous sibling's bottom to the heading text line's top) and below (the text line's bottom to the next sibling's top), in px; `item` picks the neighbour boxes.
const measure = (sel) => page.evaluate((sel) => {
  const out = [];
  for (const h of document.querySelectorAll(sel)) {
    const r = document.createRange(); r.selectNodeContents(h.querySelector(".nm") ?? h); const t = r.getBoundingClientRect();
    const prev = h.previousElementSibling ?? h.parentElement.previousElementSibling, next = h.nextElementSibling, first = h.classList.contains("first") || (h.parentElement.matches(".grp") && !h.parentElement.previousElementSibling);
    const bottom = (e) => { let m = 0; const q = e.querySelectorAll("a.row, .row, li"); const els = q.length ? [...q] : [e]; for (const x of els) m = Math.max(m, x.getBoundingClientRect().bottom); return m; };
    const top = (e) => { const f = e.querySelector("a.row, .row, li") ?? e; return f.getBoundingClientRect().top; };
    out.push({ text: h.textContent.trim().slice(0, 20), above: prev && !first ? Math.round((t.top - bottom(prev)) * 10) / 10 : null, below: next ? Math.round((top(next) - t.bottom) * 10) / 10 : null });
  }
  return out;
}, sel);
await page.goto(`${BASE}/settings/appearance`); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(700);
const settings = await measure("nav[aria-label=Settings] .section-heading");
if (dir) await page.locator("aside.side").screenshot({ path: `${dir}/settings-sidebar.png` });
await page.goto(`${BASE}/threads/t-run`); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(700);
const threads = await measure(".sec > .sh");
if (dir) await page.locator("aside.side").screenshot({ path: `${dir}/threads-sidebar.png` });
await page.goto(`${BASE}/notes/n-1`); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(700);
const notes = await measure("aside.side .section-heading");
if (dir) await page.locator("aside.side").screenshot({ path: `${dir}/notes-sidebar.png` });
console.log("settings", JSON.stringify(settings)); console.log("threads", JSON.stringify(threads)); console.log("notes", JSON.stringify(notes));
const belows = [...settings, ...threads, ...notes].map((x) => x.below).filter((v) => v !== null), aboves = [...settings, ...threads, ...notes].map((x) => x.above).filter((v) => v !== null);
const spread = (xs) => (xs.length ? Math.max(...xs) - Math.min(...xs) : 0);
ok(belows.length >= 4 && spread(belows) <= 2.5, "heading text to first item: the same distance everywhere", `${Math.min(...belows)} to ${Math.max(...belows)} px over ${belows.length} headings`);
ok(aboves.length >= 2 && spread(aboves) <= 2.5, "last item to heading text: the same distance everywhere", `${Math.min(...aboves)} to ${Math.max(...aboves)} px over ${aboves.length} headings`);
await b.close(); stop(); process.exit(bad ? 1 : 0);

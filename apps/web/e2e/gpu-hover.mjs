// Headed GPU Chrome (CDP) at 1500x970 dark: a real pointer move over thread rows (Input.dispatchMouseEvent through the page's mouse), the hover actions against the title, the child-threads chip,
// the model picker rows and the scrollbar of the conversation. Usage: BASE=http://127.0.0.1:4394 node e2e/gpu-hover.mjs <outDir> [label]
import { chromium } from "playwright-core";
import { mkdirSync } from "node:fs";
const BASE = process.env.BASE, OUT = process.argv[2], L = process.argv[3] ?? "after"; mkdirSync(OUT, { recursive: true });
const b = await chromium.connectOverCDP("http://127.0.0.1:9222"); const ctx = b.contexts()[0] ?? await b.newContext();
const page = await ctx.newPage(); await page.bringToFront(); const cdp = await ctx.newCDPSession(page);
await cdp.send("Emulation.setDeviceMetricsOverride", { width: 1500, height: 970, deviceScaleFactor: 2, mobile: false });
await page.emulateMedia({ colorScheme: "dark" });
await page.goto(BASE + "/threads/t-idle?firstrun=0"); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(2500);
// 1. hover over the longest title
const rows = page.locator("nav[aria-label=Threads] a.row"); const n = await rows.count();
let best = 0, bi = 0; for (let i = 0; i < n; i++) { const w = await rows.nth(i).locator(".t").evaluate((e) => e.scrollWidth); if (w > best) { best = w; bi = i; } }
const box = await rows.nth(bi).boundingBox();
await cdp.send("Emulation.setFocusEmulationEnabled", { enabled: true });
await page.mouse.move(box.x + 40, box.y + box.height / 2); for (let i = 0; i < 6; i++) { await page.mouse.move(box.x + 60 + i * 4, box.y + box.height / 2); await page.waitForTimeout(120); } await page.waitForTimeout(600); await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)))); await page.waitForTimeout(1800);
const m = await rows.nth(bi).evaluate((a) => { const t = a.querySelector(".t"), acts = a.closest(".rw").querySelector(".acts"); const tr = t.getBoundingClientRect(), ar = acts.getBoundingClientRect(); return { titleRight: Math.round(tr.right), actsLeft: Math.round(ar.left), actsOpacity: getComputedStyle(acts).opacity, overlap: Math.round(tr.right - ar.left) }; });
if (process.env.HOLD) { console.log("HOLDING"); await page.waitForTimeout(Number(process.env.HOLD)); }
console.log("   row text:", await rows.nth(bi).innerText(), "| box", JSON.stringify(box), "| under pointer:", await page.evaluate(([x, y]) => { const e = document.elementFromPoint(x, y); return (e?.closest("a.row")?.textContent ?? e?.tagName) + " hover=" + (e?.closest(".rw")?.matches(":hover") ?? "n/a"); }, [box.x + 80, box.y + box.height / 2]));
console.log(`${L}: hover row ${bi} (title ${best}px wide):`, JSON.stringify(m));
{ let last = null; cdp.on("Page.screencastFrame", (f) => { last = f.data; void cdp.send("Page.screencastFrameAck", { sessionId: f.sessionId }); });
  await cdp.send("Page.startScreencast", { format: "png", maxWidth: 3000, maxHeight: 2000, everyNthFrame: 1 });
  for (let i = 0; i < 6; i++) { await page.mouse.move(box.x + 64 + (i % 2), box.y + box.height / 2); await page.waitForTimeout(250); }
  await page.waitForTimeout(800); await cdp.send("Page.stopScreencast");
  if (last) (await import("node:fs")).writeFileSync(`${OUT}/${L}-screencast.png`, Buffer.from(last, "base64")); }
await page.screenshot({ path: `${OUT}/${L}-hover-warm.png`, clip: { x: 0, y: 0, width: 440, height: 520 } });
await page.mouse.move(box.x + 64, box.y + box.height / 2 + 1); await page.waitForTimeout(800);
await page.screenshot({ path: `${OUT}/${L}-hover.png`, clip: { x: 0, y: 330, width: 330, height: 100 }, scale: "device" });
// 2. the conversation scrollbar and the whole page
await page.mouse.move(900, 400); await page.waitForTimeout(300);
console.log(`${L}: conversation scrollbar-width:`, await page.locator("[role=feed]").evaluate((e) => getComputedStyle(e).scrollbarWidth + " | clientWidth " + e.clientWidth + " offsetWidth " + e.offsetWidth));
await page.screenshot({ path: `${OUT}/${L}-page.png` });
// 3. a frame from the compositor (the screenshot call can return a stale frame in this VM): the screencast, after a nudge of the real pointer.
const snap = async (name, x = 700, y = 300) => { let last = null; const h = (f) => { last = f.data; void cdp.send("Page.screencastFrameAck", { sessionId: f.sessionId }); }; cdp.on("Page.screencastFrame", h);
  await cdp.send("Page.startScreencast", { format: "png", maxWidth: 3000, maxHeight: 2000, everyNthFrame: 1 });
  for (let i = 0; i < 4; i++) { await page.mouse.move(x + (i % 2), y); await page.waitForTimeout(250); } await page.waitForTimeout(900); await cdp.send("Page.stopScreencast"); cdp.off("Page.screencastFrame", h);
  if (last) (await import("node:fs")).writeFileSync(`${OUT}/${L}-${name}.png`, Buffer.from(last, "base64")); };
// the child-threads chip: a thread whose children are running
await rows.first().click(); await page.waitForTimeout(1200); await snap("children", 700, 500);
// the model picker
await page.getByRole("button", { name: /GPT|Claude|Codex/ }).first().click().catch(() => {}); await page.waitForTimeout(900); await snap("model", 400, 300);
await page.keyboard.press("Escape");
// the approval on a phone (touch emulation, so the details fold)
await cdp.send("Emulation.setDeviceMetricsOverride", { width: 390, height: 844, deviceScaleFactor: 3, mobile: true });
await cdp.send("Emulation.setTouchEmulationEnabled", { enabled: true });
await page.goto(BASE + "/threads/t-wait?firstrun=0"); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(2200); await page.getByRole("button", { name: "Next request" }).tap().catch(() => page.getByRole("button", { name: "Next request" }).click()); await page.waitForTimeout(600); await snap("phone-approval", 200, 300);
await page.close(); await b.close().catch(() => {});

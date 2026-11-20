// Windowing on a thread of very uneven message heights (`?fixture=threads-next&scenario=uneven&messages=1500`: one-line replies beside 40-line code blocks and long paragraphs): the DOM holds a window, a jump to the end
// settles from anywhere, the person's place survives older pages being added above and survives the heights learned while walking, and the keyboard reaches every message in order. Usage: node e2e/threads-window.mjs
import { launch, open, serve } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const N = 1500;
const { page: p } = await open(browser, `fixture=threads-next&scenario=uneven&messages=${N}`, { width: 1100, height: 700 });
const errors = []; p.on("pageerror", (e) => errors.push(String(e)));
await p.waitForSelector("[data-i] button.row", { timeout: 30000 });
await p.locator("[data-i] button.row").first().evaluate((e) => e.click());
await p.waitForSelector("[data-entry]", { timeout: 30000 });
const log = p.locator("[role=log]");
const atEnd = () => log.evaluate((e) => e.scrollHeight - e.clientHeight - e.scrollTop < 2);
const opens = () => p.evaluate(() => window.__transport.log.filter((l) => l.route === "item_view.open").length);
const rectTop = (a) => p.evaluate((x) => { const e = document.querySelector(`[data-anchor="${x}"]`), l = document.querySelector("[role=log]"); return e ? e.getBoundingClientRect().top - l.getBoundingClientRect().top : null; }, a);
await p.waitForTimeout(500);
ok(await atEnd() && (await p.locator("[data-anchor=m1499]").count()) === 1, "the thread opens at its end, on the newest message");

// place kept while an older page is added above: the message at the top of the view must not move when the page arrives (the fixture's older pages take 300 ms)
await log.evaluate((e) => { e.scrollTop = 0; }); await p.waitForTimeout(120);
const topAnchor = await p.evaluate(() => { const l = document.querySelector("[role=log]").getBoundingClientRect(); const e = [...document.querySelectorAll("[data-entry]")].find((x) => x.getBoundingClientRect().bottom > l.top + 4); return e?.getAttribute("data-anchor"); });
const top0 = await rectTop(topAnchor), o0 = await opens();
await p.waitForFunction((k) => window.__transport.log.filter((l) => l.route === "item_view.open").length > k, o0 - 0, { timeout: 15000 }).catch(() => {});
await p.waitForTimeout(700); const top1 = await rectTop(topAnchor);
ok(top0 !== null && top1 !== null && Math.abs(top1 - top0) < 3, "when an older page is added above, the message at the top of the view does not move", `${topAnchor}: ${top0?.toFixed(1)} then ${top1?.toFixed(1)} px`);

// read everything older, then check the uneven heights and the window
for (let i = 0; i < 120 && (await p.locator("[data-older]").count()) > 0; i++) { const k = await opens(); await log.evaluate((e) => { e.scrollTop = 0; }); await p.waitForFunction((n) => window.__transport.log.filter((l) => l.route === "item_view.open").length > n, k, { timeout: 5000 }).catch(() => {}); await p.waitForTimeout(40); }
await p.waitForTimeout(600);
const drawn = await p.locator("[data-entry]").count();
ok(drawn < 150, "the DOM holds a window of the 1,500 messages", `${drawn} drawn`);
const hs = await p.evaluate(() => [...document.querySelectorAll("[data-vi]")].map((e) => e.offsetHeight));
ok(Math.max(...hs) / Math.min(...hs) > 8, "the heights really are uneven", `${Math.min(...hs)} to ${Math.max(...hs)} px`);

// jump to the end from several places
for (const at of [0, 0.25, 0.5, 0.8]) {
  await log.evaluate((e, f) => { e.scrollTop = (e.scrollHeight - e.clientHeight) * f; }, at); await p.waitForTimeout(250);
  const t = Date.now(); await p.locator("[data-jump-latest]").click().catch(async () => { await log.focus(); await p.keyboard.press("End"); });
  await p.waitForFunction(() => { const e = document.querySelector("[role=log]"); return e.scrollHeight - e.clientHeight - e.scrollTop < 2; }, null, { timeout: 20000 }).catch(() => {});
  await p.waitForTimeout(300);
  ok(await atEnd() && (await p.locator("[data-anchor=m1499]").count()) === 1, `a jump to the end from ${Math.round(at * 100)}% of the thread settles on the newest message`, `${Date.now() - t - 300} ms`);
}

// jump to the start of the thread
await log.focus(); await p.keyboard.press("Home"); await p.waitForTimeout(800);
ok((await p.locator("[data-anchor=m0]").count()) === 1 && await log.evaluate((e) => e.scrollTop < 40), "Home goes to the very first message", await log.evaluate((e) => String(Math.round(e.scrollTop))));

// place kept while heights are learned: from the middle, 20 wheel steps down and 20 up bring the same message back to the same place
await log.evaluate((e) => { e.scrollTop = (e.scrollHeight - e.clientHeight) / 2; }); await p.waitForTimeout(500);
const anchorAtTop = () => p.evaluate(() => { const l = document.querySelector("[role=log]").getBoundingClientRect(); const e = [...document.querySelectorAll("[data-entry]")].find((x) => x.getBoundingClientRect().bottom > l.top + 4); return e?.getAttribute("data-anchor"); });
const ref = await anchorAtTop(), refTop = await rectTop(ref);
await log.hover();
for (let i = 0; i < 20; i++) { await p.mouse.wheel(0, 150); await p.waitForTimeout(25); }
await p.waitForTimeout(300);
for (let i = 0; i < 20; i++) { await p.mouse.wheel(0, -150); await p.waitForTimeout(25); }
await p.waitForTimeout(500);
const refAfter = await rectTop(ref);
ok(refAfter !== null && Math.abs(refAfter - refTop) < 4, "walking down and back, the same message returns to the same place (heights learned on the way do not shift the view)", `${ref}: ${refTop?.toFixed(1)} then ${refAfter?.toFixed(1)} px`);

// the keyboard visits every message in order, each in view
await p.locator(`[data-anchor="${ref}"]`).evaluate((e) => e.focus({ preventScroll: true })); await p.waitForTimeout(100);
const sequence = [ref]; let hidden = 0;
for (let i = 0; i < 30; i++) {
  await p.keyboard.press("ArrowDown"); await p.waitForTimeout(40);
  const s1 = await p.evaluate(() => { const e = document.activeElement, l = document.querySelector("[role=log]").getBoundingClientRect(), r = e.getBoundingClientRect(); return { a: e.getAttribute("data-anchor"), v: r.bottom > l.top && r.top < l.bottom }; });
  sequence.push(s1.a); if (!s1.v) hidden++;
}
const idx = (a) => Number(String(a).slice(1));
ok(sequence.every((a, i) => i === 0 || idx(a) === idx(sequence[i - 1]) + 1), "the keyboard visits the messages one by one in order, through every height", sequence.slice(0, 4).join(",") + "…");
ok(hidden === 0, "and the focused message is in view at every step", `${hidden} steps out of view`);
for (let i = 0; i < 30; i++) { await p.keyboard.press("ArrowUp"); await p.waitForTimeout(40); }
ok((await p.evaluate(() => document.activeElement?.getAttribute("data-anchor"))) === ref, "30 down and 30 up return to the same message");
ok(errors.length === 0, "no script error", errors[0]);
await browser.close(); stop();
process.exit(bad ? 1 : 0);

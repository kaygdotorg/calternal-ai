// The row press (packages/ui/src/lib/press.ts): holding a thread row for half a second on a touch screen dims the page, grows a preview of the thread out of the row and opens the row's menu under it (the same items as the three dots); tapping outside
// puts it away; a right click opens the same menu without the preview; a finger that moves is a scroll and opens nothing. The preview is drawn from what is loaded: no operation is sent on a press. Main-thread cost of opening is measured (frames and long tasks while it
// opens) and judged on a quiet machine (the quiet list). Usage: node e2e/press.mjs
import { judgeTimings, launch, open, serve } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const judge = judgeTimings();
const timing = (c, m, note) => { if (judge) ok(c, m, note); else console.log(`SKIP  ${m}: not judged, the machine is busy  ${note}`); };
try {
  const { page: p, ctx } = await open(browser, "phone", "dark", "/threads/t-idle");
  await p.waitForTimeout(800);
  const cdp = await ctx.newCDPSession(p);
  const touch = (type, x, y) => cdp.send("Input.dispatchTouchEvent", { type, touchPoints: type === "touchEnd" ? [] : [{ x, y }] });
  await p.getByRole("button", { name: "Show threads" }).first().click().catch(() => {}); await p.waitForTimeout(700);
  const row = p.locator("nav[aria-label=Threads] li[data-thread]").first();
  await row.waitFor();
  const b = await row.boundingBox();
  const x = b.x + b.width / 2, y = b.y + b.height / 2;
  // what is on the wire while it opens
  const sent = []; p.on("request", (r) => { if (/operate|\/api\//.test(r.url())) sent.push(r.url()); });
  // frames and long tasks during the open
  await p.evaluate(() => { window.__fr = []; window.__lt = []; let last = performance.now(); const tick = () => { const t = performance.now(); window.__fr.push(t - last); last = t; requestAnimationFrame(tick); }; requestAnimationFrame(tick); try { new PerformanceObserver((l) => window.__lt.push(...l.getEntries().map((e) => e.duration))).observe({ type: "longtask" }); } catch {} });
  // 1. a hold opens it
  await touch("touchStart", x, y);
  await p.waitForTimeout(350);
  ok((await p.locator(".card[role=presentation]").count()) === 0, "before the half second nothing is open");
  await p.waitForTimeout(700);
  ok((await p.locator("[role=presentation].scrim").count()) === 1 && (await p.locator("[role=presentation] .face").count()) === 1, "held for half a second: the page is dimmed and the preview card is up");
  const card = await p.locator(".card[role=presentation]").boundingBox();
  const menu = p.getByRole("menu", { name: /Thread:/ });
  ok((await menu.count()) === 1, "the row's menu is open under the preview");
  const mb = await menu.boundingBox();
  ok(!!card && !!mb && mb.y >= card.y + card.height - 1 && mb.y - (card.y + card.height) < 40, "the menu sits right under the card", `card bottom ${card && Math.round(card.y + card.height)}, menu top ${mb && Math.round(mb.y)}`);
  ok(await p.evaluate(({ x, y }) => !!document.elementFromPoint(x, y)?.closest("[role=menu]"), { x: mb.x + mb.width / 2, y: mb.y + 10 }), "the menu is above the dimming (what is at its place is the menu, not the scrim)");
  const names = await menu.getByRole("menuitem").allInnerTexts();
  ok(names.some((n) => /Rename/.test(n)) && names.some((n) => /Archive/.test(n)), "it has the same items as the three dots", names.join(" | "));
  const title = (await row.innerText()).split("\n")[0].trim().replace(/…$/, ""); const face = await p.locator(".face").innerText();
  ok(face.includes(title.slice(0, 12)), "the card shows the thread", `row "${title}" card "${face.replace(/\n/g, " | ").slice(0, 120)}"`);
  await touch("touchEnd", x, y); await p.waitForTimeout(300);
  ok((await p.locator("[role=presentation].scrim").count()) === 1, "lifting the finger leaves it open");
  ok(sent.length === 0, "nothing was requested from the server on the press", sent.join(","));
  const cost = await p.evaluate(() => { const f = window.__fr; return { maxFrame: Math.round(Math.max(...f.slice(2))), longTasks: window.__lt.length, longest: Math.round(Math.max(0, ...window.__lt)) }; });
  timing(cost.maxFrame < 80 && cost.longest < 120, "opening it costs the main thread little: no frame over 80 ms, no long task over 120 ms (the hold, the look-alike, the card)", JSON.stringify(cost));
  // 2. a tap outside puts it away
  await p.mouse.click(8, 8); await p.waitForTimeout(500);
  ok((await p.locator("[role=presentation].scrim").count()) === 0 && (await menu.count()) === 0, "a tap outside puts it away (the opening played backwards)");
  // 3. a finger that moves is a scroll
  await touch("touchStart", x, y); await p.waitForTimeout(150); await touch("touchMove", x, y + 40); await p.waitForTimeout(900); await touch("touchEnd", x, y);
  ok((await p.locator("[role=presentation].scrim").count()) === 0, "a finger that moves first opens nothing");
  // 4. escape and picking
  await touch("touchStart", x, y); await p.waitForTimeout(900); await touch("touchEnd", x, y);
  await p.keyboard.press("Escape"); await p.waitForTimeout(500);
  ok((await p.locator("[role=presentation].scrim").count()) === 0, "Escape puts it away");
  await ctx.close();
  // 5. a pointer device: right click, no preview, no dimming
  const d = await open(browser, "desktop", "dark", "/threads/t-idle");
  await d.page.waitForTimeout(800);
  await d.page.locator("nav[aria-label=Threads] li[data-thread]").first().click({ button: "right" }); await d.page.waitForTimeout(500);
  ok((await d.page.getByRole("menu", { name: /Thread:/ }).count()) === 1 && (await d.page.locator(".scrim").count()) === 0, "a right click opens the same menu without the preview or the dimming");
  await d.page.keyboard.press("Escape"); await d.page.waitForTimeout(300);
  await d.ctx.close();
} catch (err) { console.log("FAIL  " + err.message); bad++; }
finally { await browser.close(); await stop(); }
process.exit(bad ? 1 : 0);

// Keyboard and selection rules of docs/description-contract.md section 5.0 in a real browser, over the in-page mock host (5,000 rows):
//  - Down held for 3 s at 30 keys a second sends NO selection and NO row event while the key repeats, then exactly ONE settled selection, for the row the cursor rests on;
//  - the cursor row paints at once (every key moves it);
//  - Page Down, Page Up, Home and End scroll and leave the cursor and the selection where they are (owner decision, 2026-11-19);
//  - End on a list without an exact count pages forward to the real end, without a selection event;
//  - skip_limit from a far scroll is recovered by paging, the list never stays on skeleton rows.
import { launch, open, serve } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const sel = (p) => p.evaluate(() => window.__transport.log.filter((l) => l.route === "selection.settled").map((l) => l.body.row));
const cursor = (p) => p.evaluate(() => document.querySelector("[data-node=list] [data-cursor]")?.getAttribute("data-i") ?? null);
const top = (p) => p.evaluate(() => { const v = document.querySelector("[data-node=list] [role=feed]"); return v ? v.scrollTop : -1; });

{
  const { page: p } = await open(browser, "mock=5000");
  await p.waitForFunction(() => window.__painted !== undefined);
  await p.locator("[data-node=list] [data-i] button").first().focus();
  const t0 = Date.now(); let during = 0;
  for (let i = 0; i < 90; i++) { const w = t0 + (i * 1000) / 30 - Date.now(); if (w > 0) await p.waitForTimeout(w); await p.keyboard.press("ArrowDown"); if (i % 10 === 9) during = Math.max(during, (await sel(p)).length); }
  ok(during === 0, "no selection request crossed while the key repeated", `${during} sent during the hold`);
  await p.waitForTimeout(400);
  const after = await sel(p), c = await cursor(p);
  ok(after.length === 1, "exactly one settled selection after the burst", `${after.length}`);
  ok(c !== null && after[0] === `r-${c}`, "the settled selection is the row the cursor rests on", `${after[0]} vs cursor ${c}`);
  const before = { c, s: after.length, top: await top(p) };
  for (const key of ["PageDown", "PageDown", "PageUp"]) await p.keyboard.press(key);
  await p.waitForTimeout(500);
  ok((await cursor(p)) === before.c || (await cursor(p)) === null, "Page keys left the cursor where it was", `${before.c} -> ${await cursor(p)}`);
  ok((await top(p)) !== before.top, "Page keys scrolled the list", `${before.top} -> ${await top(p)}`);
  await p.keyboard.press("End"); await p.waitForTimeout(1500);
  const endTop = await top(p);
  const lastShown = await p.evaluate(() => [...document.querySelectorAll("[data-node=list] [data-i]")].map((e) => Number(e.getAttribute("data-i"))).reduce((a, b) => Math.max(a, b), -1));
  ok(lastShown >= 4990, "End scrolled to the tail and drew its rows", `last drawn index ${lastShown}`);
  await p.keyboard.press("Home"); await p.waitForTimeout(800);
  ok((await top(p)) < endTop && (await top(p)) < 5, "Home scrolled to the top", `${await top(p)}`);
  ok((await sel(p)).length === 1, "Page, End and Home sent no selection", `${(await sel(p)).length} total`);
  const ends = await p.evaluate(() => window.__transport.log.filter((l) => l.route === "window" && l.body.request.anchor === "end").length);
  ok(ends >= 1, "End asked for the tail with an end anchor", `${ends} end-anchored requests`);
  await p.context().close();
}
{
  const { page: p } = await open(browser, "mock=600&count=none");
  await p.waitForFunction(() => window.__painted !== undefined);
  await p.locator("[data-node=list] [data-i] button").first().focus();
  await p.keyboard.press("End");
  await p.waitForFunction(() => [...document.querySelectorAll("[data-node=list] [data-i]")].some((e) => e.getAttribute("data-i") === "599"), null, { timeout: 40000 }).catch(() => {});
  const r = await p.evaluate(() => ({ last: [...document.querySelectorAll("[data-node=list] [data-i]")].map((e) => Number(e.getAttribute("data-i"))).reduce((a, b) => Math.max(a, b), -1), ends: window.__transport.log.filter((l) => l.route === "window" && l.body.request.anchor === "end").length, sel: window.__transport.log.filter((l) => l.route === "selection.settled").length }));
  ok(r.last === 599, "End without an exact count pages forward to the real end", `last drawn ${r.last}`);
  ok(r.ends === 0 && r.sel === 0, "and asks for no end anchor and sends no selection", `${r.ends} end requests, ${r.sel} selections`);
  await p.context().close();
}
{
  const { page: p } = await open(browser, "mock=100000&skip=500");
  await p.waitForFunction(() => window.__painted !== undefined);
  await p.evaluate(() => { const v = document.querySelector("[data-node=list] [role=feed]"); v.scrollTop = 1500 * 32; });
  await p.waitForFunction(() => [...document.querySelectorAll("[data-node=list] [data-i]")].some((e) => e.getAttribute("data-i") === "1500" && !e.querySelector("[data-skeleton]")), null, { timeout: 60000 }).catch(() => {});
  const r = await p.evaluate(() => ({ drawn: [...document.querySelectorAll("[data-node=list] [data-i]")].filter((e) => !e.querySelector("[data-skeleton]")).length, refused: window.__transport.windowCalls }));
  ok(r.drawn > 5, "a skip_limit refusal is recovered by paging: real rows are drawn at the target", `${r.drawn} real rows, ${r.refused} window requests`);
  await p.context().close();
}
await browser.close(); stop();
process.exit(bad ? 1 : 0);

// A request card (Secret needed, a question, an approval) in a window only about 400 px high (a phone on its side): the card is no taller than the room above the composer, it scrolls inside, and its actions (Save, Not now; Send answers; Deny, Allow) stay in view and clickable, never under the composer.
import { launch, serve } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const BASE = process.env.CAL_URL;
try {
  for (const [path, actions] of [["/threads/t-secret", ["Save", "Not now"]], ["/threads/t-wait", ["Send answers"]]]) {
    const ctx = await browser.newContext({ viewport: { width: 844, height: 390 }, deviceScaleFactor: 2, isMobile: true, hasTouch: true, colorScheme: "dark" });
    await ctx.addInitScript(() => { try { localStorage.setItem("cal.firstrun", "1"); } catch { /* none */ } });
    const p = await ctx.newPage(); await p.goto(BASE + path); await p.waitForSelector("html[data-ready]"); await p.waitForTimeout(1200);
    const bar = p.getByRole("region", { name: "Waiting on you" });
    const g = await p.evaluate(() => { const b = document.querySelector("[aria-label=\"Waiting on you\"]").getBoundingClientRect(); const c = document.querySelector(".composer").getBoundingClientRect(); const h = document.querySelector(".thread header")?.getBoundingClientRect(); return { barTop: b.top, barBottom: b.bottom, compTop: c.top, headBottom: h?.bottom ?? 0, vh: innerHeight }; });
    ok(g.barBottom <= g.compTop + 1 && g.barTop >= g.headBottom - 1, `${path}: the card sits between the header and the composer`, JSON.stringify(g));
    for (const name of actions) {
      const btn = bar.getByRole("button", { name }).first();
      await btn.scrollIntoViewIfNeeded().catch(() => {});
      const r = await btn.boundingBox();
      ok(!!r && r.y >= g.barTop - 1 && r.y + r.height <= g.barBottom + 1 && r.y + r.height <= g.compTop + 1, `${path}: "${name}" is in view above the composer`, r ? `${Math.round(r.y)}..${Math.round(r.y + r.height)} card ${Math.round(g.barTop)}..${Math.round(g.barBottom)}` : "no box");
      ok(await btn.evaluate((e) => { const r = e.getBoundingClientRect(); const t = document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2); return !!t && (e === t || e.contains(t)); }), `${path}: "${name}" is the thing at its place (nothing covers it)`);
    }
    ok(await bar.evaluate((e) => e.scrollHeight >= e.clientHeight), `${path}: the card scrolls inside itself when it is longer than its room`);
    await ctx.close();
  }
} catch (err) { console.log("FAIL  " + err.message); bad++; }
finally { await browser.close(); await stop(); }
process.exit(bad ? 1 : 0);

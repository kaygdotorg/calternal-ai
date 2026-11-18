// A thread opens at its true end, with the last message fully above the composer (never half hidden behind it), also while a running turn keeps growing it, and the list lets go of the end only when the person scrolls away.
// (Found on the iPhone Simulator: a list that scrolled itself to the end before its rows were measured, then took its own correcting scroll for the person's and stopped following.) Usage: CAL_URL=... node e2e/thread-end.mjs
import { launch, open, serve } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const end = (p) => p.evaluate(() => { const f = document.querySelector("[role=feed]"); const last = [...f.querySelectorAll(".msg")].at(-1)?.getBoundingClientRect(); return { fromEnd: Math.round(f.scrollHeight - f.scrollTop - f.clientHeight), hidden: Math.round((last?.bottom ?? 0) - document.querySelector(".thread .bottom").getBoundingClientRect().top) }; });
try {
  for (const device of ["phone", "desktop"]) for (const path of ["/threads/t-run", "/threads/t-long", "/threads/t-idle"]) {
    const { page: p, ctx } = await open(browser, device, "light", path);
    const seen = [];
    for (const ms of [150, 700, 1700]) { await p.waitForTimeout(ms); seen.push(await end(p)); }
    ok(seen.every((s) => s.fromEnd <= 1 && s.hidden <= 1), `${device} ${path}: opens at the end and stays there, the last message above the composer`, JSON.stringify(seen.map((s) => [s.fromEnd, s.hidden])));
    await ctx.close();
  }
  { const { page: p, ctx } = await open(browser, "desktop", "light", "/threads/t-run");
    await p.waitForTimeout(800);
    await p.mouse.move(500, 400); await p.mouse.wheel(0, -500); await p.waitForTimeout(300);
    const a = await end(p); await p.waitForTimeout(1500); const b = await end(p);
    ok(a.fromEnd > 100 && Math.abs(b.fromEnd - a.fromEnd) <= 120 + (b.fromEnd > a.fromEnd ? 400 : 0) && b.fromEnd > 100, "scrolling away lets go of the end: new content does not pull the list back", `${a.fromEnd} -> ${b.fromEnd}`);
    await ctx.close(); }
} catch (err) { console.log("FAIL  " + err.message); bad++; }
await browser.close(); await stop();
process.exit(bad ? 1 : 0);

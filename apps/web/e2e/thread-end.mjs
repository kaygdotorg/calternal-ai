// A thread opens at its true end, with the last message fully above the composer (never half hidden behind it), also while a running turn keeps growing it, and the list lets go of the end only when the person scrolls away.
// (Found on the iPhone Simulator: a list that scrolled itself to the end before its rows were measured, then took its own correcting scroll for the person's and stopped following.) Usage: CAL_URL=... node e2e/thread-end.mjs
import { launch, open, serve } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
// Measured two frames after the call: a growing stream moves the end first and the list follows it in the next frame (a sample between the two is not a failure to follow).
const endOnce = (p) => p.evaluate(async () => { await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))); const f = document.querySelector("[role=feed]"); const last = [...f.querySelectorAll(".msg")].at(-1)?.getBoundingClientRect(); return { fromEnd: Math.round(f.scrollHeight - f.scrollTop - f.clientHeight), hidden: Math.round((last?.bottom ?? 0) - document.querySelector(".thread .bottom").getBoundingClientRect().top) }; });
// A stream that changes height (its row becomes the finished message) moves the end for a few frames before the list has followed: a sample in that window is retried for up to 400 ms and counts only if the list is not back at the end by then.
const end = async (p) => { let r = await endOnce(p); for (let i = 0; i < 8 && (r.fromEnd > 1 || r.hidden > 1); i++) { await p.waitForTimeout(50); r = await endOnce(p); } return r; };
try {
  for (const device of ["phone", "desktop"]) for (const path of ["/threads/t-run", "/threads/t-long", "/threads/t-idle"]) {
    const { page: p, ctx } = await open(browser, device, "light", path);
    // A busy machine renders later, so the list is first waited for (up to 10 s, the end shown and the last message clear of the composer); what is judged is that it then STAYS there while the rest of the page settles.
    await p.waitForFunction(() => { const f = document.querySelector("[role=feed]"); const last = [...(f?.querySelectorAll(".msg") ?? [])].at(-1); return !!last && f.scrollHeight - f.scrollTop - f.clientHeight <= 1; }, null, { timeout: 10000, polling: 100 }).catch(() => {});
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

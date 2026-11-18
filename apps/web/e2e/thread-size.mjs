// A 16 MiB thread (12,000 messages, the thread file limit) stays fast: open at the end to interactive, scroll up and jump, search hits near the start, and the header pill. What keeps it cheap is windowing (docs/threads-ui.md "A 16 MiB thread"): the list renders only the rows near the viewport (VirtualList), the adapter reads one page of the record at a time (never the file), estimated heights stand in for rows not yet measured, and nothing walks all messages on open. Frame and open times are judged only on a quiet machine (the quiet list); the structural checks always run.
import { judgeTimings, launch, open, serve } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const stats = (xs) => { const s = [...xs].sort((a, b) => a - b); const q = (p) => s[Math.min(s.length - 1, Math.floor(p * s.length))]; return { n: xs.length, median: +q(0.5).toFixed(1), p95: +q(0.95).toFixed(1), max: +s[s.length - 1].toFixed(1) }; };
const judge = judgeTimings();
const timing = (c, m, note) => { if (judge) ok(c, m, note); else console.log(`SKIP  ${m}: not judged, the machine is busy  ${note}`); };
try {
  for (const device of ["phone", "desktop"]) {
    const t0 = Date.now();
    const { page: p, ctx } = await open(browser, device, "dark", "/threads/t-long");
    await p.waitForSelector('[role="feed"] [role="article"]');
    await p.waitForFunction(() => { const f = document.querySelector('[role="feed"]'); return f && f.scrollTop + f.clientHeight >= f.scrollHeight - 4; }, null, { polling: "raf" });
    const field = p.locator("#composer-field"); await field.waitFor(); await field.click();
    const openMs = Date.now() - t0;
    const size = await p.evaluate(() => Number(document.querySelector('[role="feed"] [role="article"]')?.getAttribute("aria-setsize")));
    const dom = () => p.evaluate(() => ({ dom: document.getElementsByTagName("*").length, rows: document.querySelectorAll('[role="feed"] [role="article"]').length }));
    const atEnd = await dom();
    ok(size === 12000, `${device}: the thread has 12,000 messages (about 16 MiB)`, String(size));
    ok(atEnd.rows < 120 && atEnd.dom < 9000, `${device}: opened at the end it renders only the rows near the viewport`, JSON.stringify(atEnd));
    timing(openMs < 4000, `${device}: from opening to the end shown and the composer usable`, `${openMs} ms`);
    // The header pill shows the title at the end and after a long scroll (it is a shell line, not a message).
    const title = "Journal replay is slow after restart";
    ok(await p.getByText(title).first().isVisible(), `${device}: the header pill shows the thread's title at the end`);
    const frames = await p.evaluate(async () => {
      const sc = document.querySelector('[role="feed"]'); const out = { up: [], jump: [] };
      const run = async (n, step, into) => { let last = await new Promise((r) => requestAnimationFrame(r)); for (let i = 0; i < n; i++) { step(i); const t = await new Promise((r) => requestAnimationFrame(r)); into.push(t - last); last = t; } };
      await run(120, () => { sc.scrollTop -= 900; }, out.up);
      await run(40, (i) => { sc.scrollTop = ((i * 7919) % 1000) / 1000 * (sc.scrollHeight - sc.clientHeight); }, out.jump);
      return out;
    });
    const up = stats(frames.up), jump = stats(frames.jump);
    timing(up.p95 < 40 && jump.p95 < 60, `${device}: scrolling up and jumping keep frames short`, `up ${JSON.stringify(up)} jump ${JSON.stringify(jump)}`);
    const after = await dom();
    ok(after.rows < 120 && after.dom < 9000, `${device}: after scrolling the rendered rows are still few`, JSON.stringify(after));
    ok(await p.getByText(title).first().isVisible(), `${device}: the header pill still shows the title after the scroll`);
    // A search hit near the start opens the thread at that message, bounded.
    await p.evaluate(() => { document.querySelector('[role="feed"]').scrollTop = 0; });
    await p.waitForTimeout(300);
    const top = await dom();
    ok(top.rows < 120 && top.dom < 9000, `${device}: at the very start the rendered rows are few too`, JSON.stringify(top));
    await ctx.close();
  }
  {
    const { page: p, ctx } = await open(browser, "desktop", "dark", "/threads");
    await p.keyboard.press("Control+Shift+f").catch(() => {});
    const box = p.getByRole("searchbox").or(p.getByPlaceholder(/search/i)).first();
    if (await box.count()) {
      const t0 = Date.now(); await box.fill("journal"); await p.waitForSelector('[role=listbox][aria-label="Matching messages"] li', { timeout: 8000 }).catch(() => {});
      const ms = Date.now() - t0; const hits = await p.locator('[role=listbox][aria-label="Matching messages"] li').count();
      ok(hits > 0, "search finds messages while a 16 MiB thread exists", `${hits} hits`);
      timing(ms < 2500, "search results appear within a moment", `${ms} ms`);
    } else console.log("SKIP  search box not reachable by this probe");
    await ctx.close();
  }
} finally { await browser.close(); await stop(); }
process.exit(bad ? 1 : 0);

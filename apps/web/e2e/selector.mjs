// The Selector (the old app's state switch) as the Inbox / All Threads tabs: an outer pill with the chosen item a pill inside it; the capsule is on the choice at first paint, glides to a new choice with its leading edge ahead, goes straight there from the
// keyboard, never moves the labels when one turns bold, and keeps one tab stop. Usage: CAL_URL=... node e2e/selector.mjs
import { launch, open, serve } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const near = (a, b) => Math.abs(a - b) < 1.5;
const list = (p) => p.locator("aside.side [role=tablist][aria-label='Which threads']");
const capsule = (p) => list(p).locator(".capsule").evaluate((c) => { const r = c.getBoundingClientRect(); return { left: r.left, right: r.right, shown: c.classList.contains("shown") }; });
const chosen = (p) => list(p).locator("[role=tab][aria-selected=true]").evaluate((t) => { const r = t.getBoundingClientRect(); return { left: r.left, right: r.right, name: t.textContent.trim() }; });
try {
  const { page: p, ctx } = await open(browser, "desktop", "light", "/threads/t-idle");
  await p.waitForTimeout(700);
  ok((await list(p).count()) === 1 && (await list(p).locator(".capsule").count()) === 1, "the tabs are one Selector: a tab list with one capsule");
  let c = await capsule(p), o = await chosen(p);
  ok(c.shown && near(c.left, o.left) && near(c.right, o.right), "first render: the capsule is already on the choice, nothing sweeps in", `${o.name}`);
  const track = await list(p).evaluate((t) => { const r = t.getBoundingClientRect(), cs = getComputedStyle(t); return { radius: cs.borderTopLeftRadius, h: r.height }; });
  ok(parseFloat(track.radius) >= track.h / 2 - 1, "the track is a pill and the chosen item a pill inside it", JSON.stringify(track));
  // the glide: record every frame
  const tabs = list(p).locator("[role=tab]");
  const target = (await tabs.nth(0).getAttribute("aria-selected")) === "true" ? 1 : 0;
  const before = await capsule(p);
  const trace = await p.evaluate(async (i) => {
    const g = document.querySelector("aside.side [role=tablist]"), cap = g.querySelector(".capsule"), t = g.querySelectorAll("[role=tab]")[i];
    t.click(); const frames = [];
    for (let k = 0; k < 40; k++) { await new Promise(requestAnimationFrame); const r = cap.getBoundingClientRect(); frames.push([r.left, r.right]); }
    const r = t.getBoundingClientRect(); return { frames, t: [r.left, r.right] };
  }, target);
  const dir = trace.t[0] > before.left ? 1 : -1;
  const lead = trace.frames.some(([l, r]) => (dir > 0 ? r - before.right > l - before.left + 3 : before.left - l > before.right - r + 3));
  ok(lead, "pointer: the leading edge moves ahead and the other catches up (it stretches toward the new choice)");
  await p.waitForTimeout(500);
  c = await capsule(p); o = await chosen(p);
  ok(near(c.left, o.left) && near(c.right, o.right), "and it lands on the new choice", o.name);
  // keyboard: straight there
  await tabs.nth(target).focus();
  await p.keyboard.press(target === 1 ? "ArrowLeft" : "ArrowRight");
  const key = await p.evaluate(async () => { await new Promise(requestAnimationFrame); await new Promise(requestAnimationFrame); const g = document.querySelector("aside.side [role=tablist]"); const c = g.querySelector(".capsule").getBoundingClientRect(), t = g.querySelector("[role=tab][aria-selected=true]").getBoundingClientRect(); return { dl: Math.abs(c.left - t.left), dr: Math.abs(c.right - t.right) }; });
  ok(key.dl < 1.5 && key.dr < 1.5, "keyboard: the arrows choose and the capsule goes straight there, no glide", JSON.stringify(key));
  ok((await list(p).locator("[role=tab][tabindex='0']").count()) === 1, "only the chosen tab is in the Tab order");
  await p.keyboard.press("End"); await p.waitForTimeout(100);
  ok((await tabs.last().getAttribute("aria-selected")) === "true", "End chooses the last tab");
  // labels never move when they turn bold
  const lefts = await tabs.evaluateAll((a) => a.map((x) => { const r = x.querySelector(".w, .lbl").getBoundingClientRect(); return (r.left + r.right) / 2; }));
  await tabs.first().click(); await p.waitForTimeout(500);
  const after = await tabs.evaluateAll((a) => a.map((x) => { const r = x.querySelector(".w, .lbl").getBoundingClientRect(); return (r.left + r.right) / 2; }));
  ok(lefts.every((l, i) => Math.abs(l - after[i]) < 1.5), "the tabs and the centres of their labels do not move when the choice changes weight", `${lefts.map(Math.round)} -> ${after.map(Math.round)}`);
  // no search button beside the tabs, and where a thread search starts
  ok((await p.locator("aside.side").getByRole("button", { name: "Search messages" }).count()) === 0, "there is no extra search button beside the tabs");
  await p.keyboard.press("Control+Shift+F"); await p.locator(".msg-search-input").waitFor({ timeout: 3000 });
  ok(true, "Mod+Shift+F still opens the thread search");
  await p.keyboard.press("Escape"); await p.waitForTimeout(300);
  // from the palette
  await p.keyboard.press("Control+k"); await p.waitForTimeout(500); await p.keyboard.type("journal"); await p.waitForTimeout(500);
  const row = p.locator("[role=option]", { hasText: "Search messages for" });
  ok((await row.count()) === 1, "the palette offers Search messages for what was typed");
  await row.click(); await p.locator(".msg-search-input").waitFor({ timeout: 3000 });
  ok((await p.locator(".msg-search-input").inputValue()) === "journal", "and it opens the thread search with those words");
  await ctx.close();
} catch (err) { console.log("FAIL  " + err.message); bad++; }
await browser.close(); await stop();
process.exit(bad ? 1 : 0);

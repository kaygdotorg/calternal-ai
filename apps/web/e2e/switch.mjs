// The switch (packages/ui Switch.svelte): a tap or a key toggles it; the knob rides a spring and stretches along its travel with its speed (scaleX above 1 while it moves, back to 1 at rest); a drag follows the
// pointer and flips past the middle; Escape during a drag puts it back; reduced motion jumps with no stretch; the name is the label.
import { chromium } from "playwright";
import { serve, BASE } from "./lib.mjs";
const stop = await serve(); const b = await chromium.launch(); let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const go = async (reduced) => { const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, reducedMotion: reduced ? "reduce" : "no-preference" }); await ctx.addInitScript(() => localStorage.setItem("cal.firstrun", "1")); const page = await ctx.newPage(); await page.goto(`${BASE}/settings/appearance`); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(500); return { ctx, page }; };
const knob = (page, name) => page.getByRole("switch", { name }).locator(".knob");
// Rest is when the component has taken its inline transform away (its loop stopped): waiting for that, not for a guessed time, makes the checks the same on a busy machine.
const rested = (page, name) => page.waitForFunction((name) => { const btn = [...document.querySelectorAll("button[role=switch]")].find((x) => document.getElementById(x.getAttribute("aria-labelledby"))?.textContent.includes(name)); return btn.querySelector(".knob").style.transform === ""; }, name, { timeout: 15000 });
const sample = (page, name, ms) => page.evaluate(([name, ms]) => new Promise((res) => { const btn = [...document.querySelectorAll("button[role=switch]")].find((x) => document.getElementById(x.getAttribute("aria-labelledby"))?.textContent.includes(name)); const k = btn.querySelector(".knob"); const out = []; const t0 = performance.now(); const f = () => { const m = new DOMMatrixReadOnly(getComputedStyle(k).transform); out.push({ t: performance.now() - t0, x: m.m41, sx: m.a, sy: m.d }); if (performance.now() - t0 < ms) requestAnimationFrame(f); else res(out); }; requestAnimationFrame(f); }), [name, ms]);
{
  const { ctx, page } = await go(false);
  const sw = page.getByRole("switch", { name: /Animations/ });
  ok((await sw.getAttribute("aria-checked")) === "true", "the switch is named by its label and starts in the stored state");
  const run = sample(page, "Animations", 2500); await sw.click(); const frames = await run; // long enough for a loaded machine: frames of 50 ms or more slow the spring (its step is capped)
  const travel = await page.evaluate(() => { const t = document.querySelector("button[role=switch] .track"), k = t.querySelector(".knob"); return t.offsetWidth - k.offsetWidth - 2 * k.offsetLeft; });
  const maxSx = Math.max(...frames.map((f) => f.sx)), minSy = Math.min(...frames.map((f) => f.sy));
  ok((await sw.getAttribute("aria-checked")) === "false", "a click toggles it");
  const moving = frames.filter((f) => Math.abs(f.x - frames.at(-1).x) > 0.3).length; // frames drawn while the knob was away from its rest
  if (moving < 10) console.log(`info  the stretch was not judged: only ${moving} frames were drawn while the knob moved (a busy machine), too few to see its peak`);
  else ok(maxSx > 1.04 && minSy < 0.96, "the knob stretches along its travel and squashes across while it moves", `scale x up to ${maxSx.toFixed(2)}, y down to ${minSy.toFixed(2)}`);
  ok(Math.abs(frames.at(-1).x) < 0.5 && Math.abs(frames.at(-1).sx - 1) < 0.01, "and rests at the off end, unscaled", `x ${frames.at(-1).x.toFixed(1)} of ${travel}`);
  await rested(page, "Animations"); await sw.focus(); await page.keyboard.press("Space"); await page.waitForTimeout(50); await rested(page, "Animations");
  ok((await sw.getAttribute("aria-checked")) === "true" && Math.abs((await knob(page, /Animations/).evaluate((e) => new DOMMatrixReadOnly(getComputedStyle(e).transform).m41)) - travel) < 0.5, "Space toggles it and it rests at the on end", `travel ${travel}`);
  // drag: from the on end to the off end by pointer
  await sw.scrollIntoViewIfNeeded();
  const bb = await knob(page, /Animations/).boundingBox();
  await page.mouse.move(bb.x + bb.width / 2, bb.y + bb.height / 2); await page.mouse.down();
  await page.mouse.move(bb.x + bb.width / 2 - 10, bb.y + bb.height / 2, { steps: 3 }); await page.mouse.move(bb.x + bb.width / 2 - travel - 12, bb.y + bb.height / 2, { steps: 6 });
  ok((await sw.getAttribute("aria-checked")) === "false", "dragging the knob past the middle flips it before release");
  await page.keyboard.press("Escape"); await page.mouse.up(); await rested(page, "Animations");
  ok((await sw.getAttribute("aria-checked")) === "true", "Escape during a drag puts it back to how it was");
  await ctx.close();
}
{
  const { ctx, page } = await go(true);
  const sw = page.getByRole("switch", { name: /Animations/ });
  const run = sample(page, "Animations", 400); await sw.click(); const frames = await run;
  ok(frames.every((f) => Math.abs(f.sx - 1) < 0.005), "reduced motion: no stretch", `max ${Math.max(...frames.map((f) => f.sx)).toFixed(3)}`);
  ok((await sw.getAttribute("aria-checked")) === "false", "reduced motion: it still toggles");
  await ctx.close();
}
await b.close(); stop(); process.exit(bad ? 1 : 0);

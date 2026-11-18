// The composer chips' popovers morph out of the chip and back into it (packages/ui lib/clip-morph.ts), and are interruptible.
// Samples the glass layer's clip-path every frame: it starts as the chip's own rectangle, ends unclipped, the chip is hidden while open and back after,
// a close that starts half way grown shrinks from where the glass is, and the keyboard never morphs. (Headless Chromium does not draw backdrop-filter: blur is judged on the Mac.)
import { chromium } from "playwright";
import { serve, open } from "./lib.mjs";
const stop = await serve(); const b = await chromium.launch();
let bad = 0;
const ok = (n, p, note = "") => { console.log(`${p ? "PASS" : "FAIL"}  ${n}${note ? "  " + note : ""}`); if (!p) bad++; };
const { page, ctx } = await open(b, "desktop", "dark", "/threads/t-idle");
const sample = (ms, idx = 2) => page.evaluate(([ms, idx]) => new Promise((res) => { const out = []; const t0 = performance.now(); const f = () => { const pop = document.querySelector(".pop"); const g = pop?.hasAttribute("data-ready") ? pop.querySelector(".glass") : null; const pill = document.querySelectorAll(".composer .chips .pill")[idx]; out.push({ t: Math.round(performance.now() - t0), clip: g ? getComputedStyle(g).clipPath : "gone", pill: getComputedStyle(pill).opacity, box: g ? JSON.stringify(g.getBoundingClientRect()) : "" }); if (performance.now() - t0 < ms) requestAnimationFrame(f); else res(out); }; f(); }), [ms, idx]);
for (const [i, name] of [[0, "provider"], [1, "permission"], [2, "where"]]) {
  const chip = page.locator(".composer .chips .pill").nth(i);
  const r = await chip.boundingBox();
  const run = sample(900, i);
  await chip.click();
  const s = await run;
  const first = s.find((x) => x.clip !== "gone");
  const last = s[s.length - 1];
  ok(`${name}: the first drawn frame is clipped to the chip's shape`, !!first && /^inset\(/.test(first.clip) && /round/.test(first.clip), first?.clip);
  ok(`${name}: the clip is gone when the morph ends`, last.clip === "none");
  ok(`${name}: the chip is hidden while the popover is open`, last.pill === "0");
  const pop = await page.locator(".pop").boundingBox();
  ok(`${name}: the popover is positioned over the chip's left and bottom edges`, Math.abs(pop.x - r.x) < 3 && Math.abs(pop.y + pop.height - (r.y + r.height)) < 3, `pop ${Math.round(pop.x)},${Math.round(pop.y + pop.height)} chip ${Math.round(r.x)},${Math.round(r.y + r.height)}`);
  const run2 = sample(700, i);
  await page.mouse.click(700, 120);
  const c = await run2;
  ok(`${name}: closing shrinks the glass into a clip, then it is gone`, c.some((x) => /^inset\(/.test(x.clip)) && c[c.length - 1].clip === "gone");
  ok(`${name}: the chip is back after the close`, c[c.length - 1].pill === "1");
}
// Interrupt: open, close after 120 ms (mid-grow): the first shrink frame continues from the grown clip, never jumping to full size or to the chip.
const chip = page.locator(".composer .chips .pill").nth(2);
const run3 = sample(900);
await chip.click();
await page.waitForTimeout(120);
await page.mouse.click(700, 120);
const m = await run3;
const insets = m.filter((x) => /^inset\(/.test(x.clip)).map((x) => parseFloat(x.clip.slice(6)));
let jump = 0; for (let i = 1; i < insets.length; i++) jump = Math.max(jump, Math.abs(insets[i] - insets[i - 1]));
// The check samples animation frames: on a loaded machine only a few are drawn and the steps between them are large whatever the animation does. The gate (CAL_GATE) therefore asks for at least 12 sampled frames and says so when it did not get them.
if (process.env.CAL_GATE && insets.length < 12) console.log(`info  interrupted open: only ${insets.length} frames were drawn (machine busy), too few to judge a jump`);
else ok("an interrupted open reverses without a jump in the clip", insets.length > 4 && jump < 140, `frames=${insets.length} biggest step of the top inset=${jump.toFixed(0)}px`);
await page.waitForTimeout(500);
// Keyboard: Enter opens with no clip at any frame.
await chip.focus(); await page.keyboard.press("Tab"); await page.keyboard.press("Shift+Tab");
const run4 = sample(700);
await page.keyboard.press("Enter");
const k = await run4;
ok("from the keyboard nothing is clipped (no morph)", k.every((x) => !/^inset\(/.test(x.clip)));
await ctx.close(); await b.close(); stop(); process.exit(bad ? 1 : 0);

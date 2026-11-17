// Measures the shell's transitions in a real browser and records a video of a scripted session. For each transition: time from the trigger to 10, 50, 90 and 98 % of
// the change in one animated value, and whether reversing it half way is continuous (no jump). Values come from the motion tokens and the native study
// (push: 90 % by 0.23 s, settled by 0.4 s; Mac sheet and sidebar about 0.19 s; menus open in one frame). Usage: node e2e/motion.mjs [video-dir]  (after `npm run build`).
import { mkdirSync } from "node:fs";
import { DEVICES, BASE, launch, serve } from "./lib.mjs";

const videoDir = process.argv[2];
if (videoDir) mkdirSync(videoDir, { recursive: true });
const stop = await serve();
const browser = await launch();
const rows = [];

/** Samples `read()` (a number) every frame for `ms`, returns [t, v] pairs. Runs in the page. */
const SAMPLER = `window.__sample = (readSrc, ms) => new Promise((res) => { const read = new Function("return (" + readSrc + ")()"); const out = []; const t0 = performance.now(); const f = (t) => { out.push([t - t0, read()]); if (t - t0 < ms) requestAnimationFrame(f); else res(out); }; requestAnimationFrame(f); });`;
const timesOf = (s) => {
  const v0 = s[0][1], v1 = s[s.length - 1][1];
  // Times are from the first frame in which the value changes (the trigger's own latency is not motion).
  const start = s.find(([, v]) => Math.abs(v - v0) > Math.abs(v1 - v0) * 0.004)?.[0] ?? 0;
  const at = (p) => { const t = s.find(([, v]) => (v1 === v0 ? false : (v - v0) / (v1 - v0) >= p))?.[0]; return t === undefined ? undefined : t - start; };
  return { from: +v0.toFixed(3), to: +v1.toFixed(3), p10: Math.round(at(0.1) ?? NaN), p50: Math.round(at(0.5) ?? NaN), p90: Math.round(at(0.9) ?? NaN), p98: Math.round(at(0.98) ?? NaN) };
};
async function page(device, path) {
  const ctx = await browser.newContext({ ...DEVICES[device], colorScheme: "light", ...(videoDir ? { recordVideo: { dir: videoDir, size: device === "phone" ? { width: 390, height: 844 } : { width: 1280, height: 800 } } } : {}) });
  const p = await ctx.newPage();
  await p.addInitScript(SAMPLER);
  await p.goto(BASE + path);
  await p.waitForSelector("html[data-ready]");
  await p.waitForTimeout(800);
  return { p, ctx };
}
const measure = async (p, name, readSrc, trigger, ms = 700) => {
  const run = p.evaluate(([r, m]) => window.__sample(r, m), [readSrc, ms]);
  await p.waitForTimeout(30);
  await trigger();
  const s = await run;
  rows.push({ name, ...timesOf(s) });
  return s;
};

// ---- phone: the sidebar slides in under the floating card, the right panel slides over ----
{
  const { p, ctx } = await page("phone", "/threads/t-run");
  const tx = `() => new DOMMatrix(getComputedStyle(document.querySelector('.mid')).transform).m41`;
  await measure(p, "phone: middle card slides aside (sidebar opens)", tx, () => p.getByRole("button", { name: "Show threads" }).click());
  await p.waitForTimeout(300);
  await p.locator(".peek").click();
  await p.waitForTimeout(600);
  const rx = `() => new DOMMatrix(getComputedStyle(document.querySelector('.right')).transform).m41`;
  await measure(p, "phone: right panel slides over", rx, () => p.getByRole("tab", { name: "Changes" }).click());
  // interruptibility: reverse half way, check the value does not jump
  await p.waitForTimeout(400);
  await p.locator(".scrim").click({ position: { x: 8, y: 300 } });
  await p.waitForTimeout(120);
  const read = (src) => p.evaluate((x) => new Function("return (" + x + ")()")(), src);
  const before = await read(rx);
  await p.getByRole("tab", { name: "Changes" }).click({ force: true }).catch(() => {});
  const after = await read(rx);
  rows.push({ name: "phone: right panel reversed half way", from: +before.toFixed(1), to: +after.toFixed(1), note: Math.abs(after - before) < 40 ? "continuous (no jump)" : "JUMP" });
  await ctx.close();
}
// ---- desktop ----
{
  const { p, ctx } = await page("desktop", "/threads/t-run");
  const op = `() => parseFloat(getComputedStyle(document.querySelector('.side > .side')).opacity)`;
  await measure(p, "desktop: sidebar hides (content fades and slides)", op, () => p.getByRole("button", { name: "Hide the sidebar" }).click(), 500);
  await measure(p, "desktop: sidebar shows", op, () => p.getByRole("button", { name: "Show threads" }).click(), 500);
  // menu: one frame to visible, fade on close
  const mo = `() => { const e = document.querySelector('[role=menu]'); return e ? parseFloat(getComputedStyle(e).opacity) : 0; }`;
  await measure(p, "desktop: menu opens (provider)", mo, () => p.getByRole("button", { name: /Sonnet/ }).first().click(), 300);
  await p.keyboard.press("Escape");
  await p.waitForTimeout(300);
  // sheet
  const so = `() => { const e = document.querySelector('.sheet'); return e ? parseFloat(getComputedStyle(e).opacity) : 0; }`;
  await measure(p, "desktop: sheet opens (account)", so, () => p.getByRole("button", { name: /Account/ }).click(), 500);
  await p.keyboard.press("Escape");
  await p.waitForTimeout(400);
  // press
  const sc = `() => { const e = document.querySelector('button[aria-label="Search"]'); const s = getComputedStyle(e).scale; return s === "none" ? 1 : parseFloat(s); }`;
  const box = await p.getByRole("button", { name: "Search" }).first().boundingBox();
  await measure(p, "desktop: press scales a control to 0.96", sc, async () => { await p.mouse.move(box.x + 8, box.y + 8); await p.mouse.down(); }, 400);
  await p.mouse.up();
  // keyboard: no animation
  await p.keyboard.press("Tab");
  const kd = await p.evaluate(() => { const b = document.querySelector("button"); return { input: document.documentElement.dataset.input, duration: getComputedStyle(b).transitionDuration }; });
  rows.push({ name: "keyboard in use: transitions have no duration", from: 0, to: 0, note: `data-input=${kd.input}, transition-duration=${kd.duration}` });
  await ctx.close();
}
await browser.close();
stop();
console.log(JSON.stringify(rows, null, 1));

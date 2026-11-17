import { launch, serve, open } from "./lib.mjs";
const stop = await serve(); const b = await launch();
const { page, ctx } = await open(b, "phone", "light", "/notes/n-1");
const cdp = await ctx.newCDPSession(page); await cdp.send("Emulation.setCPUThrottlingRate", { rate: 4 });
const r = await page.evaluate(async () => {
  const el = document.elementFromPoint(4, 400);
  const fire = (t, x) => el.dispatchEvent(new PointerEvent(t, { bubbles: true, pointerType: "touch", pointerId: 5, clientX: x, clientY: 400, isPrimary: true }));
  const d = []; let last = performance.now(); let on = true;
  const f = (t) => { d.push(t - last); last = t; if (on) requestAnimationFrame(f); }; requestAnimationFrame(f);
  fire("pointerdown", 4);
  for (let i = 1; i <= 20; i++) { await new Promise((r) => setTimeout(r, 16)); fire("pointermove", 4 + i * 12); }
  fire("pointerup", 244);
  await new Promise((r) => setTimeout(r, 600)); on = false;
  d.shift(); const s = [...d].sort((a, b) => a - b);
  return { frames: d.length, median: s[s.length >> 1], p95: s[Math.floor(s.length * 0.95)], worst: s.at(-1), over33: d.filter((x) => x > 33).length };
});
console.log(JSON.stringify(r)); await b.close(); stop();

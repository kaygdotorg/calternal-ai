// Palette focus: how long after Ctrl+K until the field has focus, over many fresh opens (a 1-in-4 failure of the focus check was reported). Usage: node e2e/palette-focus.mjs [runs] [--4x]
import { launch, serve, open } from "./lib.mjs";
const N = +(process.argv[2] ?? 30), RATE = process.argv.includes("--4x") ? 4 : 1;
const stop = await serve(); const b = await launch();
const res = []; let never = 0;
for (let i = 0; i < N; i++) {
  const { page, ctx } = await open(b, "desktop", "light", "/threads/t-idle");
  if (RATE > 1) await (await ctx.newCDPSession(page)).send("Emulation.setCPUThrottlingRate", { rate: RATE });
  const r = await page.evaluate(async () => {
    const t0 = performance.now();
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "k", code: "KeyK", ctrlKey: true, bubbles: true }));
    let atVisible = null, focusedAt = null, stolen = 0, had = false;
    for (let k = 0; k < 400; k++) {
      await new Promise((r) => setTimeout(r, 2));
      const dlg = document.querySelector('[role=dialog]');
      if (dlg && atVisible === null) atVisible = { t: performance.now() - t0, field: document.activeElement?.classList.contains("pal-input") };
      const f = document.activeElement?.classList.contains("pal-input");
      if (f && focusedAt === null) { focusedAt = performance.now() - t0; had = true; }
      if (had && !f) stolen++;
    }
    return { atVisible, focusedAt, stolen };
  });
  res.push(r); if (r.focusedAt === null) never++;
  await ctx.close();
}
const lateAtVisible = res.filter((r) => r.atVisible && !r.atVisible.field).length;
const f = res.map((r) => r.focusedAt).filter((x) => x !== null).sort((a, c) => a - c);
console.log(JSON.stringify({ runs: N, cpuSlowdown: RATE, neverFocused: never, notFocusedWhenDialogAppeared: lateAtVisible, focusLostAfterGaining: res.filter((r) => r.stolen).length, focusMs: { median: +f[f.length >> 1]?.toFixed(1), worst: +f.at(-1)?.toFixed(1) } }));
await b.close(); stop();

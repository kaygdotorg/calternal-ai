// How the card's wallpaper looks on a high-density phone, in the engine that has no canvas filter (WebKit): a screenshot of an empty Threads screen at 390x844, 1x, 2x and 3x, and the size of the copies. A person looks at the pictures
// (SHOTS=<dir>); the script only reports whether the copy was blurred by hand and how big it is. Not in the gate (it needs WebKit).
import { webkit, chromium } from "playwright";
import { serve, BASE } from "./lib.mjs";
import { mkdirSync } from "node:fs";
const stop = await serve(); const dir = process.env.SHOTS ?? "/tmp/wallpaper-copy"; mkdirSync(dir, { recursive: true });
for (const [name, engine] of [["webkit", webkit], ["chromium", chromium]]) {
  let b; try { b = await engine.launch(); } catch (e) { console.log(`${name}: not available (${String(e).split("\n")[0]})`); continue; }
  for (const dpr of [1, 2, 3]) {
    const ctx = await b.newContext({ viewport: { width: 390, height: 844 }, deviceScaleFactor: dpr, isMobile: true, hasTouch: true, colorScheme: "dark" });
    await ctx.addInitScript(() => localStorage.setItem("cal.firstrun", "1"));
    const page = await ctx.newPage(); await page.goto(`${BASE}/threads`); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(1500);
    const info = await page.evaluate(async () => { const out = {}; for (const k of ["--wall-card", "--wall-side"]) { const u = getComputedStyle(document.documentElement).getPropertyValue(k).match(/url\("?([^")]+)/)?.[1]; if (!u) { out[k] = null; continue; } const img = new Image(); img.src = u; await img.decode(); const bytes = (await (await fetch(u)).blob()).size; out[k] = `${img.naturalWidth}x${img.naturalHeight} px, ${(bytes / 1024).toFixed(0)} KB`; } return out; });
    console.log(`${name} ${dpr}x: ${JSON.stringify(info)}`);
    await page.screenshot({ path: `${dir}/${name}-${dpr}x.png` });
    await ctx.close();
  }
  await b.close();
}
stop();

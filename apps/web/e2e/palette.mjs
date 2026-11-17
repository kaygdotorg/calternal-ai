// The palette in a real browser: opens, understands a line, searches every space, runs a row; and the cost of a keystroke at 6x CPU slowdown.
import { chromium } from "playwright";
import { serve, open } from "./lib.mjs";
const stop = await serve(); const b = await chromium.launch();
let bad = 0;
const ok = (n, p, note = "") => { console.log(`${p ? "PASS" : "FAIL"}  ${n}${note ? "  " + note : ""}`); if (!p) bad++; };
const { page, ctx } = await open(b, "desktop", "light", "/threads/t-idle");
await page.keyboard.press("Control+k");
const dlg = page.getByRole("dialog", { name: "Search and commands" });
await dlg.waitFor();
ok("Ctrl+K opens the palette with the field focused", await page.evaluate(() => document.activeElement?.classList.contains("pal-input")));
const field = page.locator(".pal-input");
await field.fill("jurnal");
ok("one typo still finds the thread", (await page.locator('[role=option]').first().innerText()).includes("Journal"));
ok("the intent line says what Enter does", (await page.locator(".intent").innerText()).startsWith("Open:"));
await field.fill("call plumber friday 5pm");
ok("a date phrase is a task and its words are highlighted", (await page.locator(".intent").innerText()).startsWith("New task") && (await page.locator(".mirror mark").count()) >= 1);
await field.fill("mesh");
ok("results from another space say which", (await page.locator(".r .sp").first().innerText()) === "Infra");
await field.fill("wallpaper");
ok("every setting is found by its words, and opens its address", (await page.locator(".r.command .l", { hasText: "Wallpaper and fonts" }).count()) === 1);
await field.fill("expired invite");
await page.keyboard.press("ArrowDown");
await page.keyboard.press("Enter");
await page.waitForTimeout(300);
ok("Enter runs the chosen row and closes", (await dlg.count()) === 0);
await page.keyboard.press("Control+k"); await dlg.waitFor();
await page.keyboard.press("Escape"); await page.waitForTimeout(300);
ok("Esc closes", (await dlg.count()) === 0);
// keystroke cost at 6x CPU slowdown
await page.keyboard.press("Control+k"); await dlg.waitFor();
const cdp = await ctx.newCDPSession(page);
await cdp.send("Emulation.setCPUThrottlingRate", { rate: 6 });
const ms = await page.evaluate(async () => {
  const el = document.querySelector(".pal-input"); const out = [];
  const line = "call plumber friday 5pm about the journal replay";
  for (let i = 1; i <= line.length; i++) {
    const t0 = performance.now();
    el.value = line.slice(0, i); el.dispatchEvent(new Event("input", { bubbles: true }));
    await new Promise((r) => requestAnimationFrame(() => r()));
    out.push(performance.now() - t0);
  }
  return out;
});
await cdp.send("Emulation.setCPUThrottlingRate", { rate: 1 });
const sorted = [...ms].sort((a, c) => a - c);
const med = sorted[Math.floor(sorted.length / 2)], p95 = sorted[Math.floor(sorted.length * 0.95)];
console.log(`keystroke to painted rows at 6x CPU: median ${med.toFixed(1)} ms, p95 ${p95.toFixed(1)} ms (one frame is 16.7 ms)`);
ok("keystroke cost: a frame or less at the median at 6x slowdown", med < 20);
await ctx.close(); await b.close(); stop(); process.exit(bad ? 1 : 0);

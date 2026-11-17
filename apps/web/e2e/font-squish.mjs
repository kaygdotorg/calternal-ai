// Is our Google Sans Flex squished against the reference? Renders the same text with (A) our shipped variable file with automatic optical size (the old rule),
// (B) the Google Fonts css2 static instance the owner's bb plugin installs (wght 400/500 at the font's default axes), and (C) the page as it is now (optical size pinned by --text-opsz); then measures ink (cap height, x-height, advance width) and screenshots them side by side.
// Usage: node e2e/font-squish.mjs <outdir>   (needs network once, for the css2 file).
import { mkdirSync, readFileSync } from "node:fs";
import { chromium } from "playwright";
import { serve, BASE } from "./lib.mjs";
const out = process.argv[2] ?? "/tmp/squish";
mkdirSync(out, { recursive: true });
const css = await (await fetch("https://fonts.googleapis.com/css2?family=Google+Sans+Flex:wght@400;500&display=swap", { headers: { "user-agent": "Mozilla/5.0 (X11; Linux x86_64) Chrome/130" } })).text();
const ref = [...css.matchAll(/font-weight: (\d+);[\s\S]*?src: url\(([^)]+)\)/g)].map((m) => ({ w: m[1], url: m[2] }));
const stop = await serve();
const browser = await chromium.launch();
const page = await (await browser.newContext({ viewport: { width: 1100, height: 760 }, deviceScaleFactor: 2 })).newPage();
await page.goto(BASE + "/threads");
await page.waitForTimeout(800);
const sample = "Hamburgefonstiv 0123 The invite expiry was compared in local time";
const html = `
<style>
${ref.map((r) => `@font-face { font-family: "Ref"; font-weight: ${r.w}; src: url(${r.url}); }`).join("\n")}
.row { margin: 10px 24px; display: grid; grid-template-columns: 150px 1fr; align-items: baseline; color: #222; }
.row b { font: 11px monospace; color: #777; }
.A { font-family: "Google Sans Flex"; font-weight: 440; font-optical-sizing: auto; font-variation-settings: "GRAD" 0; }
.B { font-family: "Ref"; font-weight: 400; }
.C { font-family: inherit; font-weight: 440; }
.s13 { font-size: 13px; line-height: 1.45; } .s17 { font-size: 17px; line-height: 1.45; }
</style>
<div id="cmp" style="position:fixed;inset:0;background:#fff;z-index:99999;padding-top:16px">
${["A", "B", "C"].flatMap((k) => ["s13", "s17"].map((s) => `<div class="row"><b>${{ A: "A before (auto opsz)", B: "B bb plugin (css2)", C: "C now (pinned opsz)" }[k]} ${s.slice(1)}px</b><span class="${k} ${s}">${sample}</span></div>`)).join("")}
</div>`;
await page.evaluate((h) => document.body.insertAdjacentHTML("beforeend", h), html);
await page.evaluate(() => document.fonts.ready);
await page.waitForTimeout(1500);
const m = await page.evaluate(async () => {
  // Width of the sample (inline-block) for the reference file at 13 px, and for ours at several optical sizes and weights.
  const w = (css) => { const e = document.createElement("span"); e.style.cssText = `position:absolute;visibility:hidden;white-space:nowrap;font-size:13px;${css}`; e.textContent = "Hamburgefonstiv 0123 The invite expiry was compared in local time"; document.body.append(e); const r = e.getBoundingClientRect().width; e.remove(); return +r.toFixed(1); };
  const res = { ref400: w('font-family:Ref;font-weight:400'), ref500: w('font-family:Ref;font-weight:500'), oursAuto440: w('font-family:"Google Sans Flex";font-weight:440'), oursAuto400: w('font-family:"Google Sans Flex";font-weight:400') };
  for (const o of [6, 10, 14, 18, 24, 36, 48, 72, 144]) res[`ours400 opsz${o}`] = w(`font-family:"Google Sans Flex";font-weight:400;font-optical-sizing:none;font-variation-settings:"opsz" ${o}`);
  const body = getComputedStyle(document.body);
  res.nowPage = w(`font-weight:440`); res.page = { family: body.fontFamily, weight: body.fontWeight, stretch: body.fontStretch, variation: body.fontVariationSettings, lh: body.lineHeight, ls: body.letterSpacing, synthesis: body.fontSynthesis, size: body.fontSize };
  return res;
});
console.log(JSON.stringify(m, null, 1));
await page.screenshot({ path: `${out}/squish.png`, clip: { x: 0, y: 0, width: 1100, height: 330 } });
await browser.close(); stop();

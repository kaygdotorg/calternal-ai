// Proof that the Rounded family (used only for the app name, as in the existing app) really rounds with "ROND" 100 in this app. Usage: node e2e/font-rond.mjs <out.jpg>
import { launch, serve, BASE } from "./lib.mjs";
const stop = await serve(); const b = await launch();
const page = await b.newPage({ viewport: { width: 900, height: 420 }, deviceScaleFactor: 1 });
await page.goto(BASE + "/threads/t-run");
await page.waitForSelector("html[data-ready]");
await page.evaluate(async ([base]) => {
  const st = document.createElement("style");
  st.textContent = `@font-face{font-family:"RA";src:url("${base}/fonts/google-sans-flex-latin-rond-normal.woff2") format("woff2-variations");font-weight:1 1000}
@font-face{font-family:"RB";src:url("${base}/fonts/google-sans-flex-latin-rond-normal.woff2") format("woff2");font-weight:1 1000}`;
  document.head.append(st);
  const d = document.createElement("div"); d.style.cssText = "position:fixed;inset:0;z-index:99;background:#fff;padding:10px;font-size:64px;line-height:1.05;font-weight:650";
  d.innerHTML = `<div style="font-family:RA;font-variation-settings:'ROND' 0">RA ROND0 Trlm</div><div style="font-family:RA;font-variation-settings:'ROND' 100">RA ROND100 Trlm</div><div style="font-family:RB;font-variation-settings:'ROND' 100">RB woff2 Trlm</div><div style="font-family:'Google Sans Flex Rounded';font-variation-settings:'ROND' 100">API Trlm</div>`;
  document.body.append(d);
  for (const f of ["RA", "RB"]) await document.fonts.load(`650 64px ${f}`);
}, [BASE]);
await page.waitForTimeout(500);
await page.screenshot({ path: process.argv[2] ?? "e2e/out/font-rond.jpg", type: "jpeg", quality: 92 });
await b.close(); stop();

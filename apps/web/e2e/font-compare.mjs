// Side-by-side proof that the app draws Google Sans Flex exactly as the existing app does. Left: the existing app's own CSS, taken verbatim from
// origin/review/switcher-a-pills (packages/ui/src/styles/tokens.css font faces, base.css body: weight, optical size and GRAD as it sets them).
// Right: this app, through its own page. Both render the same sentences at the same sizes in the same browser. Usage: node e2e/font-compare.mjs <out.jpg>
import { BASE, launch, open, serve } from "./lib.mjs";

const LATIN = "U+0000-00FF, U+0131, U+0152-0153, U+02BB-02BC, U+02C6, U+02DA, U+02DC, U+0304, U+0308, U+0329, U+2000-206F, U+20AC, U+2122, U+2191, U+2193, U+2212, U+2215, U+FEFF, U+FFFD";
const TEXT = "Hamburgefonstiv: the replay reads each entry with its own fsync (4.1 s to 0.3 s). Split `store.rs`.";
const rows = [[13, 440], [13, 620], [17, 440], [17, 460], [19, 620]];
const existing = `<style>
@font-face { font-family: "Google Sans Flex"; src: url("${BASE}/fonts/google-sans-flex-latin-opsz-normal.woff2") format("woff2-variations"); font-weight: 1 1000; font-display: optional; unicode-range: ${LATIN}; }
@font-face { font-family: "Google Sans Flex Dark"; src: url("${BASE}/fonts/google-sans-flex-latin-opsz-grad-normal.woff2") format("woff2-variations"); font-weight: 300 800; font-display: swap; unicode-range: ${LATIN}; }
:root { --font-ui: "Google Sans Flex", ui-sans-serif, system-ui, sans-serif; --text-grad: 0; }
:root[data-scheme="dark"] { --font-ui: "Google Sans Flex Dark", "Google Sans Flex", ui-sans-serif, system-ui, sans-serif; --text-grad: 30; color-scheme: dark; background: #1f1f1f; color: #ebebeb; }
body { margin: 0; padding: 12px; font-family: var(--font-ui); font-variation-settings: "GRAD" var(--text-grad, 0); line-height: 1.45; }
p { margin: 0 0 6px; } small { font: 11px monospace; color: gray; display: block; }
</style>`;
const lines = (cls) => rows.map(([s, w]) => `<small>${s}px / ${w}</small><p style="font-size:${s}px;font-weight:${w}">${TEXT}</p>`).join("");

const stop = await serve();
const b = await launch();
try {
  for (const scheme of ["light", "dark"]) {
    const ctx = await b.newContext({ viewport: { width: 1200, height: 460 }, deviceScaleFactor: 2, colorScheme: scheme });
    const page = await ctx.newPage();
    // Left: the existing app's CSS.
    await page.goto(`${BASE}/robots.txt`).catch(() => {});
    await page.setContent(`<html data-scheme="${scheme}">${existing}<body>${lines()}</body></html>`);
    await page.evaluate(() => document.fonts.ready);
    await page.waitForTimeout(600);
    await page.screenshot({ path: `${process.argv[2]}-${scheme}-existing.jpg`, type: "jpeg", quality: 92 });
    await ctx.close();
    // Right: this app's own page, with the same lines injected into its body so its real tokens, font stack and rules apply.
    const app = await open(b, "desktop", scheme, "/threads/t-run");
    await app.page.evaluate(([html]) => { const d = document.createElement("div"); d.id = "cmp"; d.style.cssText = "position:fixed;inset:0;z-index:99;background:var(--paper);padding:12px;overflow:hidden"; d.innerHTML = html; document.body.append(d); }, [lines()]);
    await app.page.setViewportSize({ width: 1200, height: 460 });
    await app.page.waitForTimeout(300);
    await app.page.screenshot({ path: `${process.argv[2]}-${scheme}-app.jpg`, type: "jpeg", quality: 92 });
    await app.ctx.close();
  }
} finally {
  await b.close();
  stop();
}

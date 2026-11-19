// In native chrome mode nothing scrolls legibly under the shell's controls (owner, 2026-11-19: on the phone the thread's text showed between the sidebar toggle, the title and More). The page's card fades out under the header
// region the shell reserves (`--chrome-top`: the status bar plus the native controls): fully transparent from the screen's top edge to the bottom of the region (the scroller's own edge fade, which multiplies with it, is off), then a short ramp to opaque (device numbers, 2026-11-19: --chrome-top 116 px, controls 62 to 106). Checked on the PIXELS of a transparent screenshot (the
// alpha channel), at phone width, for a thread and a Notes list, with the text scrolled under the region; no pixel inside the region may be legible. Usage: node e2e/native-fade.mjs
import { BASE, DEVICES, launch, serve } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
/** The values the shell sends on an iPhone Pro: the status bar (about 59 pt) plus the 44 pt controls and a gap; see docs/ios-shell.md. */
const CHROME_TOP = 116;
/** The most opaque pixel of every row of a transparent screenshot's top `rows` rows (device pixels / scale already 1 at the phone preset's deviceScaleFactor of 1). */
async function alphaRows(p, rows) {
  const png = (await p.screenshot({ omitBackground: true, clip: { x: 0, y: 0, width: DEVICES.phone.viewport.width, height: rows } })).toString("base64");
  return p.evaluate(async ({ png, rows }) => {
    const img = new Image(); img.src = "data:image/png;base64," + png; await img.decode();
    const c = document.createElement("canvas"); c.width = img.width; c.height = img.height; const g = c.getContext("2d", { willReadFrequently: true }); g.drawImage(img, 0, 0);
    const s = img.height / rows, d = g.getImageData(0, 0, c.width, c.height).data, out = [];
    for (let y = 0; y < rows; y++) { let m = 0; for (let x = 0; x < c.width; x++) for (let k = 0; k < s; k++) m = Math.max(m, d[(((Math.floor(y * s) + k) * c.width) + x) * 4 + 3]); out.push(m); }
    return out;
  }, { png, rows });
}
async function visit(label, path, scrollSel) {
  const ctx = await browser.newContext({ ...DEVICES.phone, colorScheme: "light" });
  await ctx.addInitScript(() => { try { localStorage.setItem("cal.firstrun", "1"); localStorage.setItem("cal.sidebar", "false"); } catch { /* none */ } window.calternalNative = { chromeVersions: [1], chrome: () => Promise.resolve({ native: true }), appearance() {}, ready() {}, log() {} }; });
  const p = await ctx.newPage();
  await p.goto(`${BASE}${path}`); await p.waitForSelector("html[data-ready]"); await p.waitForTimeout(1200);
  await p.evaluate((top) => document.documentElement.style.setProperty("--chrome-top", top + "px"), CHROME_TOP);
  await p.waitForTimeout(200);
  const mask = await p.evaluate(() => getComputedStyle(document.querySelector(".mid-card")).maskImage);
  ok(mask !== "none" && mask.includes("gradient"), `${label}: the card has the fade mask`, mask.slice(0, 70));
  // put text under the region: scroll the page's scroller so its content passes through the top
  await p.evaluate((sel) => { const s = document.querySelector(sel); if (s) s.scrollTop = 400; }, scrollSel);
  await p.waitForTimeout(400);
  const a = await alphaRows(p, CHROME_TOP + 60);
  const seen = Math.max(...a.slice(0, CHROME_TOP));
  ok(seen <= 8, `${label}: nothing is legible anywhere inside the header region (the controls are over y 62 to 106): the most opaque pixel in rows 0 to ${CHROME_TOP}`, `max alpha ${seen} of 255`);
  const below = Math.max(...a.slice(CHROME_TOP + 20, CHROME_TOP + 60));
  ok(below > 150, `${label}: below the region (after the ramp) the text is drawn (the test sees ink)`, `max alpha ${below}`);
  await ctx.close();
}
try {
  await visit("thread", "/threads/t-run", ".scroller");
  await visit("settings", "/settings", ".scroller");
} finally { await browser.close(); stop(); }
console.log(bad ? `${bad} failed` : "native fade ok");
process.exit(bad ? 1 : 0);

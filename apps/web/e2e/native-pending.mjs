// The page never draws its own chrome beside the shell's (owner, 2026-11-19: the iPhone shell showed the page's own sidebar inside the card next to the native one). With a bridge that can draw the contract
// (`chrome` and `chromeVersions` including 1) the page is provisionally native from before the first paint: its sidebar, drawer, header pills and wallpaper are not drawn, whatever the route or the stored drawer
// state; a reply `native: true` keeps that; a refusal (or an error) gives the page its own chrome back; a bridge that cannot draw the contract changes nothing. Usage: node e2e/native-pending.mjs
import { BASE, DEVICES, launch, serve } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
async function visit(label, stub, check) {
  const ctx = await browser.newContext({ ...DEVICES.phone, colorScheme: "light" });
  await ctx.addInitScript((s) => { try { localStorage.setItem("cal.firstrun", "1"); localStorage.setItem("cal.sidebar", "true"); } catch { /* none */ } if (s) window.calternalNative = eval(`(${s})`); }, stub);
  const p = await ctx.newPage();
  await p.goto(`${BASE}/threads/t-run`);
  await p.waitForSelector("html[data-ready]");
  await check(p, label);
  await ctx.close();
}
const hidden = (p) => p.evaluate(() => { const vis = (sel) => { const e = document.querySelector(sel); return !!e && getComputedStyle(e).display !== "none" && e.getBoundingClientRect().width > 0; }; return { side: vis(".shell .side"), edge: vis(".shell .edge"), backdrop: vis(".backdrop") }; });
try {
  const native = "{ chromeVersions: [1], chrome: () => new Promise((r) => setTimeout(() => r({ native: true }), 1500)), appearance() {}, ready() {}, log() {} }";
  await visit("pending, then native", native, async (p, l) => {
    const h = await hidden(p);
    ok(!h.side && !h.edge && !h.backdrop, `${l}: before any reply the page's own sidebar, pills and wallpaper are not drawn`, JSON.stringify(h));
    await p.waitForTimeout(2500);
    const h2 = await hidden(p);
    ok(!h2.side && !h2.edge && !h2.backdrop && (await p.evaluate(() => document.documentElement.hasAttribute("data-native-chrome-page"))), `${l}: after native: true it stays that way`, JSON.stringify(h2));
  });
  const refuse = "{ chromeVersions: [1], chrome: () => Promise.resolve({ native: false, reason: 'unsupported' }), appearance() {}, ready() {}, log() {} }";
  await visit("refused", refuse, async (p, l) => {
    await p.waitForTimeout(800);
    ok(await p.evaluate(() => !document.documentElement.hasAttribute("data-native-chrome-pending") && !document.documentElement.hasAttribute("data-native-chrome-page")), `${l}: a refusal gives the page its own chrome back`);
  });
  const old = "{ appearance() {}, ready() {}, log() {} }";
  await visit("old shell", old, async (p, l) => {
    ok(await p.evaluate(() => !document.documentElement.hasAttribute("data-native-chrome-pending")), `${l}: a bridge that cannot draw the contract changes nothing`);
  });
  await visit("no bridge", null, async (p, l) => {
    const h = await hidden(p);
    ok(h.backdrop || h.edge, `${l}: a browser draws the page's own chrome`, JSON.stringify(h));
  });
} finally { await browser.close(); stop(); }
console.log(bad ? `${bad} failed` : "native pending ok");
process.exit(bad ? 1 : 0);

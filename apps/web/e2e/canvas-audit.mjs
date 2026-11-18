// Side-by-side audit of canvas=0 and canvas=1 (a single window, the same thread, the same size): screenshots and every element whose computed type, spacing, radius or position differs.
// Usage: CAL_URL=... node e2e/canvas-audit.mjs [outdir]  (evidence for the audit on issue #33; not part of the gate)
import { mkdirSync, writeFileSync } from "node:fs";
import { chromium } from "playwright";
import { serve, BASE } from "./lib.mjs";
const dir = process.argv[2] ?? "/tmp/canvas-audit"; mkdirSync(dir, { recursive: true });
const stop = await serve(); const browser = await chromium.launch();
const PROPS = ["fontFamily", "fontSize", "fontWeight", "lineHeight", "letterSpacing", "fontVariationSettings", "color", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "marginTop", "marginRight", "marginBottom", "marginLeft", "borderRadius", "gap", "backgroundColor", "boxShadow", "opacity", "transitionProperty", "transitionDuration", "animationName"];
async function grab(canvas, scheme) {
  const ctx = await browser.newContext({ viewport: { width: 1500, height: 970 }, deviceScaleFactor: 2, colorScheme: scheme });
  await ctx.addInitScript(() => { try { localStorage.setItem("cal.firstrun", "1"); sessionStorage.clear(); localStorage.setItem("cal.canvas.seeded.2", "1"); localStorage.setItem("cal.canvas", JSON.stringify({ cols: [{ p: 1, wins: [{ id: "w1", kind: "threads", item: "t-idle", weight: 1, look: "card", locked: false }] }], focus: "w1", x: 0 })); } catch { /* none */ } });
  const p = await ctx.newPage();
  await p.goto(BASE + `/threads/t-idle${canvas ? "?canvas=1" : ""}`); await p.waitForSelector("html[data-ready]"); await p.waitForTimeout(2800);
  await p.screenshot({ path: `${dir}/${scheme}-canvas${canvas ? 1 : 0}.png` });
  const data = await p.evaluate((PROPS) => {
    const root = document.querySelector(".thread"); const card = (root.closest("[data-win]") ?? root.closest(".mid-card")); const cr = card.getBoundingClientRect();
    const out = [];
    const walk = (el, path) => {
      const cs = getComputedStyle(el); const r = el.getBoundingClientRect();
      const sig = path + ">" + el.tagName.toLowerCase() + (typeof el.className === "string" && el.className ? "." + el.className.split(/\s+/).filter((c) => !/^svelte-|^s-/.test(c)).join(".") : "");
      out.push({ sig, style: Object.fromEntries(PROPS.map((k) => [k, cs[k]])), box: [Math.round((r.left - cr.left) * 10) / 10, Math.round((r.top - cr.top) * 10) / 10, Math.round(r.width * 10) / 10, Math.round(r.height * 10) / 10], text: (el.childNodes[0]?.nodeType === 3 ? el.childNodes[0].textContent.trim().slice(0, 24) : "") });
      for (const c of el.children) walk(c, sig);
    };
    walk(root, "");
    const cardCs = getComputedStyle(card);
    return { rows: out, card: { box: [cr.left, cr.top, cr.width, cr.height], radius: cardCs.borderRadius, shadow: cardCs.boxShadow, bg: cardCs.backgroundColor } };
  }, PROPS);
  await ctx.close(); return data;
}
const report = {};
for (const scheme of ["dark", "light"]) {
  const a = await grab(false, scheme), b = await grab(true, scheme);
  const diffs = [];
  const bySig = new Map(b.rows.map((r) => [r.sig, r]));
  for (const r of a.rows) {
    const o = bySig.get(r.sig); if (!o) { diffs.push({ sig: r.sig, missing: "in canvas=1" }); continue; }
    const d = Object.keys(r.style).filter((k) => r.style[k] !== o.style[k] && !(k === "color" || k === "backgroundColor") ).map((k) => `${k}: ${r.style[k]} -> ${o.style[k]}`);
    const boxd = r.box.some((v, i) => Math.abs(v - o.box[i]) > 0.6) ? `box ${r.box.join(",")} -> ${o.box.join(",")}` : "";
    if (d.length || boxd) diffs.push({ sig: r.sig.slice(-90), text: r.text, d, boxd });
  }
  report[scheme] = { card: { canvas0: a.card, canvas1: b.card }, count0: a.rows.length, count1: b.rows.length, diffs };
}
writeFileSync(`${dir}/audit.json`, JSON.stringify(report, null, 1));
console.log(JSON.stringify({ dark: { card: report.dark.card, n: report.dark.diffs.length }, light: { n: report.light.diffs.length } }));
await browser.close(); await stop();

// The tool deck (docs/threads-ui.md, "Tool steps"): the folded "Worked for" row shows the tools' icons, or the thumbnails of pictures they produced, as a small overlapping deck; pointing at the row fans it out (pointer only; no motion under reduced motion);
// a press opens the block; a step that produced or read a picture or a file has a real link (a new tab). Usage: CAL_URL=... node e2e/tool-deck.mjs
import { launch, open, serve } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
try {
  const { page: p, ctx } = await open(browser, "desktop", "dark", "/threads/t-idle");
  await p.waitForTimeout(800);
  const head = p.locator(".steps .head").first();
  await head.scrollIntoViewIfNeeded();
  const gaps = () => head.evaluate((h) => { const c = [...h.querySelectorAll(".chip")].map((x) => x.getBoundingClientRect()); return c.slice(1).map((r, i) => Math.round((r.left - c[i].right) * 10) / 10); });
  const rest = await gaps();
  ok(rest.length >= 1 && rest.every((g) => g < 0), "the chips overlap in a small deck at rest", rest.join(","));
  await head.hover(); await p.waitForTimeout(600);
  const fanned = await gaps();
  ok(fanned.every((g, i) => g > rest[i]), "pointing at the row fans the deck out", `${rest.join(",")} -> ${fanned.join(",")}`);
  await p.mouse.move(5, 5); await p.waitForTimeout(600);
  ok((await gaps()).every((g, i) => Math.abs(g - rest[i]) < 0.6), "leaving folds it back");
  await head.click(); await p.waitForTimeout(500);
  ok((await head.getAttribute("aria-expanded")) === "true", "a press opens the block");
  // links: find a step with a file
  const links = p.locator(".steps .file");
  const n = await links.count();
  if (n === 0) { // scroll the list until a message with a linked step is mounted
    for (let i = 0; i < 30 && (await links.count()) === 0; i++) { await p.mouse.move(700, 400); await p.mouse.wheel(0, -900); await p.waitForTimeout(200); const h = p.locator(".steps .head").first(); if (await h.count()) await h.click().catch(() => {}); }
  }
  const first = links.first();
  ok((await links.count()) > 0 && (await first.getAttribute("target")) === "_blank" && /noopener/.test((await first.getAttribute("rel")) ?? "") && !!(await first.getAttribute("href")), "a step that produced or read a picture or a file has a real link that opens in a new tab");
  const img = p.locator(".steps .file.image img").first();
  ok((await img.count()) === 0 || (await img.evaluate((e) => e.getBoundingClientRect().width)) > 40, "a picture shows as a thumbnail");
  await ctx.close();
} catch (err) { console.log("FAIL  " + err.message); bad++; }
await browser.close(); await stop();
process.exit(bad ? 1 : 0);

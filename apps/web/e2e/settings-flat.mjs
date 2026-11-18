// Settings on a desktop card: every page is one flat page (no list of sections beside or inside it), and the layout preference (Appearance, Layout, Width: the one the chat uses) decides how wide the page is:
// Wide (the default on a desktop) uses the card's width, Centred a readable column. Usage: node e2e/with-preview.mjs e2e/settings-flat.mjs
import { launch, open, serve } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
try {
  const { page: p, ctx } = await open(browser, "desktop", "light", "/settings/appearance?firstrun=0");
  await p.waitForTimeout(600);
  const flat = () => p.evaluate(() => ({ sec: document.querySelectorAll(".page .sec, .page nav[aria-label$='sections']").length, split: document.querySelectorAll(".split:not(.withpv)").length, groups: document.querySelectorAll(".page section.g").length, w: Math.round(document.querySelector(".page").getBoundingClientRect().width), card: Math.round(document.querySelector(".scroller").getBoundingClientRect().width) }));
  let f = await flat();
  ok(f.sec === 0 && f.split === 0 && f.groups >= 8, "desktop: Appearance is one flat page: every section under its heading, no section list", JSON.stringify(f));
  ok(f.w > 560 && f.w >= f.card - 4, "desktop: Wide is the default and the page uses the card's width", `${f.w} of ${f.card}`);
  const sel = p.locator("[data-setting='appearance.chat-width']").getByRole("tab", { name: "Centred" });
  await sel.scrollIntoViewIfNeeded(); await sel.click(); await p.waitForTimeout(500);
  f = await flat();
  ok(f.w <= 570, "desktop: Centred keeps a readable column", `${f.w}`);
  const chatWide = await p.evaluate(() => document.documentElement.hasAttribute("data-chat-wide"));
  ok(!chatWide, "desktop: the same preference the chat reads (data-chat-wide) went off");
  await p.locator("[data-setting='appearance.chat-width']").getByRole("tab", { name: "Wide" }).click(); await p.waitForTimeout(500);
  f = await flat(); ok(f.w > 560, "desktop: back to Wide, the page is wide again", `${f.w}`);
  // the live preview: beside the settings on a wide card in Wide, inert and hidden from assistive technology, sticky while the page scrolls; none when Centred or on a narrow card
  f = await p.evaluate(() => { const v = document.querySelector(".pv"); return { has: !!v, inert: v?.hasAttribute("inert"), hidden: v?.getAttribute("aria-hidden"), top0: Math.round(v?.getBoundingClientRect().top ?? -1) }; });
  ok(f.has && f.inert && f.hidden === "true", "desktop: Appearance on a wide card shows the live preview, inert and hidden from assistive technology", JSON.stringify(f));
  await p.locator(".scroller").evaluate((e) => { e.scrollTop = 900; }); await p.waitForTimeout(400);
  const top1 = await p.evaluate(() => Math.round(document.querySelector(".pv").getBoundingClientRect().top));
  ok(top1 > 0 && top1 < 200, "desktop: it stays in view while the page scrolls (sticky)", `top ${top1}`);
  await p.locator("[data-setting='appearance.chat-width']").getByRole("tab", { name: "Centred" }).click(); await p.waitForTimeout(400);
  ok((await p.locator(".pv").count()) === 0, "desktop: no preview when the layout is Centred");
  await p.locator("[data-setting='appearance.chat-width']").getByRole("tab", { name: "Wide" }).click(); await p.waitForTimeout(300);
  await p.setViewportSize({ width: 1000, height: 900 }); await p.waitForTimeout(500);
  ok((await p.locator(".pv").count()) === 0, "desktop: none on a card under 64 rem");
  await p.setViewportSize({ width: 1440, height: 900 });
  for (const pg of ["agents", "members", "keyboard"]) {
    await p.goto(p.url().replace(/\/settings\/.*/, `/settings/${pg}`)); await p.waitForTimeout(600);
    const g = await flat(); ok(g.sec === 0 && g.split === 0, `desktop: /settings/${pg} is flat too`);
  }
  await ctx.close();
} catch (err) { console.log("FAIL  " + err.message); bad++; }
await browser.close(); await stop();
process.exit(bad ? 1 : 0);

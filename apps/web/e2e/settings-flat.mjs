// Settings on a desktop card: every page is one flat page (no list of sections beside or inside it), and the layout preference (Appearance, Layout, Width: the one the chat uses) decides how wide the page is:
// Wide (the default on a desktop) uses the card's width, Centred a readable column. Usage: node e2e/with-preview.mjs e2e/settings-flat.mjs
import { launch, open, serve } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
try {
  const { page: p, ctx } = await open(browser, "desktop", "light", "/settings/appearance?firstrun=0");
  await p.waitForTimeout(600);
  const flat = () => p.evaluate(() => ({ sec: document.querySelectorAll(".page .sec, .page nav[aria-label$='sections']").length, split: document.querySelectorAll(".split").length, groups: document.querySelectorAll(".page section.g").length, w: Math.round(document.querySelector(".page").getBoundingClientRect().width), card: Math.round(document.querySelector(".scroller").getBoundingClientRect().width) }));
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
  for (const pg of ["agents", "members", "keyboard"]) {
    await p.goto(p.url().replace(/\/settings\/.*/, `/settings/${pg}`)); await p.waitForTimeout(600);
    const g = await flat(); ok(g.sec === 0 && g.split === 0, `desktop: /settings/${pg} is flat too`);
  }
  await ctx.close();
} catch (err) { console.log("FAIL  " + err.message); bad++; }
await browser.close(); await stop();
process.exit(bad ? 1 : 0);

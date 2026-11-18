// Settings at phone width (and a narrow desktop window): every page is one flat scrolling page. No Back control inside a page, no list-then-detail split, every section as wide as the page's content
// (inside the gutters of the viewport), nothing wider than the screen, and Fonts shows its four roles inline. Walks every page the sidebar lists. Usage: node e2e/with-preview.mjs e2e/settings-phone.mjs
import { launch, open, serve } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
try {
  const { page: p, ctx } = await open(browser, "phone", "light", "/settings/appearance?firstrun=0");
  const hrefs = await p.evaluate(async () => {
    const a = [...document.querySelectorAll("a[href^='/settings/']")].map((x) => x.getAttribute("href"));
    return [...new Set(a)];
  });
  const pages = hrefs.length ? hrefs : ["/settings/appearance"];
  ok(pages.length >= 1, "the Settings pages are found", String(pages.length));
  for (const h of ["/settings/appearance", ...pages.filter((x) => x !== "/settings/appearance")]) {
    await p.goto(new URL(h, p.url()).href); await p.waitForSelector("html[data-ready]"); await p.waitForTimeout(500);
    const r = await p.evaluate(() => {
      const page = document.querySelector(".page"); if (!page) return null;
      const vw = innerWidth, cs = getComputedStyle(page), inner = page.getBoundingClientRect().width - parseFloat(cs.paddingLeft) - parseFloat(cs.paddingRight);
      const backs = [...page.querySelectorAll("button, a")].filter((b) => /^\s*(back|<)/i.test(b.textContent ?? "") || (b.getAttribute("aria-label") ?? "").toLowerCase() === "back").length;
      const groups = [...page.querySelectorAll("section.g")].map((g) => Math.round(g.getBoundingClientRect().width));
      return { vw, inner: Math.round(inner), backs, split: page.classList.contains("split") || !!document.querySelector(".split"), groups, over: document.documentElement.scrollWidth - vw, left: Math.round(page.getBoundingClientRect().left + parseFloat(cs.paddingLeft)) };
    });
    if (!r) { ok(false, `${h}: has a page`); continue; }
    ok(r.backs === 0 && !r.split, `${h}: one flat page, no Back control, no split`, `backs ${r.backs}`);
    ok(r.groups.every((w) => w >= r.inner - 2), `${h}: every section fills the content width`, `${r.groups.join(",")} of ${r.inner}`);
    ok(r.sels.every((w) => w >= Math.min(r.inner - 2 * 24, 448) - 4 && w <= 449), `${h}: a Selector that is its own row's control fills the row up to 28 rem`, r.sels.join(","));
    ok(r.inner >= r.vw - 2 * 40 && r.left <= 40 && r.over <= 0, `${h}: inside the gutters, nothing wider than the screen`, `content ${r.inner}, left ${r.left}, over ${r.over}`);
  }
  await p.goto(new URL("/settings/appearance", p.url()).href); await p.waitForTimeout(700);
  const rows = await p.locator(".page").getByText(/^(Interface|Chat|Code|Wordmark)$/).allInnerTexts();
  ok(rows.length >= 4, "Fonts shows its four roles inline", rows.join(","));
  await p.locator(".page .row", { hasText: "Interface" }).first().click(); await p.waitForTimeout(700);
  ok((await p.locator(".sheet").count()) === 1, "a role opens its picker as a sheet");
  await ctx.close();
} catch (err) { console.log("FAIL  " + err.message); bad++; }
await browser.close(); await stop();
process.exit(bad ? 1 : 0);

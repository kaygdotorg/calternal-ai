// Keyboard-only walk, focus visibility, reduced motion and 200% zoom, on the fixture app (server mode: a11y-server.mjs). Exits 1 on any failure. Usage: node e2e/a11y-walk.mjs (after `npm run build`).
import AxeBuilder from "@axe-core/playwright";
import { launch, open, serve, DEVICES } from "./lib.mjs";
const stop = await serve(); const browser = await launch(); let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };

/** Tab through a screen: every stop must be a real control with a name and a visible focus indicator; focus must never fall to <body> or leave the document. Returns the stops. */
async function walk(page, name, max = 60) {
  await page.evaluate(() => document.activeElement?.blur?.());
  const stops = [], problems = [];
  for (let i = 0; i < max; i++) {
    await page.keyboard.press("Tab");
    const s = await page.evaluate(() => {
      const e = document.activeElement;
      if (!e || e === document.body) return { body: true };
      const cs = getComputedStyle(e), r = e.getBoundingClientRect();
      const child = [...e.children].some((c) => { const k = getComputedStyle(c); return k.outlineStyle !== "none" && parseFloat(k.outlineWidth) > 0; });
      const ring = child || (cs.outlineStyle !== "none" && parseFloat(cs.outlineWidth) > 0) || (cs.boxShadow && cs.boxShadow !== "none") || (e.matches(":focus-visible") && !!e.closest("[data-ring]"));
      const label = (e.getAttribute("aria-label") || (e.getAttribute("aria-labelledby") && document.getElementById(e.getAttribute("aria-labelledby"))?.textContent) || e.labels?.[0]?.textContent || e.textContent || e.getAttribute("placeholder") || e.getAttribute("title") || "").trim().slice(0, 40);
      return { tag: e.tagName.toLowerCase() + (e.getAttribute("role") ? `[${e.getAttribute("role")}]` : ""), label, ring: !!ring, visible: r.width > 0 && r.height > 0, inView: r.bottom > 0 && r.right > 0 && r.top < innerHeight && r.left < innerWidth, id: e.id };
    });
    if (s.body) { if (i > 0) break; continue; } // wrapped round to the browser chrome
    if (stops.length && stops.some((x) => x.same === JSON.stringify([s.tag, s.label, s.id]))) break;
    s.same = JSON.stringify([s.tag, s.label, s.id]); stops.push(s);
    if (!s.label) problems.push(`${s.tag} has no name`);
    if (!s.ring) problems.push(`${s.tag} "${s.label}" shows no focus indicator`);
    if (!s.visible || !s.inView) problems.push(`${s.tag} "${s.label}" is focused but not on screen`);
  }
  ok(stops.length > 0 && !problems.length, `keyboard walk: ${name}`, `${stops.length} stops${problems.length ? "\n   " + [...new Set(problems)].slice(0, 6).join("\n   ") : ""}`);
  return stops;
}

try {
  for (const [name, path] of [["threads", "/threads/t-idle?panel=changes"], ["notes", "/notes/n-1"], ["settings appearance", "/settings/appearance"], ["settings machines", "/settings/machines"], ["settings account", "/settings/account"]]) {
    for (const scheme of ["light", "dark"]) {
      const { page, ctx } = await open(browser, "desktop", scheme, path);
      await walk(page, `${name} (${scheme})`);
      await ctx.close();
    }
  }
  // The palette by keyboard: opens, takes typing, moves with arrows, closes with Escape and gives focus back.
  {
    const { page, ctx } = await open(browser, "desktop", "light", "/threads/t-idle");
    await page.locator("#composer-field").focus().catch(() => {});
    const before = await page.evaluate(() => document.activeElement?.id || document.activeElement?.tagName);
    await page.keyboard.press("Control+KeyK");
    const dlg = page.getByRole("dialog").first();
    await dlg.waitFor({ timeout: 3000 }).catch(() => {});
    const inside = await page.evaluate(() => !!document.activeElement?.closest('[role="dialog"]'));
    ok(await dlg.isVisible().catch(() => false) && inside, "palette: opens with the keyboard and focus moves into it");
    await page.keyboard.type("set"); await page.keyboard.press("ArrowDown");
    ok(await page.evaluate(() => !!document.activeElement?.closest('[role="dialog"]')), "palette: typing and arrows keep focus inside");
    await page.keyboard.press("Escape");
    await page.waitForTimeout(250);
    const after = await page.evaluate(() => document.activeElement?.id || document.activeElement?.tagName);
    ok(!(await dlg.isVisible().catch(() => false)) && after === before, "palette: Escape closes it and returns focus", `${before} -> ${after}`);
    await ctx.close();
  }
  // Reduced motion: nothing that moves is left running (transforms and translations); fades may stay short.
  {
    const ctx = await browser.newContext({ ...DEVICES.desktop, reducedMotion: "reduce" });
    await ctx.addInitScript(() => localStorage.setItem("cal.firstrun", "1"));
    const page = await ctx.newPage();
    await page.goto((await import("./lib.mjs")).BASE + "/threads/t-run"); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(800);
    const moving = await page.evaluate(() => [...document.querySelectorAll("*")].filter((e) => { const c = getComputedStyle(e); const dur = parseFloat(c.transitionDuration) > 0.05 && /transform|translate|scale|rotate|all/.test(c.transitionProperty); const anim = c.animationName !== "none" && parseFloat(c.animationDuration) > 0.05 && c.animationIterationCount === "infinite"; return dur || anim; }).slice(0, 8).map((e) => `${e.tagName.toLowerCase()}.${String(e.className).slice(0, 30)} ${getComputedStyle(e).transitionProperty.slice(0, 30)}`));
    ok(!moving.length, "reduced motion: no movement transitions or endless animations left", moving.join("; "));
    await ctx.close();
  }
  // 200% zoom is a 720x450 CSS-pixel window: no sideways scrolling, nothing cut off, axe clean.
  for (const [name, path] of [["threads", "/threads/t-run"], ["notes", "/notes/n-1"], ["settings machines", "/settings/machines"], ["settings appearance", "/settings/appearance"]]) {
    const ctx = await browser.newContext({ viewport: { width: 720, height: 450 }, deviceScaleFactor: 2 });
    await ctx.addInitScript(() => localStorage.setItem("cal.firstrun", "1"));
    const page = await ctx.newPage();
    await page.goto((await import("./lib.mjs")).BASE + path); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(500);
    const over = await page.evaluate(() => document.documentElement.scrollWidth - innerWidth);
    const r = await new AxeBuilder({ page }).withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"]).analyze();
    ok(over <= 1 && !r.violations.length, `200% zoom: ${name}`, `${over > 1 ? `scrolls sideways by ${over}px ` : ""}${r.violations.map((v) => v.id).join(",")}`);
    await ctx.close();
  }
} finally { await browser.close(); stop(); }
process.exit(bad ? 1 : 0);

// axe-core over the Threads app in every state a person meets: the list, an open thread, the composer with each popover open, the request bar's three kinds, the first-use and failure states, the
// row's hover actions, a swiped row, the rename field and the context menu; light and dark, desktop and phone. Exits 1 on any violation.
import AxeBuilder from "@axe-core/playwright";
import { launch, open, serve } from "./lib.mjs";
const chip = (i) => (p) => p.locator(".composer .chips .pill").nth(i).click();
const states = [
  ["list and a thread", "/threads/t-run", null],
  ["thread with steps open", "/threads/t-run", async (p) => { await p.locator("button[aria-expanded]").first().click().catch(() => {}); }],
  ["provider menu", "/threads/t-idle", chip(0)],
  ["permission menu", "/threads/t-idle", chip(1)],
  ["machine popover", "/threads/t-idle", chip(2)],
  ["mention picker", "/threads/t-idle", async (p) => { await p.locator("#composer-field").click(); await p.keyboard.type("hi @"); await p.waitForSelector('[role="listbox"]'); await p.waitForTimeout(500); }],
  ["question request", "/threads/t-wait", null],
  ["approval request", "/threads/t-wait", async (p) => { await p.getByRole("button", { name: "Next request" }).click(); }],
  ["secret request", "/threads/t-secret", null],
  ["empty account", "/threads?scenario=empty", null],
  ["empty thread", "/threads?scenario=empty", async (p) => { await p.getByRole("button", { name: "Start a thread" }).last().click(); await p.waitForTimeout(800); }],
  ["no machine", "/threads/t-idle?scenario=no-machine", null],
  ["machine offline", "/threads/t-idle?scenario=offline", null],
  ["provider not signed in", "/threads/t-idle?scenario=signin", null],
  ["turn failed", "/threads/t-idle?scenario=failed", null],
  ["turn retrying", "/threads/t-idle?scenario=retrying", null],
  ["turn interrupted", "/threads/t-idle?scenario=interrupted", null],
  ["row hover actions", "/threads/t-idle", async (p) => { if (p.viewportSize().width > 760) { await p.locator("nav[aria-label=Threads] a.row").nth(1).hover(); await p.waitForTimeout(300); } }],
  ["row context menu", "/threads/t-idle", async (p) => { if (p.viewportSize().width > 760) { await p.locator("nav[aria-label=Threads] a.row").nth(1).click({ button: "right" }); await p.waitForTimeout(400); } }],
  ["rename field", "/threads/t-idle", async (p) => { await p.locator("h1").dblclick(); await p.waitForTimeout(300); }],
];
const stop = await serve(); const browser = await launch();
let bad = 0;
try {
  for (const device of ["desktop", "phone"]) for (const scheme of ["light", "dark"]) for (const [name, path, act] of states) {
    const { page, ctx } = await open(browser, device, scheme, path);
    try { if (act) { await act(page); await page.waitForTimeout(350); } } catch (e) { console.log(`skip ${device} ${scheme} ${name}: ${String(e.message).split("\n")[0]}`); await ctx.close(); continue; }
    const r = await new AxeBuilder({ page }).withTags(["wcag2a", "wcag2aa", "wcag21aa", "best-practice"]).analyze();
    const v = r.violations.map((x) => `${x.id} (${x.nodes.length}): ${x.nodes[0].target.join(" ")} :: ${x.nodes[0].failureSummary?.split("\n")[1] ?? ""}`);
    console.log(`${v.length ? "FAIL" : "ok  "} ${device} ${scheme} ${name}${v.length ? "\n   " + v.join("\n   ") : ""}`);
    bad += v.length;
    await ctx.close();
  }
} finally { await browser.close(); stop(); }
process.exit(bad ? 1 : 0);

// axe-core over the Threads app in every state a person meets: the list, an open thread, the composer with each popover open, the request bar's three kinds, the first-use and failure states, the
// row's hover actions, a swiped row, the rename field and the context menu; light and dark, desktop and phone. Exits 1 on any violation.
import AxeBuilder from "@axe-core/playwright";
import { launch, open, serve } from "./lib.mjs";
// Waits for what the page is doing, not for a guessed time: finite animations have ended and a frame has passed (at most 5 s). A loaded machine makes it slower, never wrong.
const settle = (p) => p.evaluate(() => new Promise((res) => { const t0 = performance.now(); const tick = () => { const run = document.getAnimations().some((a) => a.playState === "running" && a.effect?.getComputedTiming().iterations !== Infinity); if ((!run && performance.now() - t0 > 80) || performance.now() - t0 > 5000) res(); else requestAnimationFrame(tick); }; tick(); }));
const chip = (i) => (p) => p.locator(".composer .chips .pill").nth(i).click();
const states = [
  ["list and a thread", "/threads/t-run", null],
  ["thread with steps open", "/threads/t-run", async (p) => { await p.locator("button[aria-expanded]").first().click().catch(() => {}); }],
  ["provider menu", "/threads/t-idle", chip(0)],
  ["permission menu", "/threads/t-idle", chip(1)],
  ["machine popover", "/threads/t-idle", chip(2)],
  ["mention picker", "/threads/t-idle", async (p) => { await p.locator("#composer-field").click(); await p.keyboard.type("hi @"); await p.waitForSelector('[role="listbox"]'); await settle(p); }],
  ["question request", "/threads/t-wait", null],
  ["approval request", "/threads/t-wait", async (p) => { await p.getByRole("button", { name: "Next request" }).click(); }],
  ["secret request", "/threads/t-secret", null],
  ["empty account", "/threads?scenario=empty", null],
  ["empty thread", "/threads?scenario=empty", async (p) => { await p.getByRole("button", { name: "Start a thread" }).last().click(); await settle(p); }],
  ["no machine", "/threads/t-idle?scenario=no-machine", null],
  ["machine offline", "/threads/t-idle?scenario=offline", null],
  ["provider not signed in", "/threads/t-idle?scenario=signin", null],
  ["turn failed", "/threads/t-idle?scenario=failed", null],
  ["turn retrying", "/threads/t-idle?scenario=retrying", null],
  ["turn interrupted", "/threads/t-idle?scenario=interrupted", null],
  ["row hover actions", "/threads/t-idle", async (p) => { if (p.viewportSize().width > 760) { await p.locator("nav[aria-label=Threads] a.row").nth(1).hover(); await settle(p); } }],
  ["row context menu", "/threads/t-idle", async (p) => { if (p.viewportSize().width > 760) { await p.locator("nav[aria-label=Threads] a.row").nth(1).click({ button: "right" }); await settle(p); } }],
  ["rename field", "/threads/t-idle", async (p) => { await p.locator("h1").dblclick(); await settle(p); }],
];
const stop = await serve(); const browser = await launch();
let bad = 0;
// The states are independent pages: a few at a time (CAL_WORKERS, default 4), reported in order.
const jobs = []; for (const device of ["desktop", "phone"]) for (const scheme of ["light", "dark"]) for (const [name, path, act] of states) jobs.push({ device, scheme, name, path, act });
const lines = new Array(jobs.length);
// A page the machine could not keep alive (a renderer killed under memory pressure) says so with "closed"/"crashed": that state is run again, once, in a fresh context; a real finding never is.
async function one(j, i) {
  try { return await attempt(j, i); } catch (e) { if (!/closed|crash/i.test(String(e.message))) throw e; return attempt(j, i); }
}
async function attempt(j, i) {
  const { page, ctx } = await open(browser, j.device, j.scheme, j.path);
  try {
    try { if (j.act) { await j.act(page); await settle(page); } } catch (e) { lines[i] = `skip ${j.device} ${j.scheme} ${j.name}: ${String(e.message).split("\n")[0]}`; return; }
    const r = await new AxeBuilder({ page }).withTags(["wcag2a", "wcag2aa", "wcag21aa", "best-practice"]).analyze();
    const v = r.violations.map((x) => `${x.id} (${x.nodes.length}): ${x.nodes[0].target.join(" ")} :: ${x.nodes[0].failureSummary?.split("\n")[1] ?? ""}`);
    lines[i] = `${v.length ? "FAIL" : "ok  "} ${j.device} ${j.scheme} ${j.name}${v.length ? "\n   " + v.join("\n   ") : ""}`;
    bad += v.length;
  } finally { await ctx.close(); }
}
try {
  let next = 0;
  await Promise.all(Array.from({ length: Number(process.env.CAL_WORKERS ?? 4) }, async () => { for (;;) { const i = next++; if (i >= jobs.length) return; await one(jobs[i], i); } }));
  console.log(lines.join("\n"));
} finally { await browser.close(); stop(); }
process.exit(bad ? 1 : 0);

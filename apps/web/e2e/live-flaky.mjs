// Repeats the live round trip N times with the adapter's own trace on (localStorage cal.debug.threads): new thread, send, wait for the agent's message, archive, undo. A failed iteration prints the
// timeline of journal entries, reads and stream events that led to it. Usage: CAL_TOKEN_FILE=... CAL_SERVER=... node e2e/live-flaky.mjs [N]
import { readFileSync } from "node:fs";
import { chromium } from "playwright";
import { serve, BASE } from "./lib.mjs";
const N = +(process.argv[2] ?? 30);
const WITH_ARCHIVE = process.argv[3] !== "noarchive";
const token = readFileSync(process.env.CAL_TOKEN_FILE, "utf8").trim();
const stop = await serve(); const b = await chromium.launch();
const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1 });
await ctx.addInitScript(([t]) => { try { localStorage.setItem("cal.firstrun", "1"); localStorage.setItem("cal.devtoken", t); localStorage.setItem("cal.server", "1"); localStorage.setItem("cal.debug.threads", "1"); } catch {} }, [token]);
const page = await ctx.newPage();
let log = []; page.on("console", (m) => { const t = m.text(); if (t.startsWith("[threads")) log.push(t); });
let ops = []; page.on("response", async (r) => { if (r.url().includes("/api/v1/operations")) { try { const q = JSON.parse(r.request().postData() ?? "{}"); ops.push(`${q.operation} ${r.status()}`); } catch {} } });
await page.goto(BASE + "/threads?server=1"); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(2500);
const rows = page.locator("nav[aria-label=Threads] a.row");
let fails = 0; const times = [];
for (let i = 1; i <= N; i++) {
  log = []; ops = [];
  try {
  const mark = `${Date.now()}`; const problems = [];
  await page.keyboard.press("Control+k"); await page.getByRole("dialog", { name: "Search and commands" }).waitFor();
  await page.locator(".pal-input").fill("new thread"); await page.waitForTimeout(200);
  await page.locator("[role=option]", { hasText: "New thread" }).first().click();
  await page.waitForURL(/\/threads\/[0-9a-f-]{36}/, { timeout: 15000 }).catch(() => problems.push("no navigation after New thread"));
  await page.locator("#composer-field").waitFor({ timeout: 10000 }).catch(() => problems.push("no composer"));
  const t0 = Date.now();
  await page.locator("#composer-field").click(); await page.locator("#composer-field").fill("Reply with the single word: ready"); await page.keyboard.press("Enter");
  let got = -1; for (let k = 0; k < 400 && got < 0; k++) { if ((await page.locator(".msg.agent").count()) > 0) got = Date.now() - t0; else await page.waitForTimeout(100); }
  if (got < 0) { problems.push("no agent message within 40 s"); log.push(`[script] page at that moment: url ${page.url().slice(-12)}, .msg=${await page.locator(".msg").count()}, roles=${(await page.locator(".msg").evaluateAll((e) => e.map((x) => x.className.replace(/ ?svelte-\S+/, "").replace("msg ", "")))).join(",")}, stop button=${await page.getByRole("button", { name: "Stop the turn" }).count()}, title=${await page.locator("h1").innerText().catch(() => "?")}`); await page.screenshot({ path: `/tmp/flaky-noanswer-${i}.png` }); } else times.push(got);
  if (WITH_ARCHIVE) { // archive the open thread through its row's hover action, then undo
  const cur = rows.first(); const title = await cur.locator(".t").innerText().catch(() => "");
  const before = await rows.count();
  await cur.hover(); await page.getByRole("button", { name: "Archive" }).first().click().catch(() => problems.push("no Archive button"));
  let gone = false; const series = []; for (let k = 0; k < 50 && !gone; k++) { const c = await rows.count(); series.push(c); if (c === before - 1) gone = true; else await page.waitForTimeout(100); }
  if (!gone) log.push(`[script] rows after clicking Archive, every 100 ms: ${series.join(",")}`);
  if (!gone) problems.push(`archive did not remove the row within 5 s (rows ${before} -> ${await rows.count()})`);
  await page.getByRole("button", { name: "Undo" }).click().catch(() => {});
  let back = false; for (let k = 0; k < 50 && !back; k++) { if ((await rows.count()) === before) back = true; else await page.waitForTimeout(100); }
  if (!back) problems.push("undo did not bring the row back within 5 s");
  }
  if (problems.length) { fails++; console.log(`#${i} FAIL: ${problems.join("; ")}\n   ops: ${ops.join(" | ")}\n   timeline:\n${log.slice(-40).map((l) => "     " + l).join("\n")}`); await page.screenshot({ path: `/tmp/flaky-${i}.png` }); }
  else console.log(`#${i} ok  agent message after ${got} ms`);
  } catch (e) { fails++; console.log(`#${i} FAIL (script): ${String(e.message).split("\n")[0]}\n   timeline:\n${log.slice(-25).map((l) => "     " + l).join("\n")}`); await page.keyboard.press("Escape").catch(() => {}); await page.goto(BASE + "/threads?server=1").catch(() => {}); await page.waitForSelector("html[data-ready]").catch(() => {}); await page.waitForTimeout(2000); }
}
console.log(`\n${N - fails} of ${N} ok; agent message time: median ${[...times].sort((a, c) => a - c)[Math.floor(times.length / 2)]} ms, max ${Math.max(...times)} ms`);
await ctx.close(); await b.close(); stop(); process.exit(fails ? 1 : 0);

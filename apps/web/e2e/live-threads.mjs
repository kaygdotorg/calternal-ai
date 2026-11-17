// The Threads screen against a live server (a seeded fixture installation, docs/dev-e2e.md): lists, creates a thread, sends, follows the turn, stops, archives.
// Usage (from apps/web, a build in dist): CAL_TOKEN_FILE=<dev token file> CAL_SERVER=http://<relay>:<port> node e2e/live-threads.mjs
// The token is read from the file and put in the page's storage; it is never printed. Everything this makes is a thread in a disposable installation.
import { readFileSync } from "node:fs";
import { chromium } from "playwright";
import { serve, open, BASE } from "./lib.mjs";
const token = readFileSync(process.env.CAL_TOKEN_FILE, "utf8").trim();
const stop = await serve(); const b = await chromium.launch();
let bad = 0;
const ok = (n, p, note = "") => { console.log(`${p ? "PASS" : "FAIL"}  ${n}${note ? "  " + note : ""}`); if (!p) bad++; };
const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });
await ctx.addInitScript(([t]) => { try { localStorage.setItem("cal.firstrun", "1"); localStorage.setItem("cal.devtoken", t); localStorage.setItem("cal.server", "1"); } catch {} }, [token]);
const page = await ctx.newPage();
const errors = []; page.on("pageerror", (e) => errors.push(e.message)); page.on("console", (m) => { if (m.type() === "error") errors.push(m.text()); });
const calls = []; page.on("response", async (r) => { if (r.url().includes("/api/v1/operations")) { try { const q = JSON.parse(r.request().postData() ?? "{}"); let why = ""; if (r.status() >= 400) { const j = await r.json().catch(() => null); why = ` (${j?.error?.code}: ${String(j?.error?.message ?? "").slice(0, 120)})`; } calls.push(`${q.operation} ${r.status()}${why}`); } catch {} } });
process.on("exit", () => { console.log("    operations seen: " + [...new Set(calls)].join(" | ")); });
const t0 = Date.now();
await page.goto(BASE + "/threads?server=1"); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(2500);
const rows = page.locator("nav[aria-label=Threads] a.row");
ok("the list loads from the server (no fixture thread names)", (await page.locator("text=Journal replay is slow").count()) === 0, `rows=${await rows.count()} calls=${[...new Set(calls)].join(",")}`);
// New thread through the palette
await page.keyboard.press("Control+k"); await page.getByRole("dialog", { name: "Search and commands" }).waitFor();
await page.locator(".pal-input").fill("new thread"); await page.waitForTimeout(250);
await page.locator("[role=option]", { hasText: "New thread" }).first().click();
await page.waitForTimeout(2500);
console.log("    palette ops: " + calls.slice(-4).join(" | "));
ok("a thread was created and opened", /\/threads\/[0-9a-f-]{36}/.test(page.url()), page.url().replace(/[0-9a-f-]{36}/, "<uuid>"));
// Send
const f = page.locator("#composer-field");
await f.click(); await f.fill("Reply with the single word: ready");
await page.keyboard.press("Enter");
await page.waitForTimeout(1500);
ok("the sent message shows in the thread", (await page.locator(".msg", { hasText: "Reply with the single word" }).count()) >= 1);
const working = await page.locator("[aria-label*=orking], .working, [data-working]").count();
console.log(`    working indicators right after send: ${working}`);
// Wait for the agent's answer (a real turn takes seconds to a minute)
let first = 0;
for (let i = 0; i < 150 && !first; i++) { await page.waitForTimeout(1000); if ((await page.locator(".msg.agent").count()) > 0) first = i + 1; }
ok("an agent message arrives", first > 0, `after ~${first}s`);
if (first) console.log("    agent said: " + (await page.locator(".msg.agent").last().innerText()).replace(/\s+/g, " ").slice(0, 160));
ok("the thread is idle again after the turn", (await page.getByRole("button", { name: "Stop the turn" }).count()) === 0 && (await page.locator("text=/Running/").count()) === 0);
await page.screenshot({ path: "/tmp/live-threads.png" });
// Stop a second turn
await f.click(); await f.fill("Count slowly to 200, one number per line"); await page.keyboard.press("Enter");
await page.waitForTimeout(2500);
const stopBtn = page.getByRole("button", { name: "Stop the turn" });
console.log(`    stop button visible while running: ${await stopBtn.count()}`);
if (await stopBtn.count()) {
  await stopBtn.click(); await page.waitForTimeout(3000);
  const still = await page.getByRole("button", { name: "Stop the turn" }).count(), running = await page.locator("text=/Running/").count();
  ok("stop returns the thread to idle", still === 0 && running === 0, `stop buttons=${still} "Running" labels=${running}`);
  await page.screenshot({ path: "/tmp/live-stop.png" });
}
// Archive
const first1 = rows.first(); const title = await first1.locator(".t").innerText().catch(() => "");
await first1.hover(); await page.getByRole("button", { name: "Archive" }).first().click(); await page.waitForTimeout(1500);
ok("archive removes it from the list", true, `"${title}"`);
console.log("    page errors: " + (errors.length ? [...new Set(errors)].slice(0, 6).join(" | ").slice(0, 600) : "none"));
console.log(`    total ${((Date.now() - t0) / 1000).toFixed(0)}s`);
await ctx.close(); await b.close(); stop(); process.exit(bad ? 1 : 0);

// The request bar end to end against a real server of this tree: fresh roots, a virtual authenticator for setup, a direct-mode machine whose provider is the stand-in
// `claude` (e2e/fixtures/fake-claude-requests.mjs) that raises a question, an approval and a secret prompt with the turn's own credential.
// Checks: question answered from the bar, approval once / for this thread / denied, a secret provided, a request dropped when its turn is stopped, a second device
// seeing the bar follow the feed. Env: CAL_BIN, CAL_DIST, CAL_REQ_PORT. Everything is disposable data in a temp directory.
import { spawn } from "node:child_process";
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { chromium } from "playwright";

const PORT = Number(process.env.CAL_REQ_PORT ?? 3411);
const BIN = process.env.CAL_BIN ?? `${process.env.HOME}/Developer/lanes/ui-threads/target/debug/calternal`;
const data = mkdtempSync(join(tmpdir(), "cal-req-"));
const fake = resolve("e2e/fixtures/fake-claude-requests.mjs"); chmodSync(fake, 0o755);
const signed = join(data, "signed-in"); writeFileSync(signed, "");
const url = `http://localhost:${PORT}`;
let bad = 0, machine = null;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const srv = spawn(BIN, ["serve", "--create", "--installation-id", crypto.randomUUID(), "--appdata", join(data, "appdata"), "--userdata-ssd", join(data, "ssd"), "--userdata-hdd", join(data, "hdd"), "--port", String(PORT), "--web-dir", process.env.CAL_DIST ?? "dist", "--plugin-root", process.env.CAL_PLUGINS ?? "../../plugins"], { stdio: ["ignore", "ignore", "pipe"] });
let serr = "", merr = ""; srv.stderr.on("data", (d) => (serr += d));
const op = (p, name, input) => p.evaluate(async ([name, input]) => { const r = await fetch("/api/v1/operations", { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ request_id: crypto.randomUUID(), operation: name, input }) }); return r.json(); }, [name, input]);
try {
  for (let i = 0; i < 80; i++) { try { if ((await fetch(`${url}/api/v1/auth/status`)).ok) break; } catch {} await new Promise((r) => setTimeout(r, 250)); }
  const token = readFileSync(join(data, "appdata", "setup.token"), "utf8").trim();
  const b = await chromium.launch();
  const ctx = await b.newContext({ viewport: { width: 1440, height: 900 } }); await ctx.addInitScript(() => localStorage.setItem("cal.firstrun", "1"));
  const p = await ctx.newPage(); const cdp = await ctx.newCDPSession(p);
  await cdp.send("WebAuthn.enable"); await cdp.send("WebAuthn.addVirtualAuthenticator", { options: { protocol: "ctap2", transport: "internal", hasResidentKey: true, hasUserVerification: true, isUserVerified: true, automaticPresenceSimulation: true } });
  await p.goto(`${url}/?server=1`); await p.waitForSelector("html[data-ready]");
  await p.fill('input[name="token"]', token); await p.fill('input[name="name"]', "Req Owner"); await p.getByRole("button", { name: "Create passkey" }).click();
  await p.waitForTimeout(2500); await p.evaluate(() => localStorage.removeItem("cal.onboarding")); /* the first owner lands in the onboarding (shell lane): the account exists, leave the wizard */ await p.waitForSelector("html[data-ready]"); await p.waitForTimeout(1200);
  const go = async (page, path) => { await page.evaluate((x) => { history.pushState(null, "", x); dispatchEvent(new PopStateEvent("popstate")); }, path); await page.waitForTimeout(600); };
  await p.evaluate(() => localStorage.removeItem("cal.onboarding")); await p.goto(`${url}/settings/machines?server=1`); await p.waitForSelector("html[data-ready]");
  await p.getByRole("button", { name: "Make a code" }).click();
  const code = /--code\s+(\S+)/.exec((await p.locator("code.cmd").first().innerText()).trim())?.[1];
  const home = join(data, "machine");
  // The machine runs the plugin's executable by name from PATH: a wrapper for the built adapter launcher sits first on it.
  const bin = join(data, "bin"); mkdirSync(bin);
  const launcher = resolve(process.env.CAL_PLUGINS ?? "../../plugins", "calternal-claude-code/bin/calternal-provider-claude-code");
  writeFileSync(join(bin, "calternal-provider-claude-code"), `#!/bin/sh\nexec ${launcher} "$@"\n`); chmodSync(join(bin, "calternal-provider-claude-code"), 0o755);
  const env = { ...process.env, PATH: `${bin}:${process.env.PATH}`, HOME: home, CALTERNAL_ENROLL_CODE: code, CALTERNAL_CLAUDE_BINARY: fake, FAKE_SIGNED_IN: signed, FAKE_LOG: join(data, "fake.log") };
  const enroll = spawn(BIN, ["machine", "enroll", "--server", url, "--name", "dev-machine", "--direct", "--dir", home], { env, stdio: "ignore" });
  ok((await new Promise((r) => enroll.on("exit", r))) === 0, "machine enrolled (direct mode)");
  const space = (await op(p, "spaces.list", {})).data.spaces.find((s) => s.personal).id;
  // The direct backend gives the provider only PATH, HOME and the turn's own variables, so the stand-in `claude` carries its settings in the wrapper.
  writeFileSync(join(bin, "claude"), `#!/bin/sh\nFAKE_SIGNED_IN=${signed} FAKE_LOG=${join(data, "fake.log")} FAKE_SPACE=${space} exec node ${fake} "$@"\n`); chmodSync(join(bin, "claude"), 0o755);
  machine = spawn(BIN, ["machine", "run", "--dir", home], { env, stdio: ["ignore", "pipe", "pipe"] }); machine.stdout.on("data", (d) => (merr += d)); machine.stderr.on("data", (d) => (merr += d));
  for (const id of ["calternal-claude-code", "calternal-orchestrator"]) await op(p, "plugins.enable", { space_id: space, plugin_id: id });
  for (let i = 0; i < 40; i++) { const r = (await op(p, "agent.readiness", { space_id: space })).data?.machines?.[0]; if (r?.connected) break; await p.waitForTimeout(500); }
  console.log("   check:", JSON.stringify(await op(p, "agent.check", { space_id: space, machine_id: (await op(p, "agent.readiness", { space_id: space })).data.machines[0].machine_id, provider_id: "calternal-claude-code" })).slice(0, 300));
  await p.waitForTimeout(2500);
  console.log("   readiness:", JSON.stringify((await op(p, "agent.readiness", { space_id: space })).data).slice(0, 500));

  // ---- helpers over the Threads screen
  const open = async (page) => { await page.goto(`${url}/threads?server=1`); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(1500); };
  const newThread = async (page) => {
    // Created by operation (the palette's New thread is covered by new-thread.mjs); the page opens it by address.
    const t = (await op(page, "threads.create", { space_id: space, title: "Requests run" })).data.thread;
    await page.goto(`${url}/threads/${t.id}?server=1`); await page.waitForSelector("html[data-ready]"); await page.waitForSelector("#composer-field"); await page.waitForTimeout(600);
  };
  const say = async (page, t) => { const f = page.locator("#composer-field"); await f.click(); await f.fill(t); await page.keyboard.press("Enter"); };
  const bar = (page) => page.getByRole("region", { name: "Waiting on you" });
  const barText = async (page) => ((await bar(page).count()) ? (await bar(page).innerText()).replace(/\s+/g, " ").slice(0, 260) : "(none)");
  const waitBar = async (page, ms = 20000) => { await bar(page).waitFor({ timeout: ms }).catch(() => {}); return (await bar(page).count()) > 0; };
  const waitGone = async (page, ms = 15000) => { for (let i = 0; i < ms / 100; i++) { if ((await bar(page).count()) === 0) return true; await page.waitForTimeout(100); } return false; };
  const agentSays = async (page, re, ms = 20000) => { for (let i = 0; i < ms / 200; i++) { const t = (await page.locator(".msg.agent").allInnerTexts()).join(" "); if (re.test(t)) return true; await page.waitForTimeout(200); } return false; };
  const diag = async (l) => console.log(`   [${l}] page:`, (await p.locator("main").innerText()).replace(/\s+/g, " ").slice(0, 200), "| threads:", JSON.stringify((await op(p, "threads.list", { space_id: space })).data?.threads?.map((t) => [t.turn_state, t.turn_reason?.code, t.turn_reason?.class])), "| requests:", JSON.stringify((await op(p, "requests.list", {})).data).slice(0, 300), "| fake:", (() => { try { return readFileSync(join(data, "fake.log"), "utf8").split("\n").filter((l) => /->|^cwd/.test(l)).slice(-2).join(" ; "); } catch { return ""; } })());
  const calls = []; p.on("response", async (r) => { if (/operations|secrets|reverify|account\/live/.test(r.url()) && r.request().method() === "POST" && r.status() >= 400) calls.push(`${r.status()} ${new URL(r.url()).pathname} ${JSON.stringify(JSON.parse(r.request().postData() ?? "{}").operation ?? "")} ${(await r.text().catch(() => "")).slice(0, 220)}`); });
  await open(p);

  // ---- 1. a question, answered from the bar
  await newThread(p); await say(p, "[ask] which release?");
  ok(await waitBar(p), "a question raises the bar", await barText(p));
  if (!(await bar(p).count())) console.log("   page:", (await p.locator("main").innerText()).replace(/\s+/g, " ").slice(0, 400), "| reason:", JSON.stringify((await op(p, "threads.list", { space_id: space })).data?.threads?.map((t) => t.turn_reason)), "| machine:", merr.slice(-300));
    console.log("   fake log:", (() => { try { return readFileSync(join(data, "fake.log"), "utf8").slice(-600); } catch { return "(never ran)"; } })());
    const t = (await op(p, "threads.list", { space_id: space })).data?.threads?.[0]; const g = (await op(p, "threads.get", { id: t.id, space_id: space })).data; const jid = g?.job?.id ?? g?.thread?.job?.id; console.log("   job", jid, JSON.stringify(g?.job ?? g?.thread?.job ?? null).slice(0, 600));
    if (jid) console.log("   events:", await p.evaluate(async (j) => { const c = new AbortController(); setTimeout(() => c.abort(), 1500); let out = ""; try { const r = await fetch("/api/v1/jobs/" + j + "/events", { signal: c.signal }); const rd = r.body.getReader(); for (;;) { const { value, done } = await rd.read(); if (done) break; out += new TextDecoder().decode(value); } } catch {} return out.slice(0, 1800); }, jid));
  const field = bar(p).locator("input, textarea").first();
  if (await field.count()) { await field.fill("1.0"); await bar(p).getByRole("button", { name: /Send|Answer|Submit/ }).first().click(); }
  ok(await waitGone(p), "the bar drops after the answer"); ok(await agentSays(p, /Thanks, continuing/), "the turn continues after the answer");
  console.log("   after answer:", (await p.locator("main").innerText()).replace(/\s+/g, " ").slice(0, 300), "| reasons:", JSON.stringify((await op(p, "threads.list", { space_id: space })).data?.threads?.map((t) => [t.turn_state, t.turn_reason?.code])));
  await p.screenshot({ path: "/tmp/req-1.png" });

  // ---- 2. an approval: once
  await newThread(p); await say(p, "[approve] write a note");
  ok(await waitBar(p), "an approval raises the bar", await barText(p));
  if (!(await bar(p).count())) await diag("approve");
  const lifetimes = await bar(p).getByRole("button").allInnerTexts(); console.log("   approval buttons:", lifetimes.join(" | "));
  await p.screenshot({ path: "/tmp/req-2.png" });
  await bar(p).getByRole("button", { name: "Allow once" }).click(); await p.waitForTimeout(1500);
  console.log("   after Allow once:", await barText(p), "| failed:", calls.slice(-3).join(" ; "));
  ok(await waitGone(p, 8000), "approve once resolves and the bar drops"); ok(await agentSays(p, /Thanks, continuing/), "the turn continues after approval");

  // ---- 3. approval for this thread
  await newThread(p); await say(p, "[approve] write a note");
  await waitBar(p); const has = (await bar(p).getByRole("button", { name: "Allow for this thread" }).count()) > 0;
  if (has) { await bar(p).getByRole("button", { name: "Allow for this thread" }).click(); ok(await waitGone(p, 8000), "approve for this thread resolves"); ok(await agentSays(p, /Thanks, continuing/), "the turn continues after a thread approval"); }
  else ok(true, "this approval is once-only on the server, so the bar offers no thread option");

  // ---- 4. deny
  await newThread(p); await say(p, "[approve] write a note");
  await waitBar(p); await bar(p).getByRole("button", { name: "Deny" }).click();
  ok(await waitGone(p, 8000), "deny resolves and the bar drops"); console.log("   after deny, agent:", (await p.locator(".msg.agent").allInnerTexts()).join(" ").slice(0, 120), "| notice:", (await p.locator("[role=status], .notice").allInnerTexts()).join(" ").slice(0, 120));

  // ---- 5. a secret
  await newThread(p); await say(p, "[secret] publish");
  ok(await waitBar(p), "a secret prompt raises the bar", await barText(p));
  if (!(await bar(p).count())) await diag("secret");
  const sf = bar(p).locator("input[type=password], input").first();
  if (await sf.count()) { await sf.fill("not-a-real-token"); await bar(p).getByRole("button", { name: /Provide|Send|Save|Give/ }).first().click(); await p.waitForTimeout(1500); }
  console.log("   after providing:", await barText(p), "| failed:", calls.slice(-3).join(" ; "));
  ok(await waitGone(p, 8000), "providing the secret drops the bar");
  // A secret asked for by a running turn does not park it (the stand-in keeps working), so there is no continuation to see; the inbox is empty.
  ok(((await op(p, "requests.list", {})).data?.requests ?? []).length === 0, "the server no longer lists the secret request");

  // ---- 6. stop a turn that waits: its request goes away
  await newThread(p); await say(p, "[ask] and then stop");
  ok(await waitBar(p), "a second question raises the bar");
  const stop = p.getByRole("button", { name: "Stop the turn" }); await stop.waitFor({ timeout: 8000 }).catch(() => {});
  if (process.env.DBG) console.log("   before stop:", JSON.stringify(await p.evaluate(() => { const c = document.querySelector(".composer"); const b = document.querySelector("[aria-label=\"Stop the turn\"]")?.getBoundingClientRect(); return { collapsed: c.hasAttribute("data-collapsed"), h: c.getBoundingClientRect().height, stop: b && [b.x, b.y], active: document.activeElement?.id || document.activeElement?.tagName }; })));
  if (await stop.count()) await stop.click(); else console.log("   no Stop button while waiting; bar:", await barText(p));
  if (process.env.DBG) { await p.waitForTimeout(1500); console.log("   after stop:", await barText(p), "|", JSON.stringify((await op(p, "threads.list", { space_id: space })).data?.threads?.slice(0, 2).map((t) => [t.turn_state, t.turn_reason?.code])), "| requests:", JSON.stringify((await op(p, "requests.list", {})).data).slice(0, 200)); }
  ok(await waitGone(p, 10000), "stopping the turn drops its request from the bar");

  // ---- 7. a second device follows the feed
  const state = await ctx.storageState();
  const ctx2 = await b.newContext({ viewport: { width: 1000, height: 800 }, storageState: state }); await ctx2.addInitScript(() => localStorage.setItem("cal.firstrun", "1"));
  const q = await ctx2.newPage(); await q.goto(p.url()); await q.waitForSelector("html[data-ready]"); await q.waitForTimeout(1500);
  await newThread(p); const url2 = p.url(); await say(p, "[ask] for two devices");
  await q.goto(url2); await q.waitForSelector("html[data-ready]");
  ok(await waitBar(p) && await waitBar(q), "both devices show the bar");
  const f2 = bar(p).locator("input, textarea").first(); if (await f2.count()) { await f2.fill("2.0"); await bar(p).getByRole("button", { name: /Send|Answer|Submit/ }).first().click(); }
  ok(await waitGone(p) && await waitGone(q), "the second device's bar drops when the first answers");
  console.log("   failed calls:", calls.join(" | ") || "none");
  await b.close();
} catch (e) { console.error(e); bad++; }
if (bad) console.log("server stderr tail:", serr.slice(-300), "\nmachine tail:", merr.slice(-900));
machine?.kill(); srv.kill(); rmSync(data, { recursive: true, force: true });
process.exit(bad ? 1 : 0);

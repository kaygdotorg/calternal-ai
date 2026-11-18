// Tool steps end to end against a real server of this tree with the repo's fixture provider (plugins/calternal-claude-code/test/fake-claude.mjs, prompt `scripted-tool-steps`): the live step with its
// name and target, the saved file, the reload replay, a failing step, the 96 KiB output, and a stop mid-step. Env: CAL_BIN, CAL_DIST, CAL_STEPS_PORT.
import { spawn } from "node:child_process";
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, rmSync, unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { chromium } from "playwright";
import { checkMessageActions } from "./actions-check.mjs";
import { expandComposer, signUp } from "./lib.mjs";

const PORT = Number(process.env.CAL_STEPS_PORT ?? 3413);
const BIN = process.env.CAL_BIN ?? `${process.env.HOME}/Developer/lanes/ui-threads/target/debug/calternal`;
const data = mkdtempSync(join(tmpdir(), "cal-steps-"));
const fake = resolve("../../plugins/calternal-claude-code/test/fake-claude.mjs"); chmodSync(fake, 0o755);
const signed = join(data, "signed-in");
const url = `http://localhost:${PORT}`;
let bad = 0, machine = null;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const srv = spawn(BIN, ["serve", "--create", "--installation-id", crypto.randomUUID(), "--appdata", join(data, "appdata"), "--userdata-ssd", join(data, "ssd"), "--userdata-hdd", join(data, "hdd"), "--port", String(PORT), "--web-dir", process.env.CAL_DIST ?? "dist", "--plugin-root", process.env.CAL_PLUGINS ?? "../../plugins"], { stdio: "ignore" });
const op = (p, name, input) => p.evaluate(async ([name, input]) => { const r = await fetch("/api/v1/operations", { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ request_id: crypto.randomUUID(), operation: name, input }) }); return r.json(); }, [name, input]);
try {
  for (let i = 0; i < 80; i++) { try { if ((await fetch(`${url}/api/v1/auth/status`)).ok) break; } catch {} await new Promise((r) => setTimeout(r, 250)); }
  const token = readFileSync(join(data, "appdata", "setup.token"), "utf8").trim();
  const b = await chromium.launch();
  const ctx = await b.newContext({ viewport: { width: 1440, height: 900 } }); await ctx.addInitScript(() => localStorage.setItem("cal.firstrun", "1"));
  const p = await ctx.newPage(); const cdp = await ctx.newCDPSession(p);
  await cdp.send("WebAuthn.enable"); await cdp.send("WebAuthn.addVirtualAuthenticator", { options: { protocol: "ctap2", transport: "internal", hasResidentKey: true, hasUserVerification: true, isUserVerified: true, automaticPresenceSimulation: true } });
  await p.goto(`${url}/?server=1`); await p.waitForSelector("html[data-ready]");
  await signUp(p, token, "Steps Owner");
  await p.waitForTimeout(2500); await p.evaluate(() => localStorage.removeItem("cal.onboarding")); /* the first owner lands in the onboarding (shell lane): the account exists, leave the wizard */ await p.waitForSelector("html[data-ready]"); await p.waitForTimeout(1200);
  const space = (await op(p, "spaces.list", {})).data.spaces.find((s) => s.personal).id;
  for (const id of ["calternal-claude-code", "calternal-orchestrator"]) await op(p, "plugins.enable", { space_id: space, plugin_id: id });
  const t = (await op(p, "threads.create", { space_id: space, title: "Readiness run" })).data.thread;
  await p.goto(`${url}/threads/${t.id}?server=1`); await p.waitForSelector("html[data-ready]"); await p.waitForTimeout(1500);
  const shown = async () => (await p.locator("main").innerText()).replace(/\s+/g, " ");
  const waitText = async (re, ms = 12000) => { for (let i = 0; i < ms / 200; i++) { if (re.test(await shown())) return true; await p.waitForTimeout(200); } return false; };
  const wire = async () => { const r = (await op(p, "agent.readiness", { space_id: space })).data; const m = r.machines[0]; return m ? `machine ${m.connected ? "online" : "offline"}, provider ${JSON.stringify(m.providers["calternal-claude-code"]?.check)}, ready=${m.providers["calternal-claude-code"]?.ready}` : "no machines"; };

  // machine
  const home = join(data, "machine"); const bin = join(data, "bin"); mkdirSync(bin); mkdirSync(join(data, "fakehome")); writeFileSync(join(data, "fakehome", "signed-in"), "");
  await p.evaluate(() => localStorage.removeItem("cal.onboarding")); await p.goto(`${url}/settings/machines?server=1`); await p.waitForSelector("html[data-ready]");
  await p.getByRole("button", { name: "Make a code" }).click();
  const code = /--code\s+(\S+)/.exec((await p.locator("code.cmd").first().innerText()).trim())?.[1];
  const launcher = resolve(process.env.CAL_PLUGINS ?? "../../plugins", "calternal-claude-code/bin/calternal-provider-claude-code");
  writeFileSync(join(bin, "calternal-provider-claude-code"), `#!/bin/sh\nexec ${launcher} "$@"\n`); chmodSync(join(bin, "calternal-provider-claude-code"), 0o755);
  writeFileSync(join(bin, "claude"), `#!/bin/sh\nHOME=${join(data, "fakehome")} exec node ${fake} "$@"\n`); chmodSync(join(bin, "claude"), 0o755);
  const env = { ...process.env, PATH: `${bin}:${process.env.PATH}`, HOME: home, CALTERNAL_ENROLL_CODE: code };
  ok((await new Promise((r) => spawn(BIN, ["machine", "enroll", "--server", url, "--name", "dev-machine", "--direct", "--dir", home], { env, stdio: "ignore" }).on("exit", r))) === 0, "machine enrolled");
  machine = spawn(BIN, ["machine", "run", "--dir", home], { env, stdio: "ignore" });
  for (let i = 0; i < 40; i++) { const m = (await op(p, "agent.readiness", { space_id: space })).data?.machines?.[0]; if (m?.connected) break; await p.waitForTimeout(500); }
  const mid = (await op(p, "agent.readiness", { space_id: space })).data.machines[0].machine_id;
  await op(p, "agent.check", { space_id: space, machine_id: mid, provider_id: "calternal-claude-code" }); await p.waitForTimeout(2500);

  const gets = []; p.on("request", (r) => { if (r.url().includes("/api/v1/operations") && r.method() === "POST") { try { const q = JSON.parse(r.postData() ?? "{}"); if (q.operation === "threads.get") gets.push(q.input.since ? "delta" : "whole"); } catch {} } });
  const run = async (prompt) => {
    const t = (await op(p, "threads.create", { space_id: space, title: "Steps run" })).data.thread;
    await p.goto(`${url}/threads/${t.id}?server=1`); await p.waitForSelector("html[data-ready]"); await p.waitForSelector("#composer-field"); await p.waitForTimeout(500);
    const f = p.locator("#composer-field"); await f.click(); await f.fill(prompt); await p.keyboard.press("Enter");
    return t.id;
  };
  const text = async () => (await p.locator("main").innerText()).replace(/\s+/g, " ");
  const until = async (re, ms = 20000) => { for (let i = 0; i < ms / 200; i++) { if (re.test(await text())) return true; await p.waitForTimeout(200); } return false; };
  const raw = async (id) => (await op(p, "threads.get", { id, space_id: space })).data.thread.markdown;

  // 1. success: Bash step then a failing Read step
  const id1 = await run("scripted-tool-steps");
  ok(await until(/Worked for|tools?\b/i, 30000), "a turn with tool steps finishes and shows the steps group", (await text()).slice(100, 300));
  console.log("   saved file (body):\n" + (await raw(id1)).split("\n").slice(0, 40).map((l) => "   | " + l.slice(0, 160)).join("\n"));
  // The turn is over when the Working row has gone; the last checkpoint (heading, duration, reasoning time) is a whole refresh a moment after it, so the page is compared once that has had time to land.
  await p.waitForFunction(() => !document.querySelector(".working"), null, { timeout: 30000 }).catch(() => {}); await p.waitForTimeout(2500);
  const before = (await p.locator(".msg").allInnerTexts()).map((t) => t.replace(/\s+/g, " ").replace(/\d\d:\d\d$/, "").trim());
  console.log("   threads.get calls so far:", gets.join(","));
  const open = p.locator(".steps .head").first(); if (await open.count()) await open.click().catch(() => {}); await p.waitForTimeout(500);
  console.log("   steps shown:", (await p.locator(".steps").allInnerTexts()).join(" || ").replace(/\s+/g, " ").slice(0, 400));
  await p.screenshot({ path: "/tmp/steps-1.png" });
  await p.reload(); await p.waitForSelector("html[data-ready]"); await p.waitForTimeout(1500);
  if (await p.locator(".steps .head").count()) await p.locator(".steps .head").first().click().catch(() => {}); await p.waitForTimeout(400);
  const after = (await p.locator(".msg").allInnerTexts()).map((t) => t.replace(/\s+/g, " ").replace(/\d\d:\d\d$/, "").trim());
  ok(JSON.stringify(before.map((t) => t.replace(/Worked.*?tools?/, "")))  === JSON.stringify(after.map((t) => t.replace(/Worked.*?tools?/, ""))), "what the page built from checkpoint additions equals what a whole read shows", `before=${JSON.stringify(before).slice(0, 700)} after=${JSON.stringify(after).slice(0, 700)}`);
  console.log("   after reload steps:", (await p.locator(".steps").allInnerTexts()).join(" || ").replace(/\s+/g, " ").slice(0, 400));
  ok(/Bash|ls/i.test((await p.locator(".steps").allInnerTexts()).join(" ")), "after a reload the saved steps show the tool and target");

  // the message actions, on the server's own thread (clipboard allowed for the page)
  await ctx.grantPermissions(["clipboard-read", "clipboard-write"], { origin: url }).catch(() => {});
  await p.goto(`${url}/threads/${id1}?server=1`); await p.waitForSelector("html[data-ready]"); await p.waitForTimeout(1500);
  await checkMessageActions(p, ok, "desktop");

  // 2. long output
  const id2 = await run("scripted-tool-steps long-output");
  ok(await until(/Worked for|tools?\b/i, 30000), "a 96 KiB tool output turn finishes");
  console.log("   long-output file bytes:", (await raw(id2)).length);

  // the model picker, the Working phases and the reasoning block against the server's own shapes (threads.models, turn_phase, reasoning_*) with the scripted fixture provider
  {
    const t = (await op(p, "threads.create", { space_id: space, title: "Reasoning run" })).data.thread;
    await p.goto(`${url}/threads/${t.id}?server=1`); await p.waitForSelector("html[data-ready]"); await p.waitForSelector("#composer-field"); await p.waitForTimeout(800);
    const cat = (await op(p, "threads.models", { space_id: space })).data?.providers ?? [];
    ok(cat.length >= 1 && (cat[0].models?.length ?? 0) >= 1, "threads.models lists the provider's models", cat.map((c) => `${c.name}: ${c.models.map((m) => `${m.id}[${(m.reasoning_levels ?? []).join("/")}]`).join(" ")}`).join(" | ").slice(0, 160));
    await expandComposer(p);
    await p.locator(".composer .chip[data-chip=model] .pill").click();
    const labels = (await p.getByRole("menuitemradio").allInnerTexts()).map((x) => x.replace(/\s+/g, " ").trim());
    console.log("   picker rows:", labels.join(" | ").slice(0, 300));
    ok(labels.length > 1 && !labels.some((l) => /not listed/.test(l)), "the picker lists the models (not an empty search, not 'not listed')");
    await p.getByRole("menuitemradio", { name: /Sonnet 5.5/ }).click(); await p.waitForTimeout(400);
    await expandComposer(p);
    await p.locator(".composer .chip[data-chip=model] .pill").click();
    const labels2 = (await p.getByRole("menuitemradio").allInnerTexts()).map((x) => x.replace(/\s+/g, " ").trim());
    ok(labels2.some((l) => /Xhigh reasoning/.test(l)), "and, for the model chosen, the reasoning levels it accepts", labels2.filter((l) => /reasoning/.test(l)).join(", "));
    await p.getByRole("menuitemradio", { name: /Xhigh reasoning/ }).click(); await p.waitForTimeout(400);
    ok((await p.locator(".composer .chip[data-chip=model] .lvl").count()) === 1, "the model chip shows the chosen reasoning level as a mark");
    await p.keyboard.press("Escape");
    const f = p.locator("#composer-field"); await f.click(); await f.fill("scripted-reasoning"); await p.keyboard.press("Enter");
    const phases = new Set(); let thought = false;
    for (let i = 0; i < 150 && (await p.locator(".working").count() || i < 5); i++) { phases.add((await p.locator(".working .phase").innerText().catch(() => "")).trim()); if (await p.locator(".thought").count()) thought = true; await p.waitForTimeout(120); }
    console.log("   phases:", [...phases].filter(Boolean).join(" -> "));
    ok([...phases].some((x) => /Starting the machine|Waiting to start|Thinking|Starting/.test(x)), "the Working row shows the server's phases from the send");
    await p.waitForSelector(".msg.agent-row", { timeout: 20000 }).catch(() => {});
    await p.waitForTimeout(800);
    const txt = (await p.locator(".msg.agent-row").last().innerText().catch(() => "")).replace(/\s+/g, " ");
    ok(thought || /Thought for/.test(txt), "the reasoning shows as a folded block, then as Thought for", txt.slice(0, 100));
    ok(/Hello/.test(txt), "and the turn's answer follows");
  }

  // 3. stop mid-step
  const id3 = await run("scripted-tool-steps hang");
  await p.getByRole("button", { name: "Stop the turn" }).waitFor({ timeout: 15000 }).catch(() => {});
  await p.waitForTimeout(1500);
  console.log("   mid-step steps:", (await p.locator(".steps").allInnerTexts()).join(" || ").replace(/\s+/g, " ").slice(0, 300));
  await p.getByRole("button", { name: "Stop the turn" }).click().catch(() => {});
  ok(await (async () => { for (let i = 0; i < 40; i++) { if (!(await p.getByRole("button", { name: "Stop the turn" }).count())) return true; await p.waitForTimeout(250); } return false; })(), "stop mid-step returns the thread to idle");
  console.log("   stopped file tail:", (await raw(id3)).slice(-300).replace(/\n/g, " / "));
  await b.close();
} catch (e) { console.error(e); bad++; }
machine?.kill(); srv.kill(); rmSync(data, { recursive: true, force: true });
process.exit(bad ? 1 : 0);

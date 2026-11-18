// Turn readiness against a real server of this tree, each state made true in turn: no machine enrolled, a machine that is stopped, a provider signed out (stand-in `claude`
// reports it), then Check again after signing in; also what `agent.readiness` says against what the thread shows. Same setup as requests-server.mjs. Env: CAL_BIN, CAL_DIST, CAL_READY_PORT.
import { spawn } from "node:child_process";
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, rmSync, unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { chromium } from "playwright";

const PORT = Number(process.env.CAL_READY_PORT ?? 3412);
const BIN = process.env.CAL_BIN ?? `${process.env.HOME}/Developer/lanes/ui-threads/target/debug/calternal`;
const data = mkdtempSync(join(tmpdir(), "cal-ready-"));
const fake = resolve("e2e/fixtures/fake-claude-requests.mjs"); chmodSync(fake, 0o755);
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
  await p.fill('input[name="token"]', token); await p.fill('input[name="name"]', "Ready Owner"); await p.getByRole("button", { name: "Create passkey" }).click();
  await p.waitForTimeout(2500); await p.evaluate(() => localStorage.removeItem("cal.onboarding")); /* the first owner lands in the onboarding (shell lane): the account exists, leave the wizard */ await p.waitForSelector("html[data-ready]"); await p.waitForTimeout(1200);
  const space = (await op(p, "spaces.list", {})).data.spaces.find((s) => s.personal).id;
  for (const id of ["calternal-claude-code", "calternal-orchestrator"]) await op(p, "plugins.enable", { space_id: space, plugin_id: id });
  const t = (await op(p, "threads.create", { space_id: space, title: "Readiness run" })).data.thread;
  await p.goto(`${url}/threads/${t.id}?server=1`); await p.waitForSelector("html[data-ready]"); await p.waitForTimeout(1500);
  const shown = async () => (await p.locator("main").innerText()).replace(/\s+/g, " ");
  const waitText = async (re, ms = 12000) => { for (let i = 0; i < ms / 200; i++) { if (re.test(await shown())) return true; await p.waitForTimeout(200); } return false; };
  const wire = async () => { const r = (await op(p, "agent.readiness", { space_id: space })).data; const m = r.machines[0]; return m ? `machine ${m.connected ? "online" : "offline"}, provider ${JSON.stringify(m.providers["calternal-claude-code"]?.check)}, ready=${m.providers["calternal-claude-code"]?.ready}` : "no machines"; };

  // 1. no machine
  console.log("   wire:", await wire());
  ok(await waitText(/no machine|not.*machine|Add a machine|set up a machine/i), "no machine enrolled: the thread says so", (await shown()).slice(150, 400));
  await p.screenshot({ path: "/tmp/ready-1-none.png" });

  // 2. enroll one, signed in: nothing extra
  writeFileSync(signed, "");
  await p.evaluate(() => localStorage.removeItem("cal.onboarding")); await p.goto(`${url}/settings/machines?server=1`); await p.waitForSelector("html[data-ready]");
  await p.getByRole("button", { name: "Make a code" }).click();
  const code = /--code\s+(\S+)/.exec((await p.locator("code.cmd").first().innerText()).trim())?.[1];
  const home = join(data, "machine"); const bin = join(data, "bin"); mkdirSync(bin);
  const launcher = resolve(process.env.CAL_PLUGINS ?? "../../plugins", "calternal-claude-code/bin/calternal-provider-claude-code");
  writeFileSync(join(bin, "calternal-provider-claude-code"), `#!/bin/sh\nexec ${launcher} "$@"\n`); chmodSync(join(bin, "calternal-provider-claude-code"), 0o755);
  writeFileSync(join(bin, "claude"), `#!/bin/sh\nFAKE_SIGNED_IN=${signed} FAKE_SPACE=${space} exec node ${fake} "$@"\n`); chmodSync(join(bin, "claude"), 0o755);
  const env = { ...process.env, PATH: `${bin}:${process.env.PATH}`, HOME: home, CALTERNAL_ENROLL_CODE: code };
  ok((await new Promise((r) => spawn(BIN, ["machine", "enroll", "--server", url, "--name", "dev-machine", "--direct", "--dir", home], { env, stdio: "ignore" }).on("exit", r))) === 0, "machine enrolled");
  const run = () => spawn(BIN, ["machine", "run", "--dir", home], { env, stdio: "ignore" });
  machine = run();
  await p.goto(`${url}/threads/${t.id}?server=1`); await p.waitForSelector("html[data-ready]"); await p.waitForTimeout(2500);
  const mid = (await op(p, "agent.readiness", { space_id: space })).data.machines[0].machine_id;
  const check = () => op(p, "agent.check", { space_id: space, machine_id: mid, provider_id: "calternal-claude-code" });
  await check(); await p.waitForTimeout(2500);
  console.log("   wire:", await wire());
  ok(!/no machine|offline|sign in|signed out/i.test(await shown()), "machine online and provider signed in: no notice", (await shown()).slice(150, 330));
  await p.screenshot({ path: "/tmp/ready-2-ready.png" });

  // 3. provider signed out
  unlinkSync(signed); await check();
  ok(await waitText(/sign in|signed out|not signed in/i, 15000), "provider signed out: the thread shows the sign-in notice and command", (await shown()).slice(150, 420));
  console.log("   wire:", await wire());
  await p.screenshot({ path: "/tmp/ready-3-signedout.png" });

  // 4. fix it, Check again
  writeFileSync(signed, "");
  const again = p.getByRole("button", { name: /Check again/ }).first();
  if (await again.count()) { await again.click(); ok(await (async () => { for (let i = 0; i < 60; i++) { if (!/sign in|signed out|not signed in/i.test(await shown())) return true; await p.waitForTimeout(250); } return false; })(), "Check again clears the sign-in notice after signing in"); }
  else { ok(false, "the sign-in notice offers Check again"); }
  console.log("   wire:", await wire());

  // 5. machine stopped
  machine.kill(); await p.waitForTimeout(500);
  ok(await waitText(/offline|isn.t connected|not connected/i, 30000), "machine stopped: the thread says it is offline", (await shown()).slice(150, 400));
  console.log("   wire:", await wire());
  await p.screenshot({ path: "/tmp/ready-4-offline.png" });
  machine = run();
  ok(!(await (async () => { for (let i = 0; i < 100; i++) { if (!/offline/i.test(await shown())) return false; await p.waitForTimeout(300); } return true; })()), "machine back: the offline notice goes away", (await shown()).slice(150, 330));
  await b.close();
} catch (e) { console.error(e); bad++; }
machine?.kill(); srv.kill(); rmSync(data, { recursive: true, force: true });
process.exit(bad ? 1 : 0);

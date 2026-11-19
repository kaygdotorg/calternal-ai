import { launch, openRealServerPage, signUpRealServerOwner, startRealServer } from "./lib.mjs";
// Machines and provider readiness against a real server (this tree's build) with a real enrolled machine: make a code in the page, enroll and run `calternal machine` (direct execution), then the Machines page shows the machine
// and the providers' readiness from `agent.readiness`; Check again runs `agent.check` and the account feed keeps the page current. Env: CAL_BIN, CAL_DIST, CAL_MACH_PORT.
import { spawn } from "node:child_process";
import { join } from "node:path";

const PORT = Number(process.env.CAL_MACH_PORT ?? 3399);
let server, browser;
let bad = 0, machine = null;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
try {
  server = await startRealServer(PORT);
  const { bin: BIN, data, url } = server;
  browser = await launch();
  const p = await openRealServerPage(browser);
  await signUpRealServerOwner(p, server, "Mach Owner");
  const go = async (path) => { await p.evaluate((x) => { history.pushState(null, "", x); dispatchEvent(new PopStateEvent("popstate")); }, path); await p.waitForTimeout(600); };
  await go("/settings/machines");
  await p.getByRole("button", { name: "Make a code" }).click();
  const cmd = (await p.locator("code.cmd").first().innerText()).trim();
  const code = /--code\s+(\S+)/.exec(cmd)?.[1];
  ok(!!code, "a one-use enrollment code is made");
  const home = join(data, "machine");
  const env = { ...process.env, HOME: home, CALTERNAL_ENROLL_CODE: code };
  const enroll = spawn(BIN, ["machine", "enroll", "--server", url, "--name", "dev-machine", "--direct", "--dir", home], { env, stdio: ["ignore", "pipe", "pipe"] });
  let out = ""; enroll.stdout.on("data", (d) => (out += d)); enroll.stderr.on("data", (d) => (out += d));
  const code1 = await new Promise((r) => enroll.on("exit", r));
  ok(code1 === 0, "the machine enrolled", code1 === 0 ? "" : out.slice(-200));
  machine = spawn(BIN, ["machine", "run", "--dir", home], { env, stdio: "ignore" });
  // The provider is enabled in the space (Settings > Space plugins): readiness lists providers that are.
  const sp = await p.evaluate(async () => (await (await fetch("/api/v1/operations", { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ request_id: crypto.randomUUID(), operation: "spaces.list", input: {} }) })).json()).data.spaces[0].id);
  const enable = (id) => p.evaluate(async ([sp, id]) => (await (await fetch("/api/v1/operations", { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ request_id: crypto.randomUUID(), operation: "plugins.enable", input: { space_id: sp, plugin_id: id } }) })).json()).ok, [sp, id]);
  const plugins = await p.evaluate(async (sp) => (await (await fetch("/api/v1/operations", { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ request_id: crypto.randomUUID(), operation: "plugins.list", input: { space_id: sp } }) })).json()).data.plugins.map((x) => `${x.manifest.id}:${x.enabled}`), sp);
  console.log("   plugins:", plugins.join(" "));
  for (const id of ["calternal-claude-code", "calternal-orchestrator"]) console.log("   enable", id, await enable(id));
  await go("/settings/account"); await go("/settings/machines");
  await p.getByText("dev-machine").first().waitFor({ timeout: 15000 });
  ok(true, "the enrolled machine is listed");
  await p.getByText(/Online|Offline|Not taking/).first().waitFor({ timeout: 15000 });
  const rows = await p.locator('[data-setting^="machines."][data-setting*="."] ').count();
  const providers = await p.locator("[data-setting] .t").allInnerTexts();
  console.log("   page text:", providers.slice(0, 8).join(" | "));
  console.log("   readiness:", JSON.stringify(await p.evaluate(async () => { const sp = JSON.parse(localStorage.getItem("cal.boot") ?? "{}").spaces?.[0]?.id; const r = await fetch("/api/v1/operations", { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ request_id: crypto.randomUUID(), operation: "agent.readiness", input: { space_id: sp } }) }); return await r.json(); })).slice(0, 900));
  const check = p.getByRole("button", { name: "Check again" }).first();
  ok(await check.isVisible().catch(() => false), "a provider row with Check again is shown for the machine");
  if (await check.isVisible().catch(() => false)) {
    await check.click(); await p.waitForTimeout(4000);
    const text = await p.locator(".sp").first().innerText();
    console.log("   after Check again:", text.replace(/\n+/g, " | ").slice(0, 300));
    ok(/Signed in|Not signed in|Not installed|Not checked|Not ready|Off in this space/.test(text), "the provider's state is said in words after Check again");
  }
} catch (e) { console.error(e); bad++; }
finally {
  machine?.kill();
  try { await browser?.close(); } finally { await server?.stop(); }
}
process.exit(bad ? 1 : 0);

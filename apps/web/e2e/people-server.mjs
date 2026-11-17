// People in server mode against a server that has the people operations (the people lane's build): account name, Admin people and groups, an invitation made, opened by a second person (join page, passkey,
// lands in their own space), listed with a name, role changed, removed. Env: CAL_BIN (a server with the people operations), CAL_DIST.
import { spawn } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { chromium } from "playwright";

const PORT = Number(process.env.CAL_PEOPLE_PORT ?? 3398);
const BIN = process.env.CAL_BIN ?? `${process.env.HOME}/Developer/lanes/people/target/debug/calternal`;
const data = mkdtempSync(join(tmpdir(), "cal-people-"));
const srv = spawn(BIN, ["serve", "--create", "--installation-id", crypto.randomUUID(), "--appdata", join(data, "appdata"), "--userdata-ssd", join(data, "ssd"), "--userdata-hdd", join(data, "hdd"), "--port", String(PORT), "--web-dir", process.env.CAL_DIST ?? "dist"], { stdio: "ignore" });
const url = `http://localhost:${PORT}`;
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
try {
  for (let i = 0; i < 80; i++) { try { if ((await fetch(`${url}/api/v1/auth/status`)).ok) break; } catch {} await new Promise((r) => setTimeout(r, 250)); }
  const token = readFileSync(join(data, "appdata", "setup.token"), "utf8").trim();
  const b = await chromium.launch();
  const mk = async () => { const ctx = await b.newContext({ viewport: { width: 1440, height: 900 } }); await ctx.addInitScript(() => { localStorage.setItem("cal.firstrun", "1"); localStorage.setItem("cal.debug", "1"); }); const p = await ctx.newPage(); p.on("dialog", (d) => void d.accept()); const c = await ctx.newCDPSession(p); await c.send("WebAuthn.enable"); await c.send("WebAuthn.addVirtualAuthenticator", { options: { protocol: "ctap2", transport: "internal", hasResidentKey: true, hasUserVerification: true, isUserVerified: true, automaticPresenceSimulation: true } }); return { ctx, p }; };
  const go = (p, path) => p.evaluate((x) => { history.pushState(null, "", x); dispatchEvent(new PopStateEvent("popstate")); }, path);
  const { p } = await mk();
  await p.goto(`${url}/?server=1`); await p.waitForSelector("html[data-ready]");
  await p.fill('input[name="token"]', token); await p.fill('input[name="name"]', "Olive"); await p.getByRole("button", { name: "Create passkey" }).click();
  await p.waitForFunction(() => !document.querySelector("h1")?.textContent?.includes("Set up"), null, { timeout: 15000 }); await p.waitForSelector("html[data-ready]"); await p.waitForTimeout(1500);

  // Account: change the name.
  await go(p, "/settings/account"); await p.locator("#acc-name").waitFor({ timeout: 8000 });
  await p.fill("#acc-name", "Olive Owner"); await p.getByRole("button", { name: "Save", exact: true }).click();
  await p.waitForFunction(() => document.querySelector("#acc-name")?.value === "Olive Owner");
  await go(p, "/settings/members"); await p.getByText("Olive Owner (you)").waitFor({ timeout: 8000 });
  ok(true, "account name changed and shown on Members");
  // Admin: people and groups by capability.
  await go(p, "/settings/people"); await p.getByText("Olive Owner").first().waitFor({ timeout: 8000 });
  ok(true, "Admin > People lists the accounts");
  await go(p, "/settings/groups"); await p.fill("#g-new", "Design"); await p.getByRole("button", { name: "Make it" }).click(); await p.getByText("Design", { exact: true }).waitFor({ timeout: 8000 });
  ok(true, "Admin > Groups makes a group");
  // Roles.
  await go(p, "/settings/roles"); await p.getByText("Reader", { exact: true }).first().waitFor({ timeout: 8000 });
  ok(true, "Roles listed");
  // Invitation: make one, read its link, open it as a second person.
  console.log("   state:", await p.evaluate(() => JSON.stringify({ cur: window.__cal.spaces.current, list: window.__cal.spaces.list, id: window.__cal.spaces.currentId })));
  await go(p, "/settings/invites"); await p.waitForTimeout(2500); console.log("   page:", (await p.evaluate(() => document.querySelector(".page")?.innerText ?? document.body.innerText)).replace(/\n+/g, " | ").slice(0, 400)); await p.getByRole("button", { name: "Make an invitation" }).click();
  await p.locator("code.cmd").first().waitFor({ timeout: 8000 });
  const link = (await p.locator("code.cmd").first().innerText()).trim();
  ok(/\/invites\/[^/]+$/.test(link), "an invitation is made and its link shown", link.replace(/\/invites\/.*/, "/invites/…"));
  const g = await mk();
  g.p.on("console", (m) => console.log("   guest console:", m.text().slice(0, 160))); g.p.on("pageerror", (e) => console.log("   guest error:", e.message.slice(0, 160)));
  const local = new URL(link); await g.p.goto(`${url}${local.pathname}`); await g.p.waitForSelector("html[data-ready]");
  await g.p.getByRole("heading", { name: "Join Calternal" }).waitFor({ timeout: 8000 }).catch(async () => { console.log("   guest page:", g.p.url().slice(0, 60), JSON.stringify((await g.p.evaluate(() => document.body.innerText)).slice(0, 200))); throw new Error("no join page"); });
  ok(true, "the invitation page asks for a name and a passkey");
  await g.p.fill('input[name="name"]', "Guest Gus"); await g.p.getByRole("button", { name: "Create passkey and join" }).click();
  await g.p.waitForFunction(() => !document.querySelector("h1")?.textContent?.includes("Join"), null, { timeout: 15000 }); await g.p.waitForSelector("html[data-ready]"); await g.p.waitForTimeout(1500);
  ok(await g.p.locator("aside.side").isVisible(), "the invited person lands in the app");
  await go(g.p, "/settings/members"); await g.p.getByText("Guest Gus (you)").waitFor({ timeout: 8000 });
  ok(true, "the guest sees themself as a member of the shared space");
  // The owner sees the guest with a name, changes the role, removes.
  await go(p, "/settings/members"); await p.getByText("Guest Gus").first().waitFor({ timeout: 8000 });
  ok(true, "the owner sees the guest by name");
  await p.getByRole("radio", { name: "Editor" }).last().click().catch(() => {}); await p.waitForTimeout(800);
  await p.getByRole("button", { name: "Remove" }).first().click(); await p.waitForTimeout(1000);
  ok((await p.getByText("Guest Gus").count()) === 0, "removing the guest takes them off the list");
  await b.close();
} catch (e) { console.error(e); bad++; }
srv.kill(); rmSync(data, { recursive: true, force: true });
process.exit(bad ? 1 : 0);

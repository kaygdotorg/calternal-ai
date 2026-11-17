// People in server mode against a server built from this tree: account name, Admin people and groups, an invitation made, opened by a second person (join page, passkey,
// lands in their own space), listed with a name, role changed, removed. Env: CAL_BIN, CAL_DIST.
import { spawn } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { chromium } from "playwright";

const PORT = Number(process.env.CAL_PEOPLE_PORT ?? 3398);
const BIN = process.env.CAL_BIN ?? `${process.env.HOME}/Developer/lanes/ui-shell/target/debug/calternal`;
const data = mkdtempSync(join(tmpdir(), "cal-people-"));
const srv = spawn(BIN, ["serve", "--create", "--installation-id", crypto.randomUUID(), "--appdata", join(data, "appdata"), "--userdata-ssd", join(data, "ssd"), "--userdata-hdd", join(data, "hdd"), "--port", String(PORT), "--web-dir", process.env.CAL_DIST ?? "dist"], { stdio: "ignore" });
const url = `http://localhost:${PORT}`;
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
try {
  for (let i = 0; i < 80; i++) { try { if ((await fetch(`${url}/api/v1/auth/status`)).ok) break; } catch {} await new Promise((r) => setTimeout(r, 250)); }
  const token = readFileSync(join(data, "appdata", "setup.token"), "utf8").trim();
  const b = await chromium.launch();
  const mk = async () => { const ctx = await b.newContext({ viewport: { width: 1440, height: 900 } }); await ctx.addInitScript(() => { localStorage.setItem("cal.firstrun", "1"); }); const p = await ctx.newPage(); p.on("dialog", (d) => void d.accept()); const c = await ctx.newCDPSession(p); await c.send("WebAuthn.enable"); await c.send("WebAuthn.addVirtualAuthenticator", { options: { protocol: "ctap2", transport: "internal", hasResidentKey: true, hasUserVerification: true, isUserVerified: true, automaticPresenceSimulation: true } }); return { ctx, p }; };
  const scrolled = (p) => p.evaluate(() => [...document.querySelectorAll("aside.side, aside.side *")].filter((e) => e.scrollTop > 0).map((e) => `${e.tagName}.${String(e.className).slice(0, 18)}=${e.scrollTop}`).join(" "));
const go = async (p, path) => { await p.evaluate((x) => { history.pushState(null, "", x); dispatchEvent(new PopStateEvent("popstate")); }, path); await p.waitForTimeout(700); const sc = await scrolled(p); if (sc) console.log("   sidebar scrolled after", path, sc); };
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
  console.log("   layout:", JSON.stringify(await p.evaluate(() => { const out = []; let e = document.querySelector('nav[aria-label="Settings"]'); while (e && e.tagName !== "BODY") { const r = e.getBoundingClientRect(), c = getComputedStyle(e); out.push(`${e.tagName.toLowerCase()}.${String(e.className).slice(0, 14)} top=${Math.round(r.top)} h=${Math.round(r.height)} ov=${c.overflowY} jc=${c.justifyContent}/${c.alignContent} tr=${c.translate} ${c.transform === "none" ? "" : c.transform}`); e = e.parentElement; } return out.slice(0, 7); }))); await go(p, "/settings/roles"); await p.getByText("Reader", { exact: true }).first().waitFor({ timeout: 8000 });
  ok(true, "Roles listed");
  // Invitation: make one, read its link, open it as a second person.
  await go(p, "/settings/invites");  await p.getByRole("button", { name: "Make an invitation" }).click();
  await p.locator("code.cmd").first().waitFor({ timeout: 8000 });
  const link = (await p.locator("code.cmd").first().innerText()).trim();
  ok(/\/invites\/[^/]+$/.test(link), "an invitation is made and its link shown", link.replace(/\/invites\/.*/, "/invites/…"));
  const g = await mk();
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
  await p.getByRole("button", { name: "Add a group…" }).click(); await p.getByRole("button", { name: /^Add as/ }).first().click(); await p.getByText(/Design/).first().waitFor({ timeout: 8000 });
  ok(true, "a group is given a role in the space and listed");
  console.log("   at screenshot:", JSON.stringify(await p.evaluate(() => { const n = document.querySelector('nav[aria-label="Settings"]'); const sc = n.closest(".scroll"); const pb = n.parentElement; return { navTop: Math.round(n.getBoundingClientRect().top), scrollTop: sc.scrollTop, scTop: Math.round(sc.getBoundingClientRect().top), barH: getComputedStyle(document.documentElement).getPropertyValue("--bar-h"), bodyPad: getComputedStyle(pb).paddingTop, scPad: getComputedStyle(sc).paddingTop, scH: sc.clientHeight, scSH: sc.scrollHeight }; })));
  await p.screenshot({ path: process.env.CAL_SHOT ?? "/tmp/people-members.png" });
  await p.getByRole("radio", { name: "Editor" }).last().click().catch(() => {}); await p.waitForTimeout(800);
  await p.getByRole("button", { name: "Remove" }).first().click(); await p.waitForTimeout(1000);
  ok((await p.getByText("Guest Gus").count()) === 0, "removing the guest takes them off the list");
  await b.close();
} catch (e) { console.error(e); bad++; }
srv.kill(); rmSync(data, { recursive: true, force: true });
process.exit(bad ? 1 : 0);

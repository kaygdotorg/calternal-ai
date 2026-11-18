import { signUp } from "./lib.mjs";
// Skills and Automations against a real server: create a skill (hand-held, reviewed), create an automation from words with the server's reviewed schedule, preview, Run now, Pause, Resume, Remove.
// Records what the server does and fails on what the page gets wrong. Env: CAL_BIN, CAL_DIST, CAL_SK_PORT. Frames go to $CAL_SHOTS (default /tmp/calternal-skills-shots).
import { spawn } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { chromium } from "playwright";

const PORT = Number(process.env.CAL_SK_PORT ?? 3398);
const BIN = process.env.CAL_BIN ?? `${process.env.HOME}/Developer/lanes/ui-shell/target/debug/calternal`;
const SHOTS = process.env.CAL_SHOTS ?? "/tmp/calternal-skills-shots"; mkdirSync(SHOTS, { recursive: true });
const data = mkdtempSync(join(tmpdir(), "cal-sk-"));
const dist = process.env.CAL_DIST ?? "dist";
const srv = spawn(BIN, ["serve", "--create", "--installation-id", crypto.randomUUID(), "--appdata", join(data, "appdata"), "--userdata-ssd", join(data, "ssd"), "--userdata-hdd", join(data, "hdd"), "--port", String(PORT), "--web-dir", dist], { stdio: ["ignore", "pipe", "pipe"] });
let log = ""; srv.stdout.on("data", (d) => (log += d)); srv.stderr.on("data", (d) => (log += d));
const url = `http://localhost:${PORT}`;
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
try {
  for (let i = 0; i < 80; i++) { try { if ((await fetch(`${url}/api/v1/auth/status`)).ok) break; } catch {} await new Promise((r) => setTimeout(r, 250)); }
  const token = readFileSync(join(data, "appdata", "setup.token"), "utf8").trim();
  const b = await chromium.launch();
  const ctx = await b.newContext({ viewport: { width: 1440, height: 900 } }); await ctx.addInitScript(() => localStorage.setItem("cal.firstrun", "1"));
  const p = await ctx.newPage(); const cdp = await ctx.newCDPSession(p);
  await cdp.send("WebAuthn.enable"); await cdp.send("WebAuthn.addVirtualAuthenticator", { options: { protocol: "ctap2", transport: "internal", hasResidentKey: true, hasUserVerification: true, isUserVerified: true, automaticPresenceSimulation: true } });
  const calls = []; p.on("response", async (r) => { if (r.url().includes("/api/v1/") && r.request().method() === "POST" && !r.url().includes("/auth/")) calls.push(`${r.status()} ${r.url().split("/api/v1/")[1]} ${JSON.stringify(r.request().postDataJSON?.() ?? "").slice(0, 90)}`); });
  p.on("pageerror", (e) => { console.log("   pageerror:", String(e).slice(0, 200)); });
  await p.goto(`${url}/?server=1`); await p.waitForSelector("html[data-ready]");
  await signUp(p, token, "Skills Owner");
  await p.waitForFunction(() => !document.querySelector("h1")?.textContent?.includes("Set up"), null, { timeout: 15000 }); await p.waitForSelector("html[data-ready]"); await p.waitForTimeout(1200);
  const go = async (path) => { await p.evaluate((x) => { history.pushState(null, "", x); dispatchEvent(new PopStateEvent("popstate")); }, path); await p.waitForTimeout(900); };
  const alert = async () => (await p.getByRole("alert").allInnerTexts()).join(" | ");
  const shot = (n) => p.screenshot({ path: `${SHOTS}/${n}.png` });
  // Skills
  await go("/settings/skills"); await shot("skills-empty");
  ok(await p.getByRole("button", { name: "New skill" }).isVisible().catch(() => false), "Skills page loads with New skill", await alert());
  await p.getByRole("button", { name: "New skill" }).click();
  await p.getByLabel("What is the skill for? (a few words)").fill("Write Release Notes");
  await p.getByRole("button", { name: "Next" }).click();
  await p.getByLabel("Description").fill("Writes release notes from merged changes. Use when preparing a release."); await p.getByRole("button", { name: "Next" }).click();
  await p.getByRole("button", { name: "Next" }).click(); await p.getByRole("button", { name: "Claude" }).click(); await p.getByRole("button", { name: "Next" }).click();
  await shot("skill-review");
  await p.getByRole("button", { name: "Create and open" }).click(); await p.waitForTimeout(2500);
  ok(/\/notes\//.test(p.url()), "Create and open: the skill is made and opens in Notes", `${p.url()} ${await alert()}`);
  await go("/settings/skills"); await shot("skills-after");
  ok((await p.getByText("write-release-notes").count()) > 0, "the new skill is in the list");
  // Remove the skill
  const rm = p.getByRole("button", { name: /Remove/ }).first();
  if (await rm.isVisible().catch(() => false)) { p.once("dialog", (d) => d.accept()); await rm.click(); await p.waitForTimeout(1500); ok((await p.getByText("write-release-notes").count()) === 0, "Remove takes the skill out of the list", await alert()); }
  else ok(false, "a Remove button on the skill row");
  // Automations
  await go("/settings/automations"); await shot("auto-empty");
  await p.getByRole("button", { name: "New automation" }).click();
  await p.getByLabel("Automation name").fill("Friday wrap-up"); await p.getByRole("button", { name: "Next" }).click();
  await p.getByLabel("When").fill("every 2 weeks at 9"); await p.waitForTimeout(900); await p.getByRole("button", { name: "Next" }).click();
  console.log("   odd schedule says:", (await alert()).slice(0, 160));
  await p.getByLabel("When").fill("every friday at 5pm"); await p.waitForTimeout(1500);
  const read = await p.locator(".wz .h[aria-live]").innerText().catch(() => "");
  ok(/Friday/.test(read) && /17:00|5/.test(read), "the schedule is read back (server preview_schedule)", read.replace(/\n/g, " ").slice(0, 160));
  await p.getByRole("button", { name: "Next" }).click(); await p.getByLabel("Prompt").fill("Summarize the week."); await p.getByRole("button", { name: "Next" }).click();
  await shot("auto-review");
  console.log("   review text:", (await p.locator("pre.cmd").innerText().catch(() => "")).replace(/\n/g, " | ").slice(0, 240));
  await p.getByRole("button", { name: "Create and open" }).click(); await p.waitForTimeout(2500);
  console.log("   after create:", p.url(), await alert());
  await go("/settings/automations"); await shot("auto-after");
  ok((await p.getByText("Friday wrap-up").count()) > 0, "the automation is listed", await alert());
  const body = await p.locator("main, [role=tabpanel]").first().innerText().catch(() => "");
  console.log("   row text:", body.replace(/\n+/g, " | ").slice(0, 300));
  await p.getByRole("button", { name: "Run now" }).first().click(); await p.waitForTimeout(3000);
  console.log("   after Run now:", p.url(), await alert()); await shot("auto-run");
  await go("/settings/automations");
  await p.getByRole("button", { name: "Pause" }).first().click(); await p.waitForTimeout(1500);
  ok((await p.getByRole("button", { name: "Resume" }).count()) > 0, "Pause turns into Resume", await alert());
  await p.getByRole("button", { name: "Resume" }).first().click(); await p.waitForTimeout(1200);
  const rem = p.getByRole("button", { name: /Remove/ }).first();
  if (await rem.isVisible().catch(() => false)) { p.once("dialog", (d) => d.accept()); await rem.click(); await p.waitForTimeout(1500); ok((await p.getByText("Friday wrap-up").count()) === 0, "Remove takes the automation out of the list", await alert()); }
  else ok(false, "a Remove button on the automation row");
  console.log(calls.join("\n"));
  await b.close();
} catch (e) { console.error(e); bad++; }
srv.kill(); console.log("server log tail:", log.split("\n").slice(-4).join(" | ").slice(0, 300)); rmSync(data, { recursive: true, force: true });
process.exit(bad ? 1 : 0);

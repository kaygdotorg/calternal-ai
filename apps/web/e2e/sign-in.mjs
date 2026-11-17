// Passkey sign-in end to end: a real `calternal serve` on an empty installation (no --development, so it writes a setup token), the built app served by it,
// Chromium with a CDP virtual authenticator (WebAuthn.addVirtualAuthenticator, resident key, user verified). First owner setup, reload (session), sign out, sign in again.
// Env: CAL_BIN (server binary), CAL_DIST (built app, base /), CAL_SIGNIN_PORT.
import { spawn } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { chromium } from "playwright";

const PORT = Number(process.env.CAL_SIGNIN_PORT ?? 3391);
const BIN = process.env.CAL_BIN ?? `${process.env.HOME}/Developer/lanes/ui-shell/target/debug/calternal`;
const DIST = process.env.CAL_DIST ?? "dist";
const data = mkdtempSync(join(tmpdir(), "cal-signin-"));
const srv = spawn(BIN, ["serve", "--data-dir", data, "--port", String(PORT), "--web-dir", DIST], { stdio: ["ignore", "ignore", "pipe"] });
let log = ""; srv.stderr.on("data", (d) => { log += d; });
const stop = () => { srv.kill(); rmSync(data, { recursive: true, force: true }); };
let failed = 0;
const check = (ok, what) => { console.log(`${ok ? "ok  " : "FAIL"} ${what}`); if (!ok) failed++; };
try {
  const url = `http://localhost:${PORT}`;
  for (let i = 0; i < 80; i++) { try { if ((await fetch(`${url}/api/v1/auth/status`)).ok) break; } catch { /* not yet */ } await new Promise((r) => setTimeout(r, 250)); }
  const setupToken = (() => { for (let i = 0; i < 40; i++) { try { return readFileSync(join(data, "setup.token"), "utf8").trim(); } catch { /* not yet */ } } return ""; })()
    || (() => { try { return readFileSync(join(data, "appdata", "setup.token"), "utf8").trim(); } catch { return ""; } })();
  check(!!setupToken, "fresh installation wrote a setup token");

  const browser = await chromium.launch();
  const ctx = await browser.newContext({ viewport: { width: 1200, height: 800 } });
  const page = await ctx.newPage();
  page.on("pageerror", (e) => console.log("pageerror", e.message, (e.stack ?? "").split("\n").slice(0, 4).join(" | "))); page.on("response", (r) => r.status() === 404 && r.request().postData() && console.log("404", r.url(), r.request().postData()?.slice(0, 160))); page.on("console", (m) => m.type() === "error" && console.log("console", m.text()));
  const cdp = await ctx.newCDPSession(page);
  await cdp.send("WebAuthn.enable");
  await cdp.send("WebAuthn.addVirtualAuthenticator", { options: { protocol: "ctap2", transport: "internal", hasResidentKey: true, hasUserVerification: true, isUserVerified: true, automaticPresenceSimulation: true } });
  await page.addInitScript(() => localStorage.setItem("cal.firstrun", "1"));

  await page.goto(`${url}/?server=1`);
  await page.waitForSelector("html[data-ready]");
  check(await page.getByRole("heading", { name: "Set up Calternal" }).isVisible(), "empty installation shows first-owner setup");
  await page.fill('input[name="token"]', setupToken);
  await page.fill('input[name="name"]', "Test Owner");
  await page.getByRole("button", { name: "Create passkey" }).click();
  await page.waitForSelector("[data-app], aside, nav", { timeout: 15000 }).catch(() => {});
  await page.waitForLoadState("networkidle");
  let st = await (await ctx.request.get(`${url}/api/v1/auth/status`)).json();
  check(st.signed_in === true && st.setup === false, `setup signs in (status ${JSON.stringify(st)})`);
  check(!(await page.getByRole("heading", { name: "Set up Calternal" }).isVisible().catch(() => false)), "app shown after setup");

  await page.goto(`${url}/`); await page.waitForSelector("html[data-ready]");
  check(!(await page.getByRole("heading", { name: "Sign in" }).isVisible().catch(() => false)), "session survives a reload");

  await page.goto(`${url}/`); await page.waitForSelector("html[data-ready]");
  await page.evaluate(() => { history.pushState(null, "", "/settings/account"); dispatchEvent(new PopStateEvent("popstate")); });
  await page.getByRole("button", { name: /Sign out/ }).first().click();
  await page.getByRole("heading", { name: "Sign in" }).waitFor({ timeout: 10000 });
  st = await (await ctx.request.get(`${url}/api/v1/auth/status`)).json();
  check(st.signed_in === false, "sign out ends the session");

  await page.getByRole("button", { name: "Sign in with a passkey" }).click();
  await page.waitForSelector("html[data-ready]");
  await page.waitForFunction(() => !document.querySelector("h1")?.textContent?.includes("Sign in"), null, { timeout: 15000 }).catch(() => {});
  st = await (await ctx.request.get(`${url}/api/v1/auth/status`)).json();
  check(st.signed_in === true, "signing in again with the passkey works");
  await browser.close();
} catch (e) { console.error(e); console.error(log.slice(-1500)); failed++; }
stop();
process.exit(failed ? 1 : 0);

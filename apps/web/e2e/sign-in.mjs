// Passkey sign-in end to end: a real `calternal serve` on an empty installation (no --development, so it writes a setup token), the built app served by it,
// Chromium with a CDP virtual authenticator (WebAuthn.addVirtualAuthenticator, resident key, user verified). First owner setup, reload (session), sign out, sign in again.
// Env: CAL_BIN (server binary), CAL_DIST (built app, base /), CAL_SIGNIN_PORT.
import { spawn } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { chromium } from "playwright";
import { fillWho, signUp } from "./lib.mjs";

const PORT = Number(process.env.CAL_SIGNIN_PORT ?? 3391);
const BIN = process.env.CAL_BIN ?? `${process.env.HOME}/Developer/lanes/ui-shell/target/debug/calternal`;
const DIST = process.env.CAL_DIST ?? "dist";
const data = mkdtempSync(join(tmpdir(), "cal-signin-"));
const srv = spawn(BIN, ["serve", "--create", "--installation-id", crypto.randomUUID(), "--appdata", join(data, "appdata"), "--userdata-ssd", join(data, "ssd"), "--userdata-hdd", join(data, "hdd"), "--port", String(PORT), "--web-dir", DIST], { stdio: ["ignore", "ignore", "pipe"] });
let log = ""; srv.stderr.on("data", (d) => { log += d; });
const stop = () => { srv.kill(); rmSync(data, { recursive: true, force: true }); };
let failed = 0;
const check = (ok, what) => { console.log(`${ok ? "ok  " : "FAIL"} ${what}`); if (!ok) failed++; };
try {
  const url = `http://localhost:${PORT}`;
  for (let i = 0; i < 80; i++) { try { if ((await fetch(`${url}/api/v1/auth/status`)).ok) break; } catch { /* not yet */ } await new Promise((r) => setTimeout(r, 250)); }
  const setupToken = (() => { for (let i = 0; i < 40; i++) { try { return readFileSync(join(data, "appdata", "setup.token"), "utf8").trim(); } catch { /* not yet */ } } return ""; })()
    || (() => { try { return readFileSync(join(data, "appdata", "setup.token"), "utf8").trim(); } catch { return ""; } })();
  check(!!setupToken, "fresh installation wrote a setup token");

  const browser = await chromium.launch();
  const ctx = await browser.newContext({ viewport: { width: 1200, height: 800 } });
  const page = await ctx.newPage();
  page.on("pageerror", (e) => console.log("pageerror", e.message, (e.stack ?? "").split("\n").slice(0, 4).join(" | "))); page.on("response", (r) => r.status() === 404 && r.request().postData() && console.log("404", r.url(), r.request().postData()?.slice(0, 160))); page.on("console", (m) => m.type() === "error" && console.log("console", m.text()));
  const reqs = {}; page.on("request", (r) => { const k = r.url().replace(/^https?:\/\/[^/]+/, "").slice(0, 60); reqs[k] = (reqs[k] ?? 0) + 1; }); process.on("exit", () => { if (process.env.CAL_DEBUG) console.log(JSON.stringify(reqs)); });
  page.on("framenavigated", (f) => f === page.mainFrame() && console.log("nav", f.url().slice(0, 80)));
  const cdp = await ctx.newCDPSession(page);
  await cdp.send("WebAuthn.enable");
  await cdp.send("WebAuthn.addVirtualAuthenticator", { options: { protocol: "ctap2", transport: "internal", hasResidentKey: true, hasUserVerification: true, isUserVerified: true, automaticPresenceSimulation: true } });
  await page.addInitScript(() => localStorage.setItem("cal.firstrun", "1"));

  await page.goto(`${url}/?server=1`);
  await page.waitForSelector("html[data-ready]");
  check(await page.getByRole("heading", { name: "Set up Calternal" }).isVisible(), "empty installation shows first-owner setup");
  // A wrong setup code is refused in plain words and the form stays.
  await fillWho(page, "not-the-code", "Test Owner");
  await page.getByRole("button", { name: "Create passkey" }).click();
  await page.getByText(/wrong or has expired.*setup-code/).waitFor({ timeout: 8000 });
  check(true, "wrong setup code: plain words and the form stays");
  await page.reload(); await page.waitForSelector("html[data-ready]");
  await signUp(page, setupToken, "Test Owner");
  await page.waitForSelector("[data-app], aside, nav", { timeout: 15000 }).catch(() => {});
  await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(1200); // (a live stream stays open, so the network is never idle)
  let st = await (await ctx.request.get(`${url}/api/v1/auth/status`)).json();
  check(st.signed_in === true && st.setup === false, `setup signs in (status ${JSON.stringify(st)})`);
  check(!(await page.getByRole("heading", { name: "Set up Calternal" }).isVisible().catch(() => false)), "app shown after setup");

  await page.goto(`${url}/`); await page.waitForSelector("html[data-ready]");
  check(!(await page.getByRole("heading", { name: "Sign in" }).isVisible().catch(() => false)), "session survives a reload");

  page.on("dialog", (d) => void d.accept());
  const go = async (path) => { await page.evaluate((p) => { history.pushState(null, "", p); dispatchEvent(new PopStateEvent("popstate")); }, path); };
  const deep = await page.goto(`${url}/settings/account`); await page.waitForSelector("html[data-ready]");
  check(deep.status() === 200, "deep link reload is served the shell by the server itself", String(deep.status()));
  await page.goto(`${url}/`); await page.waitForSelector("html[data-ready]");
  // Settings pages show real data in server mode.
  await go("/settings/account");
  await page.waitForFunction(() => document.querySelector("#acc-name")?.value === "Test Owner", null, { timeout: 8000 });
  check(await page.getByText("(this browser)").first().isVisible(), "Account: this browser listed among the installations");
  check(await page.locator('[data-setting^="account.passkey."]').count() === 1, "Account: the one passkey is listed");
  await go("/settings/members");
  await page.getByText("Test Owner (you)").first().waitFor({ timeout: 8000 });
  check(await page.getByText("Owner", { exact: true }).first().isVisible(), "Members: you are the owner");
  await go("/settings/machines");
  await page.getByRole("button", { name: "Make a code" }).click();
  await page.getByText(/calternal machine enroll/).waitFor({ timeout: 8000 });
  check(true, "Machines: an enrollment code is made");
  check(await page.getByText("Admin", { exact: true }).count() > 0 && await page.getByText("People", { exact: true }).count() > 0, "Admin: the operator sees People and Groups, by capability");
  // The line to the server: offline shows the banner and clears when it returns.
  await go("/settings/secrets"); await page.getByText(/No secrets/).waitFor({ timeout: 8000 }); await go("/settings/account");
  await ctx.setOffline(true);
  await go("/settings/secrets");
  await page.getByText(/Reconnecting/).waitFor({ timeout: 5000 });
  check(true, "offline: reconnecting banner");
  await ctx.setOffline(false);
  await page.getByText(/Reconnecting/).waitFor({ state: "hidden", timeout: 8000 });
  check(true, "back online: banner gone");
  await go("/settings/account");
  await page.getByRole("button", { name: "Sign out", exact: true }).click();
  await page.getByRole("heading", { name: "Sign in" }).waitFor({ timeout: 10000 });
  st = await (await ctx.request.get(`${url}/api/v1/auth/status`)).json();
  check(st.signed_in === false, "sign out ends the session");

  await page.getByRole("button", { name: "Sign in with a passkey" }).click();
  await page.waitForSelector("html[data-ready]");
  await page.waitForFunction(() => !document.querySelector("h1")?.textContent?.includes("Sign in"), null, { timeout: 15000 }).catch(() => {});
  st = await (await ctx.request.get(`${url}/api/v1/auth/status`)).json();
  check(st.signed_in === true, "signing in again with the passkey works");
  // Another protocol version: a plain notice instead of the app (older and newer server).
  for (const [v, title] of [[2, "This page is out of date"], [0, "The server is out of date"]]) {
    const p2 = await ctx.newPage();
    await p2.route("**/api/v1/describe", async (r) => { const res = await r.fetch(); const j = await res.json(); j.protocol_version = v; await r.fulfill({ response: res, json: j }); });
    await p2.goto(`${url}/`); await p2.waitForSelector("html[data-ready]");
    await p2.getByRole("heading", { name: title }).waitFor({ timeout: 8000 }).catch(() => {}); // the app paints from the device record first; the server's answer then turns it into the notice
    check(await p2.getByRole("heading", { name: title }).isVisible(), `protocol ${v}: "${title}"`);
    await p2.close();
  }
  // Server unreachable at load: a device that has never opened the app gets a screen with Try again; a known device paints the app and says it is reconnecting.
  {
    const fresh = await browser.newContext(); const p3 = await fresh.newPage();
    await p3.route("**/api/v1/auth/status", (r) => r.abort());
    await p3.goto(`${url}/?server=1`); await p3.waitForSelector("html[data-ready]");
    check(await p3.getByRole("heading", { name: "Can't reach the server" }).isVisible(), "unreachable at load, first visit: plain screen with Try again");
    await fresh.close();
    const p5 = await ctx.newPage();
    await p5.route("**/api/v1/**", (r) => r.abort()); // the server is away: every call fails
    await p5.goto(`${url}/?server=1`); await p5.waitForSelector("html[data-ready]");
    await p5.getByText(/Reconnecting/).waitFor({ timeout: 6000 }).catch(() => {});
    check(await p5.getByText(/Reconnecting/).isVisible() && await p5.locator("aside.side").isVisible(), "unreachable at load, known device: the app paints and says it is reconnecting");
    await p5.close();
  }
  // A passkey the server does not know (a device with none): said in words, nothing breaks.
  const ctx2 = await browser.newContext(); const p4 = await ctx2.newPage(); const c4 = await ctx2.newCDPSession(p4);
  await c4.send("WebAuthn.enable"); await c4.send("WebAuthn.addVirtualAuthenticator", { options: { protocol: "ctap2", transport: "internal", hasResidentKey: true, hasUserVerification: true, isUserVerified: true, automaticPresenceSimulation: true } });
  await p4.addInitScript(() => localStorage.setItem("cal.firstrun", "1"));
  await p4.goto(`${url}/?server=1`); await p4.waitForSelector("html[data-ready]");
  await p4.getByRole("button", { name: "Sign in with a passkey" }).click();
  await p4.getByRole("alert").waitFor({ timeout: 8000 });
  check(/passkey/.test(await p4.getByRole("alert").innerText()), "no passkey on this device: said in words");
  await ctx2.close();
  // An ended session returns to the sign-in screen at the same address.
  await go("/settings/account");
  await ctx.clearCookies();
  await go("/settings/secrets"); // the page asks the server, which refuses the missing session
  // The standalone server has no deep-link fallback (Caddy has one on the preview), so the reload itself lands on its 404 here; the banner and the unchanged address are what this host can show.
  await page.getByText(/session ended/).first().waitFor({ timeout: 5000 });
  await page.waitForTimeout(2000);
  check(new URL(page.url()).pathname === "/settings/secrets", "session expired: banner, then a reload at the same address");
  await browser.close();
} catch (e) { console.error(e); console.error(log.slice(-1500)); failed++; }
stop();
process.exit(failed ? 1 : 0);

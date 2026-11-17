// Start-up in server mode: a real server with an owner; cold (this browser has never opened the app: no device cache) and warm (a second visit in the same browser), with no added latency and with 100 ms of network
// round trip each way added (a phone on a normal connection). Reports, per scenario, the median of N loads: ready (the shell is painted: `data-ready`) and settled (the last API answer is in). Env: CAL_BIN, CAL_DIST.
import { spawn } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { chromium } from "playwright";

const PORT = Number(process.env.CAL_START_PORT ?? 3397), N = Number(process.argv[2] ?? 9);
const BIN = process.env.CAL_BIN ?? `${process.env.HOME}/Developer/lanes/ui-shell/target/debug/calternal`;
const data = mkdtempSync(join(tmpdir(), "cal-start-"));
const srv = spawn(BIN, ["serve", "--create", "--installation-id", crypto.randomUUID(), "--appdata", join(data, "appdata"), "--userdata-ssd", join(data, "ssd"), "--userdata-hdd", join(data, "hdd"), "--port", String(PORT), "--web-dir", process.env.CAL_DIST ?? "dist"], { stdio: "ignore" });
const url = `http://localhost:${PORT}`;
const med = (a) => [...a].sort((x, y) => x - y)[Math.floor(a.length / 2)];
let bad = 0;
try {
  for (let i = 0; i < 80; i++) { try { if ((await fetch(`${url}/api/v1/auth/status`)).ok) break; } catch {} await new Promise((r) => setTimeout(r, 250)); }
  const token = readFileSync(join(data, "appdata", "setup.token"), "utf8").trim();
  const b = await chromium.launch();
  const setup = await b.newContext();
  const sp = await setup.newPage(); const c = await setup.newCDPSession(sp);
  await c.send("WebAuthn.enable"); await c.send("WebAuthn.addVirtualAuthenticator", { options: { protocol: "ctap2", transport: "internal", hasResidentKey: true, hasUserVerification: true, isUserVerified: true, automaticPresenceSimulation: true } });
  await sp.addInitScript(() => localStorage.setItem("cal.firstrun", "1"));
  await sp.goto(`${url}/?server=1`); await sp.waitForSelector("html[data-ready]");
  await sp.fill('input[name="token"]', token); await sp.fill('input[name="name"]', "Starter"); await sp.getByRole("button", { name: "Create passkey" }).click();
  await sp.waitForFunction(() => !document.querySelector("h1")?.textContent?.includes("Set up"), null, { timeout: 15000 });
  await sp.waitForTimeout(1500);
  const state = await setup.storageState(); // the session cookie (and, once it exists, the device cache)
  const cookieOnly = { cookies: state.cookies, origins: [] };
  const flashes = [];
  async function visit(ctx, latency) {
    const p = await ctx.newPage();
    if (latency) { const cc = await ctx.newCDPSession(p); await cc.send("Network.enable"); await cc.send("Network.emulateNetworkConditions", { offline: false, latency, downloadThroughput: -1, uploadThroughput: -1 }); }
    await p.addInitScript(() => { localStorage.setItem("cal.firstrun", "1"); localStorage.setItem("cal.server", "1"); window.__api = 0; const f = window.fetch; window.fetch = (...a) => f(...a).finally(() => { window.__api = performance.now(); }); // the last API answer
      new MutationObserver(() => { const t = document.body?.innerText ?? ""; if (/Sign in|Set up Calternal/.test(t) && document.documentElement.hasAttribute("data-ready")) window.__flashSignIn = true; }).observe(document, { subtree: true, childList: true, characterData: true }); });
    const t0 = Date.now();
    await p.goto(`${url}/threads`, { waitUntil: "commit" });
    await p.waitForSelector("html[data-ready]", { timeout: 20000 });
    const ready = await p.evaluate(() => performance.now());
    await p.waitForTimeout(1200 + (latency ?? 0) * 4);
    const m = await p.evaluate(() => ({ settled: window.__api, flash: !!window.__flashSignIn, shell: !!document.querySelector("aside.side") }));
    await p.close();
    return { ready, settled: m.settled, flash: m.flash, shell: m.shell };
  }
  for (const latency of [0, 100]) {
    const cold = [], warm = [], coldS = [], warmS = [];
    for (let i = 0; i < N; i++) { const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, storageState: cookieOnly }); const r = await visit(ctx, latency); cold.push(r.ready); coldS.push(r.settled); if (!r.shell) bad++; if (r.flash) flashes.push("cold"); await ctx.close(); }
    for (let i = 0; i < N; i++) { const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, storageState: cookieOnly }); await visit(ctx, latency); const r = await visit(ctx, latency); warm.push(r.ready); warmS.push(r.settled); if (!r.shell) bad++; if (r.flash) flashes.push("warm"); await ctx.close(); }
    console.log(`latency +${latency} ms each way   cold: ready ${med(cold).toFixed(0)} ms, settled ${med(coldS).toFixed(0)} ms   warm: ready ${med(warm).toFixed(0)} ms, settled ${med(warmS).toFixed(0)} ms`);
  }
  console.log(flashes.length ? `FAIL  the sign-in screen flashed on ${flashes.length} loads` : "PASS  no sign-in flash for a signed-in person");
  if (flashes.length || bad) bad++;
  await b.close();
} catch (e) { console.error(e); bad++; }
srv.kill(); rmSync(data, { recursive: true, force: true });
process.exit(bad ? 1 : 0);

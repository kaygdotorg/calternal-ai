// Accessibility in server mode: a real `calternal serve` on an empty installation, a virtual authenticator, axe on every screen (setup, signed-in shell, each Settings page, the failure screens), light and dark,
// desktop and phone, plus a keyboard-only walk of the setup and sign-in screens. Env: CAL_BIN, CAL_DIST, CAL_A11Y_PORT.
import AxeBuilder from "@axe-core/playwright";
import { spawn } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { chromium } from "playwright";

const PORT = Number(process.env.CAL_A11Y_PORT ?? 3393);
const BIN = process.env.CAL_BIN ?? `${process.env.HOME}/Developer/lanes/ui-shell/target/debug/calternal`;
const data = mkdtempSync(join(tmpdir(), "cal-a11y-"));
const srv = spawn(BIN, ["serve", "--create", "--installation-id", crypto.randomUUID(), "--appdata", join(data, "appdata"), "--userdata-ssd", join(data, "ssd"), "--userdata-hdd", join(data, "hdd"), "--port", String(PORT), "--web-dir", process.env.CAL_DIST ?? "dist"], { stdio: "ignore" });
const url = `http://localhost:${PORT}`;
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const DEV = { desktop: { viewport: { width: 1440, height: 900 } }, phone: { viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true } };
const axe = async (page, name) => {
  const r = await new AxeBuilder({ page }).withTags(["wcag2a", "wcag2aa", "wcag21aa", "best-practice"]).analyze();
  const v = r.violations.map((x) => `${x.id} (${x.nodes.length}): ${x.nodes[0].target.join(" ")}`);
  ok(!v.length, name, v.length ? "\n   " + v.join("\n   ") : "");
};
try {
  for (let i = 0; i < 80; i++) { try { if ((await fetch(`${url}/api/v1/auth/status`)).ok) break; } catch {} await new Promise((r) => setTimeout(r, 250)); }
  const token = (() => { for (const f of ["appdata/setup.token"]) try { return readFileSync(join(data, f), "utf8").trim(); } catch {} return ""; })();
  const b = await chromium.launch();
  const newCtx = async (dev, scheme) => {
    const ctx = await b.newContext({ ...DEV[dev], colorScheme: scheme, reducedMotion: "no-preference" });
    await ctx.addInitScript(() => localStorage.setItem("cal.firstrun", "1"));
    const p = await ctx.newPage(); const c = await ctx.newCDPSession(p);
    await c.send("WebAuthn.enable"); await c.send("WebAuthn.addVirtualAuthenticator", { options: { protocol: "ctap2", transport: "internal", hasResidentKey: true, hasUserVerification: true, isUserVerified: true, automaticPresenceSimulation: true } });
    return { ctx, p };
  };
  const ready = async (p) => { await p.waitForSelector("html[data-ready]"); await p.waitForTimeout(400); };
  // Setup screen, then its keyboard walk (Tab to the fields and the button, type, Enter), then the account is made.
  {
    const { ctx, p } = await newCtx("desktop", "light");
    await p.goto(`${url}/?server=1`); await ready(p);
    await axe(p, "desktop light setup");
    await p.keyboard.press("Tab"); const first = await p.evaluate(() => document.activeElement?.getAttribute("name"));
    await p.keyboard.type(token); await p.keyboard.press("Tab"); const second = await p.evaluate(() => document.activeElement?.getAttribute("name"));
    await p.keyboard.type("Test Owner"); await p.keyboard.press("Tab");
    const third = await p.evaluate(() => document.activeElement?.textContent?.trim());
    ok(first === "token" && second === "name" && third === "Create passkey", "setup: Tab goes code, name, button, in that order", `${first}, ${second}, ${third}`);
    const ring = await p.evaluate(() => { const c = getComputedStyle(document.activeElement); return c.outlineStyle !== "none" || c.boxShadow !== "none"; });
    ok(ring, "setup: the focused button shows a focus indicator");
    await p.keyboard.press("Enter");
    await p.waitForFunction(() => !document.querySelector("h1")?.textContent?.includes("Set up"), null, { timeout: 15000 });
    await p.waitForTimeout(800);
    global.state = await ctx.storageState();
    await ctx.close();
  }
  // The sign-in screen (a browser without the session or the passkey).
  for (const dev of ["desktop", "phone"]) for (const scheme of ["light", "dark"]) {
    const { ctx, p } = await newCtx(dev, scheme);
    await p.goto(`${url}/?server=1`); await ready(p);
    await axe(p, `${dev} ${scheme} sign-in`);
    if (dev === "desktop" && scheme === "light") {
      await p.keyboard.press("Tab");
      const name = await p.evaluate(() => document.activeElement?.textContent?.trim());
      ok(name === "Sign in with a passkey", "sign-in: the first Tab stop is the button", name);
    }
    await ctx.close();
  }
  // Signed in (the session made by setup, reused in every context): the shell and every Settings page, both schemes, desktop and phone.
  const PAGES = ["/threads", "/notes", "/settings/appearance", "/settings/keyboard", "/settings/machines", "/settings/account", "/settings/members", "/settings/secrets", "/settings/space-plugins", "/settings/manage"];
  for (const dev of ["desktop", "phone"]) for (const scheme of ["light", "dark"]) {
    const ctx = await b.newContext({ ...DEV[dev], colorScheme: scheme, storageState: global.state });
    await ctx.addInitScript(() => localStorage.setItem("cal.firstrun", "1"));
    const p = await ctx.newPage();
    for (const path of PAGES) {
      await p.goto(`${url}/?server=1`); await ready(p);
      await p.evaluate((x) => { history.pushState(null, "", x); dispatchEvent(new PopStateEvent("popstate")); }, path);
      await p.waitForTimeout(1200);
      await axe(p, `${dev} ${scheme} ${path}`);
    }
    // The failure screens.
    const q = await ctx.newPage();
    await q.route("**/api/v1/describe", async (r) => { const res = await r.fetch(); const j = await res.json(); j.protocol_version = 2; await r.fulfill({ response: res, json: j }); });
    await q.goto(`${url}/`); await ready(q); await axe(q, `${dev} ${scheme} version notice`); await q.close();
    await ctx.close();
    const c2 = await b.newContext({ ...DEV[dev], colorScheme: scheme });
    const u = await c2.newPage(); await u.route("**/api/v1/auth/status", (r) => r.abort());
    await u.goto(`${url}/?server=1`); await ready(u); await axe(u, `${dev} ${scheme} server unreachable`);
    await c2.close();
  }
  await b.close();
} catch (e) { console.error(e); bad++; }
srv.kill(); rmSync(data, { recursive: true, force: true });
process.exit(bad ? 1 : 0);

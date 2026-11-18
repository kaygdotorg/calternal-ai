// The app as `calternal serve` itself serves it: at the root (no /ai/ prefix), same origin as the API, under the server's strict Content-Security-Policy. Starts a real server on a disposable installation with this build's `dist/`
// (Env: CAL_BIN, CAL_DIST) and looks for what would differ from the preview host: CSP violations (reported by the browser), failed requests, console errors and uncaught exceptions on the fixtures (`?server=0`) and on the
// sign-in screen (`?server=1`), light and dark, a deep link reloaded (the server's fallback to index.html), the manifest and its icons, the fonts and the wallpaper copies (blob images), the web animations.
import { spawn } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createServer } from "node:net";
import { chromium } from "playwright";
const free = () => new Promise((res) => { const s = createServer(); s.listen(0, "127.0.0.1", () => { const { port } = s.address(); s.close(() => res(port)); }); });
const PORT = await free(), BIN = process.env.CAL_BIN ?? `${process.env.HOME}/Developer/lanes/ui-fresh/target/debug/calternal`, DIST = process.env.CAL_DIST ?? "dist";
const data = mkdtempSync(join(tmpdir(), "cal-served-"));
const srv = spawn(BIN, ["serve", "--create", "--installation-id", crypto.randomUUID(), "--appdata", join(data, "appdata"), "--userdata-ssd", join(data, "ssd"), "--userdata-hdd", join(data, "hdd"), "--port", String(PORT), "--web-dir", DIST], { stdio: "ignore" });
const stop = () => { srv.kill(); rmSync(data, { recursive: true, force: true }); };
process.on("exit", stop);
let bad = 0; const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const url = `http://localhost:${PORT}`; // the server checks Host against its own address: localhost, not 127.0.0.1
for (let i = 0; i < 120; i++) { try { if ((await fetch(`${url}/api/v1/auth/status`)).ok) break; } catch { /* not yet */ } await new Promise((r) => setTimeout(r, 250)); }
const b = await chromium.launch();
for (const scheme of ["light", "dark"]) for (const [name, path] of [["fixtures", "/?server=0"], ["deep link (a UUID, as the server allows)", "/threads/0197f3a2-5c1e-7b4d-9a60-3d2e8f1c4b11?server=0"], ["sign-in", "/?server=1"]]) {
  const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, colorScheme: scheme }); await ctx.addInitScript(() => { localStorage.setItem("cal.firstrun", "1"); window.__csp = []; document.addEventListener("securitypolicyviolation", (e) => window.__csp.push(`${e.violatedDirective} ${e.blockedURI}`)); });
  const page = await ctx.newPage(); const errors = [], failed = [];
  page.on("pageerror", (e) => errors.push(e.message.slice(0, 120))); page.on("console", (m) => { if (m.type() === "error" && !/401|Unauthorized/.test(m.text())) errors.push(m.text().slice(0, 160)); });
  page.on("requestfailed", (r) => failed.push(r.url().replace(url, "") + " " + (r.failure()?.errorText ?? ""))); page.on("response", (r) => { if (r.status() >= 400 && !/\/api\/v1\/(auth|account|operations)/.test(r.url())) failed.push(`${r.status()} ${r.url().replace(url, "")}`); });
  await page.goto(url + path); await page.waitForSelector("html[data-ready]", { timeout: 20000 }); await page.waitForTimeout(2000);
  const r = await page.evaluate(async () => { const m = document.querySelector('link[rel=manifest]')?.href; const mj = m ? await (await fetch(m)).json().catch(() => null) : null; const icon = mj ? await fetch(new URL(mj.icons[0].src, m)).then((x) => x.status) : null; return { csp: window.__csp, manifest: !!mj, icon, wall: document.documentElement.style.getPropertyValue("--wall-side").slice(0, 5), bd: getComputedStyle(document.querySelector(".backdrop") ?? document.documentElement).backgroundImage.slice(0, 8), fonts: [...document.fonts].filter((f) => f.status === "loaded").length, theme: document.querySelector('meta[name="theme-color"]').getAttribute("content") }; });
  const tag = `${scheme} ${name}`;
  ok(r.csp.length === 0, `${tag}: no Content-Security-Policy violation`, r.csp.join("; "));
  ok(errors.length === 0 && failed.length === 0, `${tag}: no console error, uncaught exception or failed request`, [...errors, ...failed].slice(0, 3).join(" | "));
  ok(r.manifest && r.icon === 200, `${tag}: the manifest and its icon are served`, `icon ${r.icon}`);
  ok(r.fonts >= 1 && (r.bd.startsWith("url(") || name === "sign-in"), `${tag}: fonts loaded and the wallpaper is painted`, `${r.fonts} fonts, backdrop ${r.bd}, wall-side ${r.wall || "(from the cheap default)"}`);
  await ctx.close();
}
await b.close(); stop(); process.exit(bad ? 1 : 0);

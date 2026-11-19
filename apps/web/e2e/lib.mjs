import { cpus, loadavg, tmpdir } from "node:os";
// Shared by the e2e scripts: starts `vite preview` of the built app, opens Chromium, and exposes small helpers.
import { spawn } from "node:child_process";
import { mkdtempSync, readdirSync, readFileSync, readlinkSync, rmSync } from "node:fs";
import { join } from "node:path";
import { chromium } from "playwright";

export const PORT = 4174;
/** CAL_URL points the scripts at an already deployed app (for example http://127.0.0.1/ai on the preview host) instead of a local preview. */
export const BASE = process.env.CAL_URL ?? `http://127.0.0.1:${PORT}`;

/** Starts the preview server and resolves when it answers. Returns a stop function. */
/** Stops any preview server already listening for this app on the port (a run that timed out leaves one behind, and it would serve an old build or the wrong proxy). Linux only; only processes whose directory is this app's. */
function clearPort() {
  if (process.platform !== "linux") return;
  try {
    for (const pid of readdirSync("/proc").filter((n) => /^\d+$/.test(n))) {
      let cmd = "";
      try { cmd = readFileSync(`/proc/${pid}/cmdline`, "utf8").replaceAll("\0", " "); } catch { continue; }
      if (!cmd.includes("vite") || !cmd.includes("preview") || !cmd.includes(`--port ${PORT}`)) continue;
      let cwd = "";
      try { cwd = readlinkSync(`/proc/${pid}/cwd`); } catch { continue; }
      if (cwd.startsWith(process.cwd())) { try { process.kill(Number(pid)); } catch { /* gone */ } }
    }
  } catch { /* no /proc */ }
}

export async function serve() {
  if (process.env.CAL_URL) return () => {};
  // A `dist` built for another base (the preview host's /ai/) serves assets from addresses this preview does not have: every script 404s and the page never becomes ready. Say so at once.
  try {
    const html = readFileSync("dist/index.html", "utf8");
    if (/(?:src|href)="\/[a-z0-9-]+\/assets\//.test(html)) { console.error("dist/ was built for a sub-path (CAL_BASE). Run `npm run build` (base /) before the e2e scripts; the preview host build goes to dist-preview."); process.exit(2); }
  } catch { console.error("No dist/ yet: run `npm run build` first."); process.exit(2); }
  clearPort();
  const p = spawn("npx", ["vite", "preview", "--port", String(PORT), "--strictPort", "--host", "127.0.0.1"], { stdio: "ignore", detached: true }); // its own process group, so stopping it also stops the node process npx starts
  for (let i = 0; i < 60; i++) {
    try { if ((await fetch(BASE)).ok) break; } catch { /* not yet */ }
    await new Promise((r) => setTimeout(r, 250));
  }
  return () => { try { process.kill(-p.pid); } catch { p.kill(); } };
}

/** Start a real server with disposable roots. Wait for this server's setup token and stop it before removing the roots. `extra` adds server arguments (and `env` environment variables): the plugin-prompt suite passes `--test-plugin-root` and the direct worker backend. */
export async function startRealServer(port, extra = { args: [], env: {} }) {
  const bin = process.env.CAL_BIN ?? `${process.env.HOME}/Developer/lanes/ui-shell/target/debug/calternal`;
  const data = mkdtempSync(join(tmpdir(), "cal-real-server-"));
  const appdata = join(data, "appdata");
  const server = spawn(bin, [
    "serve", "--create", "--installation-id", crypto.randomUUID(), "--appdata", appdata,
    "--userdata-ssd", join(data, "ssd"), "--userdata-hdd", join(data, "hdd"),
    "--port", String(port), "--web-dir", process.env.CAL_DIST ?? "dist",
    "--plugin-root", process.env.CAL_PLUGINS ?? "../../plugins",
    ...(extra.args ?? []),
  ], { stdio: ["ignore", "pipe", "pipe"], env: { ...process.env, ...(extra.env ?? {}) } });
  const url = `http://localhost:${port}`;
  let tail = "", exited = false, startupError;
  for (const stream of [server.stdout, server.stderr]) {
    stream.on("data", (chunk) => { tail = (tail + chunk).slice(-4096); });
  }
  const closed = new Promise((resolve) => {
    server.once("error", (error) => { startupError = error; });
    server.once("close", () => { exited = true; resolve(); });
  });
  const stop = async () => {
    if (!exited) server.kill();
    await closed;
    rmSync(data, { recursive: true, force: true });
  };
  try {
    for (let attempt = 0; attempt < 80 && !exited; attempt++) {
      try {
        const startup = JSON.parse(readFileSync(join(appdata, "startup.json"), "utf8"));
        if (startup.pid !== server.pid) throw new Error("Unexpected server PID");
        const token = readFileSync(join(appdata, "setup.token"), "utf8").trim();
        const ready = await fetch(`${url}/api/v1/auth/status`, { signal: AbortSignal.timeout(500) });
        if (token && ready.ok && !exited) {
          return { bin, data, url, token, stop, logTail: () => tail };
        }
      } catch { /* Server not ready yet. */ }
      await new Promise((resolve) => setTimeout(resolve, 250));
    }
    throw startupError ?? new Error(`Real server at ${url} did not become ready.
${tail}`);
  } catch (error) {
    await stop();
    throw error;
  }
}

/** Open a real-server test page with a disposable virtual passkey. */
export async function openRealServerPage(browser, options = {}) {
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 }, ...options });
  await context.addInitScript(() => localStorage.setItem("cal.firstrun", "1"));
  const page = await context.newPage();
  const cdp = await context.newCDPSession(page);
  await cdp.send("WebAuthn.enable");
  await cdp.send("WebAuthn.addVirtualAuthenticator", {
    options: {
      protocol: "ctap2", transport: "internal", hasResidentKey: true, hasUserVerification: true,
      isUserVerified: true, automaticPresenceSimulation: true,
    },
  });
  return page;
}

/** Complete first-owner signup against the disposable server. */
export async function signUpRealServerOwner(page, server, name) {
  await page.goto(`${server.url}/?server=1`);
  await page.waitForSelector("html[data-ready]");
  await signUp(page, server.token, name);
  await page.waitForFunction(() => !document.querySelector("h1")?.textContent?.includes("Set up"), null, { timeout: 15000 });
  await page.waitForSelector("html[data-ready]");
  await page.waitForTimeout(1200);
}

/** The devices the screenshots and checks use. */
export const DEVICES = {
  desktop: { viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 },
  phone: { viewport: { width: 390, height: 844 }, deviceScaleFactor: 2, isMobile: true, hasTouch: true },
};

/** Opens a page on a device in a scheme and waits until the app has painted (fonts in). */
export async function open(browser, device, scheme, path) {
  const ctx = await browser.newContext({ ...DEVICES[device], colorScheme: scheme, reducedMotion: "no-preference" });
  if (!process.env.CAL_FIRSTRUN) await ctx.addInitScript(() => { try { localStorage.setItem("cal.firstrun", "1"); } catch { /* none */ } });
  // CAL_CANVAS=1 runs any suite with the canvas on (the fixtures build's `?canvas=1` is remembered for the visit in sessionStorage): the same checks, windows instead of one card.
  if (process.env.CAL_CANVAS === "1") await ctx.addInitScript(() => { try { sessionStorage.setItem("cal.canvas.on", "1"); if (!localStorage.getItem("cal.canvas.prefs") && !location.search.includes("canvas=1")) localStorage.setItem("cal.canvas.prefs", JSON.stringify({ look: "card", width: 1 })); /* a window opens at the full width, as the one card the suites were written for does */ } catch { /* none */ } });
  const page = await ctx.newPage();
  await page.goto(BASE + path);
  await page.waitForSelector("html[data-ready]");
  await page.waitForFunction(() => getComputedStyle(document.documentElement).getPropertyValue("--wall-side").length > 0 || document.documentElement.dataset.nowall !== undefined, null, { timeout: 8000 }).catch(() => {});
  await page.waitForTimeout(500);
  return { ctx, page };
}

export const launch = () => chromium.launch();

/** Focuses the composer and waits until it has expanded and its morph has finished (the context and chips rows are in the layout and fully drawn): unfocused, the composer is collapsed and its chips are not there to click. Waits for the state, not for a time. */
export async function expandComposer(page) {
  await page.locator("#composer-field").click();
  await page.waitForFunction(() => { const c = (document.querySelector("[data-win][data-focused]") ?? document).querySelector(".composer"); const r = c?.querySelector(".chips"); return !!c && !c.hasAttribute("data-collapsed") && !!r && getComputedStyle(r).display !== "none" && !c.getAnimations({ subtree: true }).some((a) => a.playState === "running" && a.effect?.getComputedTiming().iterations !== Infinity); });
}

/** The Threads sidebar opens on its Inbox when something waits on the person; suites that work on the whole list switch to All Threads first (a no-op when that tab is already showing or there are no tabs). */
export async function allThreads(page) {
  const tab = page.locator("aside.side [role=tab]", { hasText: "All Threads" });
  if (await tab.count() && (await tab.getAttribute("aria-selected")) !== "true") { await tab.click(); await page.waitForTimeout(150); }
}

/**
 * Whether this machine is too busy to judge a timing: the one-minute load is over half the cores, or a short spin of 30 ms of CPU takes noticeably longer than 30 ms of wall time (other processes are taking the CPU). A timing check run in the
 * gate records its numbers and says "SKIP judgement" when this is true, and judges them when it is not; `CAL_QUIET=1` (the quiet list, `npm run e2e:quiet`) always judges. Cost: about 100 ms.
 */
export function busyNow() {
  const load = loadavg()[0], cores = cpus().length;
  let worst = 1;
  for (let i = 0; i < 3; i++) {
    const c0 = process.cpuUsage(), w0 = performance.now();
    for (;;) { const c = process.cpuUsage(c0); if (c.user + c.system >= 30000) break; }
    worst = Math.max(worst, (performance.now() - w0) / 30);
  }
  const busy = load > cores / 2 || worst > 1.4;
  return { busy, load, cores, spin: worst, why: `load ${load.toFixed(1)} on ${cores} cores, a 30 ms spin took ${(worst * 30).toFixed(0)} ms` };
}
/**
 * True when timing checks must be judged: the quiet list (`CAL_QUIET=1`), or a suite run by hand on a machine that is not busy. Never inside the gate (`CAL_GATE=1`): the gate runs several suites at once, so
 * the load it measures at a suite's start is not the load the suite will meet (a perf check that goes red because the gate's own neighbours started a moment later proves nothing); the gate records the
 * numbers and the quiet list judges them.
 */
export const judgeTimings = () => process.env.CAL_QUIET === "1" || (process.env.CAL_GATE !== "1" && !busyNow().busy);

/** The username a test account gets from its name (3 to 32 letters, digits and single hyphens). */
const usernameOf = (name) => (name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "").slice(0, 32) + "-qa").replace(/^-+/, "");

/** First-owner (or invited) signup through the real screens up to the passkey screen: setup code, name, username, then the email screen left empty (optional on a server with no mail). */
export async function fillWho(p, token, name) {
  if (token) await p.fill('input[name="token"]', token);
  await p.fill('input[name="name"]', name);
  await p.fill('input[name="username"]', usernameOf(name));
  await p.getByRole("button", { name: "Continue" }).click();
  await p.getByLabel(/^Email/).waitFor();
  await p.getByRole("button", { name: "Continue" }).click();
  await p.getByRole("button", { name: "Create passkey" }).waitFor();
}

/** A whole signup for tests that only need an account: the screens above, the passkey, the recovery codes saved, every optional step skipped, then into the app. */
export async function signUp(p, token, name) {
  await fillWho(p, token, name);
  await p.getByRole("button", { name: "Create passkey" }).click();
  await p.getByRole("button", { name: "I have saved them" }).click();
  for (const step of ["A second passkey", "Choose a look"]) { await p.getByRole("heading", { name: step }).waitFor({ timeout: 15000 }); await p.locator("footer").getByRole("button", { name: "Skip" }).click(); }
  for (let i = 0; i < 3; i++) {
    const done = p.getByRole("button", { name: "Start the first thread" });
    if (await done.isVisible().catch(() => false)) break;
    await p.waitForTimeout(600);
    const skip = p.locator("footer").getByRole("button", { name: "Skip" });
    if (await skip.isVisible().catch(() => false)) await skip.click();
  }
  await p.getByRole("button", { name: "Start the first thread" }).click();
  await p.waitForSelector("html[data-ready]");
}

/**
 * The window a suite is looking at: the focused canvas window when the canvas is on, the middle card when it is not. A canvas can hold several windows (each with its own composer, header and
 * headings), so a suite scopes `.composer`, `h1`, the header and the list to this, never to the page. (The composer's field keeps the id `composer-field` in the focused window alone.)
 */
export const win = (page) => page.locator("[data-win][data-focused], .mid-card:not([data-win])").first();

/**
 * An https front for a real server, for what needs a secure public origin (web push: "Web push requires an HTTPS server"): a self-signed certificate made here, a TLS listener on `port` that forwards every request (streams included) to the server's own http address. Start the server with
 * `--origin https://localhost:<port>` and open pages with `ignoreHTTPSErrors`. Returns `{ url, stop }`.
 */
export async function startTlsProxy(targetUrl, port) {
  const { execFileSync } = await import("node:child_process");
  const https = await import("node:https"), http = await import("node:http");
  const dir = mkdtempSync(join(tmpdir(), "cal-tls-"));
  execFileSync("openssl", ["req", "-x509", "-newkey", "rsa:2048", "-nodes", "-keyout", join(dir, "k.pem"), "-out", join(dir, "c.pem"), "-days", "1", "-subj", "/CN=localhost", "-addext", "subjectAltName=DNS:localhost"], { stdio: "ignore" });
  const target = new URL(targetUrl);
  const srv = https.createServer({ key: readFileSync(join(dir, "k.pem")), cert: readFileSync(join(dir, "c.pem")) }, (req, res) => {
    const up = http.request({ host: target.hostname, port: target.port, method: req.method, path: req.url, headers: req.headers }, (r) => { res.writeHead(r.statusCode ?? 502, r.headers); r.pipe(res); });
    up.on("error", () => { if (!res.headersSent) res.writeHead(502); res.end(); });
    res.on("close", () => up.destroy());
    req.pipe(up);
  });
  await new Promise((resolve) => srv.listen(port, "127.0.0.1", resolve));
  return { url: `https://localhost:${port}`, stop: async () => { srv.closeAllConnections?.(); await new Promise((r) => srv.close(r)); rmSync(dir, { recursive: true, force: true }); } };
}

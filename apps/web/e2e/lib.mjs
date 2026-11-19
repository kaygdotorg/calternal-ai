import { cpus, loadavg } from "node:os";
// Shared by the e2e scripts: starts `vite preview` of the built app, opens Chromium, and exposes small helpers.
import { spawn } from "node:child_process";
import { readdirSync, readFileSync, readlinkSync } from "node:fs";
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

/** Cumulative [steal, total] jiffies of all CPUs (/proc/stat): steal is the time the host gave to another VM, which the guest's own load average cannot see. Null where /proc/stat has no steal column (not Linux). */
function stealJiffies() { try { const f = readFileSync("/proc/stat", "utf8").split("\n")[0].trim().split(/\s+/).slice(1).map(Number); return f.length > 7 ? [f[7], f.slice(0, 8).reduce((a, b) => a + b, 0)] : null; } catch { return null; } }
/** Starts watching steal time; `end()` gives the share of the window (0 to 1) the host spent elsewhere, or 0 when it cannot be read. A sample with more than 1 percent (`STEAL_LIMIT`) is not judged. */
export function stealWindow() { const a = stealJiffies(); return { end: () => { const b = stealJiffies(); return a && b && b[1] > a[1] ? (b[0] - a[0]) / (b[1] - a[1]) : 0; } }; }
export const STEAL_LIMIT = 0.01;
/** The median of a list of numbers (the upper middle for an even count). */
export const median = (xs) => { const s = [...xs].sort((a, b) => a - b); return s.length ? s[Math.floor(s.length / 2)] : NaN; };
/**
 * Judges samples of one timing the statistical way: it passes when the MEDIAN is under `bound` and at most one sample is over `hard`; every sample is printed (a sample marked "not judged", steal time over 1 percent, is
 * printed and left out). Returns `{ pass, judged, text }`. At least three judged samples are needed, or the result is "not judged" (`pass: null`).
 */
export function judgeSamples(label, samples, { bound, hard, unit = "ms" }) {
  const used = samples.filter((x) => !x.notJudged), m = median(used.map((x) => x.value)), over = used.filter((x) => x.value > hard).length;
  const list = samples.map((x) => (x.notJudged ? `[${x.value.toFixed(0)}: not judged, steal ${(x.steal * 100).toFixed(1)}%]` : x.value.toFixed(0))).join(" ");
  const text = `${label}: ${list} ${unit}; median ${Number.isFinite(m) ? m.toFixed(0) : "?"} (bound ${bound}), ${over} over ${hard}`;
  if (used.length < 3) return { pass: null, judged: used.length, text: `${text}; only ${used.length} judged samples` };
  return { pass: m < bound && over <= 1, judged: used.length, text };
}

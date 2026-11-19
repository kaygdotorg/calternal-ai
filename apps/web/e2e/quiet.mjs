// The quiet list: the timing-sensitive suites, run ALONE and in turn, always judged (CAL_QUIET=1), only in a window the machine was quiet for. The gate records these numbers and says "SKIP judgement" when the machine
// is busy; this is where they are judged, so a FAIL here must mean the code, never a neighbour. Rules: (1) start only when the one-minute AND five-minute load are both under 1.0 and the spin probe is clean (a
// gate that just finished leaves the five-minute load high); (2) before each suite wait for the machine to be idle again (the previous suite's own browser is gone), and measure how many cores were busy
// while the suite ran: a suite whose window was busier than a lone browser explains, or whose end probe was disturbed, is NOT JUDGED (its verdict is thrown away), and is run again up to twice; (3) exit 0 all
// pass, 1 a suite FAILED in a quiet window, 3 something could not be judged. Usage: `npm run build -w apps/web && npm run e2e:quiet -w apps/web`. Env: CAL_QUIET_WAIT_S (default 900), CAL_QUIET_CORES (default 2: a lone browser at 4x throttling uses about 1 to 1.5 cores, so more than 2 means something else was running).
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { cpus, loadavg } from "node:os";
import { busyNow } from "./lib.mjs";
const SUITES = ["clarity-perf", "canvas", "composer-fold", "switch", "phone-nav", "sidebar-pull", "palette", "thread-size", "live-typing", "press"];
const wait = Number(process.env.CAL_QUIET_WAIT_S ?? 900), MAX_CORES = Number(process.env.CAL_QUIET_CORES ?? 2), RETRIES = 2, cores = cpus().length;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
/** Cumulative [busy, total] jiffies of every CPU (/proc/stat's first line). */
const jiffies = () => { const f = readFileSync("/proc/stat", "utf8").split("\n")[0].trim().split(/\s+/).slice(1).map(Number); const idle = f[3] + (f[4] ?? 0); const total = f.reduce((a, b) => a + b, 0); return [total - idle, total]; };
/** The number of cores that were busy (all processes) between two jiffies samples. */
const busyCores = ([b0, t0], [b1, t1]) => (t1 > t0 ? ((b1 - b0) / (t1 - t0)) * cores : 0);
/** Busy cores over `ms` of waiting. */
const sample = async (ms) => { const a = jiffies(); await sleep(ms); return busyCores(a, jiffies()); };
const t0 = Date.now();
const left = () => wait - (Date.now() - t0) / 1000;
/** Waits until `ok()` holds or the patience runs out; says why it is waiting. */
async function until(what, ok) { for (;;) { const s = await ok(); if (s.ok) return true; if (left() <= 0) { console.log(`NOT JUDGED  ${what}: still not quiet after ${wait} s (${s.why})`); return false; } console.log(`waiting for ${what} (${s.why})`); await sleep(15000); } }
const startOk = async () => { const [l1, , l5] = loadavg(); const b = busyNow(); return { ok: l1 < 1.0 && l5 < 1.0 && b.spin <= 1.4, why: `load ${l1.toFixed(1)} (1 min) ${l5.toFixed(1)} (5 min), spin ${(b.spin * 30).toFixed(0)} ms for 30 ms` }; };
const idleOk = async () => { const c = await sample(2000), b = busyNow(); return { ok: c < 1.0 && b.spin <= 1.4, why: `${c.toFixed(1)} cores busy, spin ${(b.spin * 30).toFixed(0)} ms for 30 ms` }; };
let bad = 0, unjudged = 0;
if (!(await until("the start (both load averages under 1.0)", startOk))) { console.log("Nothing was run (exit code 3, not a failure)."); process.exit(3); }
for (const name of SUITES) {
  let verdict = null;
  for (let attempt = 0; attempt <= RETRIES && !verdict; attempt++) {
    if (!(await until(`the machine to be idle before ${name}`, idleOk))) break;
    const j0 = jiffies(), w0 = Date.now();
    const args = name === "palette-focus" ? ["12"] : [];
    const r = spawnSync("node", ["e2e/with-preview.mjs", `e2e/${name}.mjs`, ...args], { env: { ...process.env, CAL_QUIET: "1" }, encoding: "utf8", timeout: 300000 });
    const used = busyCores(j0, jiffies()), after = busyNow(), secs = (Date.now() - w0) / 1000;
    const disturbed = used > MAX_CORES || after.spin > 1.4;
    const fails = (r.stdout ?? "").split("\n").filter((l) => /^\s*FAIL/.test(l));
    if (disturbed && r.status !== 0) { console.log(`NOT JUDGED  ${name}${attempt < RETRIES ? " (again)" : ""}: its window was disturbed (${used.toFixed(1)} cores busy over ${secs.toFixed(0)} s, spin after ${(after.spin * 30).toFixed(0)} ms)`); continue; }
    verdict = { pass: r.status === 0, fails, used };
  }
  if (!verdict) { unjudged++; console.log(`NOT JUDGED  ${name}`); continue; }
  console.log(`${verdict.pass ? "PASS" : "FAIL"}  ${name}  (${verdict.used.toFixed(1)} cores busy)${verdict.fails.length ? "\n   " + verdict.fails.join("\n   ") : ""}`);
  if (!verdict.pass) bad++;
}
console.log(bad ? `${bad} of ${SUITES.length} quiet suites FAILED (each in a window of at most CAL_QUIET_CORES busy cores)` : unjudged ? `${unjudged} of ${SUITES.length} could not be judged (nothing failed in a quiet window)` : `all ${SUITES.length} quiet suites pass`);
process.exit(bad ? 1 : unjudged ? 3 : 0);

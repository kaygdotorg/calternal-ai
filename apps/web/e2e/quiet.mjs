// The quiet list (statistical): the timing-sensitive suites, run ALONE and in turn, always judged (CAL_QUIET=1), only in a window the machine was quiet for. The gate records these numbers and says "SKIP judgement" when the machine
// is busy; this is where they are judged, so a FAIL here must mean the code, never a neighbour. Rules: (1) start only when the one-minute AND five-minute load are both under 1.0 and the spin probe is clean (a
// gate that just finished leaves the five-minute load high); (2) before each suite wait for the machine to be idle again (the previous suite's own browser is gone), and measure how many cores were busy
// while the suite ran: a suite whose window was busier than a lone browser explains, or whose end probe was disturbed, is NOT JUDGED (its verdict is thrown away), and is run again up to twice; (3) exit 0 all
// pass, 1 a suite FAILED in a quiet window, 3 something could not be judged. Usage: `npm run build -w apps/web && npm run e2e:quiet -w apps/web`. Env: CAL_QUIET_WAIT_S (default 900), CAL_QUIET_CORES (default 2: a lone browser at 4x throttling uses about 1 to 1.5 cores, so more than 2 means something else was running).
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { cpus, loadavg } from "node:os";
import { busyNow } from "./lib.mjs";
const SUITES = ["clarity-perf", "canvas", "composer-fold", "switch", "phone-nav", "sidebar-pull", "palette", "thread-size", "live-typing", "press", "list-keys"];
const wait = Number(process.env.CAL_QUIET_WAIT_S ?? 900), MAX_CORES = Number(process.env.CAL_QUIET_CORES ?? 2), RETRIES = 2, cores = cpus().length;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
/** Cumulative [busy, total] jiffies of every CPU (/proc/stat's first line). */
const jiffies = () => { const f = readFileSync("/proc/stat", "utf8").split("\n")[0].trim().split(/\s+/).slice(1).map(Number); const idle = f[3] + (f[4] ?? 0); const total = f.slice(0, 8).reduce((a, b) => a + b, 0); return [total - idle, total, f[7] ?? 0]; };
/** The number of cores that were busy (all processes) between two jiffies samples. */
const stealShare = ([, t0, s0], [, t1, s1]) => (t1 > t0 ? (s1 - s0) / (t1 - t0) : 0);
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
// A suite that takes its own samples (and says so: clarity-perf prints every sample, with its median rule) runs once; every other suite runs RUNS judged times and passes when at most one run failed. A run whose window
// was not quiet (more busy cores than CAL_QUIET_CORES, a dirty spin probe after it, or more than 1 percent steal time: the host gave the CPU to another VM) is printed but does not count.
const SELF_SAMPLED = new Set(["clarity-perf"]);
const RUNS = Number(process.env.CAL_QUIET_RUNS ?? 3), ATTEMPTS = RUNS + 2;
for (const name of SUITES) {
  const need = SELF_SAMPLED.has(name) ? 1 : RUNS, runs = [];
  for (let attempt = 0; attempt < (SELF_SAMPLED.has(name) ? 3 : ATTEMPTS) && runs.filter((x) => !x.notJudged).length < need; attempt++) {
    if (!(await until(`the machine to be idle before ${name}`, idleOk))) break;
    const j0 = jiffies(), w0 = Date.now();
    const args = name === "palette-focus" ? ["12"] : [];
    const r = spawnSync("node", ["e2e/with-preview.mjs", `e2e/${name}.mjs`, ...args], { env: { ...process.env, CAL_QUIET: "1" }, encoding: "utf8", timeout: 600000 });
    const j1 = jiffies(), used = busyCores(j0, j1), steal = stealShare(j0, j1), after = busyNow(), secs = (Date.now() - w0) / 1000;
    const why = used > MAX_CORES ? `${used.toFixed(1)} cores busy` : after.spin > 1.4 ? `spin after ${(after.spin * 30).toFixed(0)} ms for 30 ms` : steal > 0.01 ? `${(steal * 100).toFixed(1)}% steal` : "";
    const lines = (r.stdout ?? "").split("\n");
    runs.push({ pass: r.status === 0, notJudged: !!why, why, used, steal, secs, fails: lines.filter((l) => /^\s*FAIL/.test(l)), info: lines.filter((l) => /^\s*(SKIP|PASS|FAIL)?\s*\w*:?.*(median|samples)/.test(l)) });
  }
  const judged = runs.filter((x) => !x.notJudged), failed = judged.filter((x) => !x.pass);
  for (const [i, x] of runs.entries()) console.log(`   run ${i + 1}: ${x.notJudged ? "NOT JUDGED (" + x.why + ")" : x.pass ? "pass" : "FAIL"}  ${x.used.toFixed(1)} cores busy, steal ${(x.steal * 100).toFixed(2)}%, ${x.secs.toFixed(0)} s${x.pass ? "" : "\n      " + x.fails.join("\n      ")}`);
  if (judged.length < need) { unjudged++; console.log(`NOT JUDGED  ${name}: ${judged.length} quiet runs of ${need} needed`); continue; }
  const ok = failed.length <= (SELF_SAMPLED.has(name) ? 0 : 1);
  console.log(`${ok ? "PASS" : "FAIL"}  ${name}  (${judged.length - failed.length} of ${judged.length} judged runs passed)`);
  if (!ok) bad++;
}
console.log(bad ? `${bad} of ${SUITES.length} quiet suites FAILED (judged only in quiet windows)` : unjudged ? `${unjudged} of ${SUITES.length} could not be judged (nothing failed in a quiet window)` : `all ${SUITES.length} quiet suites pass`);
process.exit(bad ? 1 : unjudged ? 3 : 0);

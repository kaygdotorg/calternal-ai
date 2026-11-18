// The quiet list: the timing-sensitive suites, run ALONE and in turn, always judged (CAL_QUIET=1), after waiting for the machine to be quiet. The gate records these numbers and says "SKIP judgement" when the machine is busy
// (a perf check that goes red whenever another lane builds gets ignored; one that never judges proves nothing), so this is where they are judged: run it by itself before a merge. Usage: `npm run build -w apps/web && npm run e2e:quiet -w apps/web`.
import { spawnSync } from "node:child_process";
import { busyNow } from "./lib.mjs";
const SUITES = ["clarity-perf", "canvas", "composer-fold", "switch", "phone-nav", "sidebar-pull", "palette"];
const wait = Number(process.env.CAL_QUIET_WAIT_S ?? 600);
const t0 = Date.now(); let b = busyNow();
while (b.busy && (Date.now() - t0) / 1000 < wait) { console.log(`waiting for the machine to be quiet (${b.why})`); await new Promise((r) => setTimeout(r, 15000)); b = busyNow(); }
if (b.busy) console.log(`WARNING  the machine is still busy (${b.why}): the verdicts below are not trustworthy; run again when other work has finished`);
let bad = 0;
for (const name of SUITES) {
  const args = name === "palette-focus" ? ["12"] : [];
  const r = spawnSync("node", ["e2e/with-preview.mjs", `e2e/${name}.mjs`, ...args], { env: { ...process.env, CAL_QUIET: "1" }, encoding: "utf8", timeout: 300000 });
  const fails = (r.stdout ?? "").split("\n").filter((l) => /^\s*FAIL/.test(l));
  console.log(`${r.status === 0 ? "PASS" : "FAIL"}  ${name}${fails.length ? "\n   " + fails.join("\n   ") : ""}`);
  if (r.status !== 0) bad++;
}
console.log(bad ? `${bad} of ${SUITES.length} quiet suites FAILED` : `all ${SUITES.length} quiet suites pass`);
process.exit(bad ? 1 : 0);

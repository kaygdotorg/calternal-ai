// Runs the browser suites in order and records what they say: pass and fail counts per suite and every TABLE line (the performance and scale numbers) into docs/reviews/renderer-web-e2e-results.md with the commit, the date
// and the machine. Needs `npm run harness:build`, the step 0 host (for the host suites) and CAL_CHROME where Playwright's own browser is not installed. Timings on a shared VM are recorded, not judged.
// Usage: node scripts/run-e2e.mjs [suite ...]   (default: all)
import { spawnSync } from "node:child_process";
import { writeFileSync } from "node:fs";
import { hostname } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const here = join(dirname(fileURLToPath(import.meta.url)), "..");
const ALL = [["keys", []], ["fixtures", []], ["components", []], ["actions", []], ["bulk", []], ["reading", []], ["images", []], ["perf", []], ["scale-notes", ["notes"]], ["scale-threads", ["threads"]]];
const want = process.argv.slice(2);
const git = spawnSync("git", ["rev-parse", "--short", "HEAD"], { cwd: here, encoding: "utf8" }).stdout.trim();
const rows = [], tables = []; let failed = 0;
for (const [name, args] of ALL.filter(([n]) => !want.length || want.includes(n))) {
  const script = name.startsWith("scale") ? "scale" : name;
  const r = spawnSync("node", [join(here, `e2e/${script}.mjs`), ...args], { cwd: here, encoding: "utf8", timeout: 600000 });
  const out = `${r.stdout}\n${r.stderr}`;
  const pass = (out.match(/^PASS /gm) ?? []).length, fail = (out.match(/^FAIL /gm) ?? []).length;
  const crashed = r.status !== 0 && fail === 0;
  rows.push(`| ${name} | ${pass} | ${fail}${crashed ? " (crashed: no result)" : ""} | ${r.status === 0 ? "ok" : "exit " + r.status} |`);
  if (r.status !== 0) failed++;
  for (const l of out.split("\n")) if (l.startsWith("TABLE")) tables.push(`${name}: ${l.replace(/^TABLE \| /, "")}`);
}
const md = `# Web renderer browser suites (${new Date().toISOString().slice(0, 10)})\n\nCommit \`${git}\` on ${hostname()} (a shared VM: timings are recorded, not judged; judge on the perf VMs).\n\n| suite | pass | fail | exit |\n|---|---|---|---|\n${rows.join("\n")}\n\n## Performance and scale lines\n\n${tables.map((t) => "- " + t).join("\n") || "(none)"}\n`;
writeFileSync(join(here, "../../docs/reviews/renderer-web-e2e-results.md"), md);
console.log(md);
process.exit(failed ? 1 : 0);

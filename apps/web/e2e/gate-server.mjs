// The server-mode suites: the built app against a real `calternal serve` on a disposable installation (virtual passkey, temporary roots, torn down by each suite): sign-up with Threads AND Notes ready on first load and a warm reload, the pickers and message search, plugin job prompts in the Inbox, requests, readiness, steps, notes, people, machines, sign-in, accessibility.
// Runs the suites one after another (each starts its own server) on free ports, each under a timeout, one line per suite, exit 1 on any failure, headless, no timing judgements (CAL_GATE=1 keeps the performance checks from judging).
// Usage: `CALTERNAL_SERVER_BIN=/path/to/calternal npm run e2e:server -w apps/web` (the binary must be built with `--features test-plugins`, which the plugin-prompt suite needs; run it on the machine where the server is built, dev-01), or without the variable it builds the server itself (`cargo build -p calternal-server --bin calternal --features test-plugins`) and the app. `dist/` is built when it is missing. Name suites to run only those: `npm run e2e:server -w apps/web -- startup-ready`.
import { execFileSync, spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { createServer } from "node:net";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..", "..", "..");
const free = () => new Promise((res, rej) => { const s = createServer(); s.listen(0, "127.0.0.1", () => { const { port } = s.address(); s.close(() => res(port)); }); s.on("error", rej); });

/** Suite, the environment variable that holds its port, extra arguments, timeout in seconds. */
const SUITES = [
  ["startup-ready", "CAL_READY_UP_PORT"], ["lookups-server", "CAL_LOOKUP_PORT"], ["job-prompts-server", "CAL_JOB_PORT"], ["requests-server", "CAL_REQ_PORT"], ["readiness-server", "CAL_READY_PORT"], ["steps-server", "CAL_STEPS_PORT"], ["notes-server", "CAL_NOTES_PORT"],
  ["sign-in", "CAL_SIGNIN_PORT"], ["a11y-server", "CAL_A11Y_PORT"], ["people-server", "CAL_PEOPLE_PORT"], ["machines-server", "CAL_MACH_PORT"], ["served-by-server", "CAL_SERVED_UNUSED"],
].map(([name, port, args = [], timeout = 300]) => ({ name, port, args, timeout }));

let bin = process.env.CALTERNAL_SERVER_BIN;
if (!bin) {
  console.log("building the server ...");
  execFileSync("cargo", ["build", "-p", "calternal-server", "--bin", "calternal", "--features", "test-plugins"], { cwd: root, stdio: "inherit" });
  bin = join(process.env.CARGO_TARGET_DIR ?? join(root, "target"), "debug", "calternal");
}
if (!existsSync(bin)) { console.error(`no server binary at ${bin}`); process.exit(2); }
if (!existsSync(join(here, "..", "dist", "index.html"))) { console.log("building the app ..."); execFileSync("npm", ["run", "build"], { cwd: join(here, ".."), stdio: "inherit" }); }

const only = process.argv.slice(2);
let failed = 0;
for (const s of SUITES.filter((x) => !only.length || only.includes(x.name))) {
  const t0 = Date.now();
  const code = await new Promise(async (resolve) => {
    const p = spawn("node", [`e2e/${s.name}.mjs`, ...s.args], { cwd: join(here, ".."), env: { ...process.env, CAL_BIN: bin, CAL_GATE: "1", CAL_DIST: join(here, "..", "dist"), CAL_PLUGINS: join(root, "plugins"), CAL_TEST_PLUGINS: join(root, "tests", "plugins"), [s.port]: String(await free()) }, detached: true });
    let out = ""; p.stdout.on("data", (d) => { out += d; }); p.stderr.on("data", (d) => { out += d; });
    const timer = setTimeout(() => { try { process.kill(-p.pid, "SIGKILL"); } catch { /* gone */ } out += `\n(timed out after ${s.timeout} s)`; }, s.timeout * 1000);
    p.on("close", (c) => { clearTimeout(timer); const ok = c === 0; console.log(`${ok ? "pass" : "FAIL"}  ${s.name.padEnd(16)} ${((Date.now() - t0) / 1000).toFixed(1).padStart(6)} s${ok ? "" : `  (exit ${c})`}`); if (!ok) console.log(out.trim().split("\n").slice(-25).map((l) => `      ${l}`).join("\n")); resolve(c); });
  });
  if (code !== 0) failed++;
}
console.log(`\n${failed ? "FAILED" : "ok"}: ${failed} failing`);
process.exit(failed ? 1 : 0);

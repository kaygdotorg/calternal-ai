// The server-mode suites: the built app against a real `calternal serve` on a disposable installation (sign-in, accessibility of the setup screens, people, machines, start-up).
// Builds what it needs (the server binary, `dist/`), then runs the suites one after another (each starts its own server) on free ports, each under a timeout, one line per suite, exit 1 on a failure.
// Usage: `npm run e2e:server -w apps/web`. Slower than `e2e:gate` (a Rust build the first time); not part of the every-merge gate unless the merge touches server mode.
import { execFileSync, spawn } from "node:child_process";
import { createServer } from "node:net";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..", "..", "..");
const free = () => new Promise((res, rej) => { const s = createServer(); s.listen(0, "127.0.0.1", () => { const { port } = s.address(); s.close(() => res(port)); }); s.on("error", rej); });

/** Suite, the environment variable that holds its port, extra arguments, timeout in seconds. */
const SUITES = [
  ["sign-in", "CAL_SIGNIN_PORT"], ["a11y-server", "CAL_A11Y_PORT"], ["people-server", "CAL_PEOPLE_PORT"], ["machines-server", "CAL_MACH_PORT"], ["startup-server", "CAL_START_PORT", ["3"]], ["served-by-server", "CAL_SERVED_UNUSED"], ["live-real", "CAL_LIVE_REAL_UNUSED", [], 420],
].map(([name, port, args = [], timeout = 240]) => ({ name, port, args, timeout }));

console.log("building the server and the app ...");
execFileSync("cargo", ["build", "-p", "calternal-server", "--bin", "calternal"], { cwd: root, stdio: "inherit" });
execFileSync("npm", ["run", "build"], { cwd: join(here, ".."), stdio: "inherit" });
const bin = join(process.env.CARGO_TARGET_DIR ?? join(root, "target"), "debug", "calternal");

const only = process.argv.slice(2);
let failed = 0;
for (const s of SUITES.filter((x) => !only.length || only.includes(x.name))) {
  const t0 = Date.now();
  const code = await new Promise(async (resolve) => {
    const p = spawn("node", [`e2e/${s.name}.mjs`, ...s.args], { cwd: join(here, ".."), env: { ...process.env, CAL_BIN: bin, [s.port]: String(await free()) }, detached: true });
    let out = ""; p.stdout.on("data", (d) => { out += d; }); p.stderr.on("data", (d) => { out += d; });
    const timer = setTimeout(() => { try { process.kill(-p.pid, "SIGKILL"); } catch { /* gone */ } out += `\n(timed out after ${s.timeout} s)`; }, s.timeout * 1000);
    p.on("close", (c) => { clearTimeout(timer); const ok = c === 0; console.log(`${ok ? "pass" : "FAIL"}  ${s.name.padEnd(16)} ${((Date.now() - t0) / 1000).toFixed(1).padStart(6)} s${ok ? "" : `  (exit ${c})`}`); if (!ok) console.log(out.trim().split("\n").slice(-25).map((l) => `      ${l}`).join("\n")); resolve(c); });
  });
  if (code !== 0) failed++;
}
console.log(`\n${failed ? "FAILED" : "ok"}: ${failed} failing`);
process.exit(failed ? 1 : 0);

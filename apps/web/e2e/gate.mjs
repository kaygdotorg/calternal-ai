// The merge gate for UI work: the deterministic headless suites on fixtures that guard decided behaviour. Serves the built app (`dist/`) once on a free port, runs each suite against it
// (`CAL_URL`), a few at a time, each under its own timeout, prints one line per suite and exits 1 if any failed. Usage: `npm run build -w apps/web && npm run e2e:gate -w apps/web`
// (or `node e2e/gate.mjs [suite ...]` for some). Suites that need a server build are not here (`e2e:server`); the reasons for any excluded suite are in apps/web/e2e/README.md.
import { spawn } from "node:child_process";
import { createServer } from "node:net";
import { cpus } from "node:os";
import { existsSync, readFileSync } from "node:fs";

/** The suites: file, extra arguments, and a timeout in seconds (default 150; `a11y` walks every screen in four device-and-scheme combinations and takes about two minutes alone). */
const SUITES = [
  ["first-visit"], ["shell-audit"], ["flat-sidebars"], ["resize"], ["sidebar-scroll"], ["a11y", [], 300], ["text-parity"], ["palette"],
  ["palette-focus", ["12"]], ["interact"], ["phone-nav"], ["settings-pages"], ["focus-ring"], ["swipe-rows"], ["morph"], ["states"], ["new-thread"],
].map(([name, args = [], timeout = 150]) => ({ name, args, timeout }));

const free = () => new Promise((res, rej) => { const s = createServer(); s.listen(0, "127.0.0.1", () => { const { port } = s.address(); s.close(() => res(port)); }); s.on("error", rej); });

try { readFileSync("dist/index.html"); } catch { console.error("No dist/ yet: run `npm run build -w apps/web` first."); process.exit(2); }
// The suites look at the default photographs: a build without them would fail for the wrong reason, so say so first.
const wanted = JSON.parse(readFileSync("wallpapers.json", "utf8")).filter((w) => !existsSync(`dist/wallpapers/${w.id}.jpg`));
if (wanted.length) { console.error(`dist/ lacks ${wanted.length} default wallpapers (the image host refused this machine). Copy ~/.cache/calternal-ai/wallpapers from a machine that has them (dev-02), run \`npm run build\` again, then retry.`); process.exit(2); }
const only = process.argv.slice(2);
const list = only.length ? SUITES.filter((s) => only.includes(s.name)) : SUITES;
const port = await free();
const url = `http://127.0.0.1:${port}`;
const server = spawn("npx", ["vite", "preview", "--port", String(port), "--strictPort", "--host", "127.0.0.1"], { stdio: "ignore", detached: true });
const stop = () => { try { process.kill(-server.pid); } catch { /* gone */ } };
process.on("exit", stop);
for (let i = 0; i < 120; i++) { try { if ((await fetch(url)).ok) break; } catch { /* not yet */ } await new Promise((r) => setTimeout(r, 250)); }

/** Runs one suite; resolves to its result line. Output is kept and shown only for a failure. */
function run(s) {
  return new Promise((resolve) => {
    const t0 = Date.now();
    const p = spawn("node", [`e2e/${s.name}.mjs`, ...s.args], { env: { ...process.env, CAL_URL: url, CAL_GATE: "1" }, detached: true });
    let out = "";
    p.stdout.on("data", (d) => { out += d; }); p.stderr.on("data", (d) => { out += d; });
    let timedOut = false;
    const timer = setTimeout(() => { timedOut = true; try { process.kill(-p.pid, "SIGKILL"); } catch { /* gone */ } }, s.timeout * 1000);
    p.on("close", (code) => {
      clearTimeout(timer);
      const secs = ((Date.now() - t0) / 1000).toFixed(1);
      const ok = code === 0 && !timedOut;
      console.log(`${ok ? "pass" : "FAIL"}  ${s.name.padEnd(15)} ${secs.padStart(6)} s${timedOut ? `  (timed out after ${s.timeout} s)` : ok ? "" : `  (exit ${code})`}`);
      if (!ok) console.log(out.trim().split("\n").slice(-25).map((l) => `      ${l}`).join("\n"));
      resolve(ok);
    });
  });
}

// Chromium suites measure timing and focus: more than this at once on a small machine makes them time-dependent.
const width = Math.max(1, Math.min(+(process.env.GATE_JOBS ?? 4), Math.floor(cpus().length / 2)));
const queue = [...list];
const results = [];
const t0 = Date.now();
await Promise.all(Array.from({ length: width }, async () => { for (let s; (s = queue.shift()); ) results.push(await run(s)); }));
stop();
const failed = results.filter((r) => !r).length;
console.log(`\n${failed ? "FAILED" : "ok"}: ${results.length - failed}/${results.length} suites in ${((Date.now() - t0) / 1000).toFixed(0)} s`);
process.exit(failed ? 1 : 0);

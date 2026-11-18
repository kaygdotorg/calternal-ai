// The merge gate for UI work: the deterministic headless suites on fixtures that guard decided behaviour. Serves the built app (`dist/`) once on a free port, runs each suite against it
// (`CAL_URL`), a few at a time, each under its own timeout, prints one line per suite and exits 1 if any failed. Usage: `npm run build -w apps/web && npm run e2e:gate -w apps/web`
// (or `node e2e/gate.mjs [suite ...]` for some). Suites that need a server build are not here (`e2e:server`); the reasons for any excluded suite are in apps/web/e2e/README.md.
import { spawn } from "node:child_process";
import { createServer } from "node:net";
import { cpus } from "node:os";
import { appendFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

/** The suites: file, extra arguments, a timeout in seconds (default 240: it only bounds a hang, and dev VMs are shared), the name shown and extra environment . The accessibility walk is four suites (device and scheme) so that no one suite runs long. */
const SUITES = [
  ["a11y", ["desktop", "light"], 300, "a11y desktop light"], ["a11y", ["desktop", "dark"], 300, "a11y desktop dark"], ["a11y", ["phone", "light"], 300, "a11y phone light"], ["a11y", ["phone", "dark"], 300, "a11y phone dark"],
  ["first-visit"], ["shell-audit"], ["flat-sidebars"], ["resize"], ["sidebar-scroll"], ["text-parity"], ["palette"],
  ["palette-focus", ["12"]], ["interact"], ["phone-nav"], ["settings-pages"], ["focus-ring"], ["swipe-rows"], ["morph"], ["states"], ["new-thread"], ["keyboard-threads"], ["swipe"], ["sidebar-pull"], ["switch"], ["safe-areas"], ["seam"], ["section-spacing"], ["floating-material"], ["onboarding"], ["app-switcher"], ["inset-sheet"], ["phone-menus"], ["skills-automations"], ["composer"], ["composer-fold"], ["canvas-audit"], ["clarity-perf"], ["sidebar-top"], ["message-actions"], ["turn-ui"], ["chips"], ["attachments"], ["canvas"], ["signin"], ["phone-ui"], ["tool-deck"], ["provider-mark"], ["thread-search"], ["message-edit"], ["snooze"],
].map(([name, args = [], timeout = 240, label = name, env = {}]) => ({ name, args, timeout, label, env }));

const free = () => new Promise((res, rej) => { const s = createServer(); s.listen(0, "127.0.0.1", () => { const { port } = s.address(); s.close(() => res(port)); }); s.on("error", rej); });

try { readFileSync("dist/index.html"); } catch { console.error("No dist/ yet: run `npm run build -w apps/web` first."); process.exit(2); }
// The suites look at the default photographs: a build without them would fail for the wrong reason, so say so first.
const wanted = JSON.parse(readFileSync("wallpapers.json", "utf8")).filter((w) => !existsSync(`dist/wallpapers/${w.id}.jpg`));
if (wanted.length) { console.error(`dist/ lacks ${wanted.length} default wallpapers (the image host refused this machine). Copy them from a machine that has them, build again, then retry:\n  rsync -a ubuntu@calternal-dev-02-std:.cache/calternal-ai/wallpapers/ ~/.cache/calternal-ai/wallpapers/ && npm run build -w apps/web`); process.exit(2); }
// Every run keeps a log per suite and a summary outside the checkout (a failure is never lost with a deleted folder): CAL_GATE_LOGS, else <tmp>/calternal-e2e-gate/<timestamp>/.
const logs = process.env.CAL_GATE_LOGS ?? join(tmpdir(), "calternal-e2e-gate", new Date().toISOString().replace(/[:.]/g, "-"));
mkdirSync(logs, { recursive: true });
const summary = join(logs, "summary.txt");
const only = process.argv.slice(2);
const list = only.length ? SUITES.filter((s) => only.includes(s.name) || only.includes(s.label)) : SUITES;
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
    const p = spawn("node", [`e2e/${s.name}.mjs`, ...s.args], { env: { ...process.env, CAL_URL: url, CAL_GATE: "1", ...s.env }, detached: true });
    let out = "";
    p.stdout.on("data", (d) => { out += d; }); p.stderr.on("data", (d) => { out += d; });
    let timedOut = false;
    const timer = setTimeout(() => { timedOut = true; try { process.kill(-p.pid, "SIGKILL"); } catch { /* gone */ } }, s.timeout * 1000);
    p.on("close", (code) => {
      clearTimeout(timer);
      const secs = ((Date.now() - t0) / 1000).toFixed(1);
      const ok = code === 0 && !timedOut;
      const line = `${ok ? "pass" : "FAIL"}  ${s.label.padEnd(19)} ${secs.padStart(6)} s${timedOut ? `  (timed out after ${s.timeout} s)` : ok ? "" : `  (exit ${code})`}`;
      console.log(line);
      writeFileSync(join(logs, `${s.label.replace(/\s+/g, "-")}.log`), out);
      // The failing check, in one line: the first FAIL line of the output, or the last line (an exception).
      appendFileSync(summary, line + (ok ? "" : `\n      ${out.split("\n").find((l) => /^\s*FAIL/.test(l))?.trim() ?? out.trim().split("\n").filter(Boolean).slice(-1)[0] ?? ""}`) + "\n");
      if (!ok) console.log(out.trim().split("\n").slice(-25).map((l) => `      ${l}`).join("\n"));
      resolve(ok);
    });
  });
}

// Chromium suites measure timing and focus: more than this at once on a small machine makes them time-dependent.
const width = Math.max(1, Math.min(+(process.env.GATE_JOBS ?? 6), Math.floor(cpus().length / 2)));
const queue = [...list];
const results = [];
const t0 = Date.now();
await Promise.all(Array.from({ length: width }, async () => { for (let s; (s = queue.shift()); ) results.push(await run(s)); }));
stop();
const failed = results.filter((r) => !r).length;
console.log(`\n${failed ? "FAILED" : "ok"}: ${results.length - failed}/${results.length} suites in ${((Date.now() - t0) / 1000).toFixed(0)} s`);
if (failed) console.log(`logs of every suite and a summary: ${logs}`);
process.exit(failed ? 1 : 0);

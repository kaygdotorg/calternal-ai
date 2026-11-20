#!/usr/bin/env node
// The plugin author's one command: type-check, bundle the worker, emit the
// package and verify it against the pinned contract. Also lists the limits.
import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { mkdtemp, rm } from "node:fs/promises";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join, relative, resolve } from "node:path";
import { parseArgs } from "node:util";

const require = createRequire(import.meta.url);
const usage = `Usage:
  calternal-plugin build [<plugin.ts> [<output-dir>]] [--worker <file>] [--no-check] [--json]
  calternal-plugin limits [--json]

build   Type-checks the package (tsc --noEmit, when tsconfig.json exists), bundles
        a TypeScript worker with esbuild, emits the manifest with pinned assets and
        verifies it with the pinned contract tools. Defaults: src/plugin.ts, dist,
        and src/worker.ts when it exists. A .js or .mjs worker is used as is.
limits  Lists every numeric limit an author can hit, with its contract source.`;

class UsageError extends Error {}

const width = () => Math.max(40, Math.min(process.stdout.columns || 100, 120));
const fit = (text, room) => (text.length <= room ? text : "…" + text.slice(text.length - room + 1));
const count = (n, word) => `${n} ${word}${n === 1 ? "" : "s"}`;
const kib = bytes => (bytes < 1024 ? `${bytes} B` : `${(bytes / 1024).toFixed(1)} KiB`);

function table(rows) {
  const left = Math.max(...rows.map(([label]) => label.length));
  return rows.map(([label, value]) => `  ${label.padEnd(left)}  ${fit(value, width() - left - 4)}`).join("\n");
}

async function bundle(entry) {
  const { build } = await import("esbuild");
  const directory = await mkdtemp(join(tmpdir(), "calternal-plugin-worker-"));
  const outfile = join(directory, "worker.mjs");
  await build({ entryPoints: [entry], bundle: true, platform: "node", format: "esm", target: "node22", outfile, logLevel: "silent" });
  return { outfile, cleanup: () => rm(directory, { recursive: true, force: true }) };
}

function typecheck(directory) {
  if (!existsSync(join(directory, "tsconfig.json"))) return "skipped (no tsconfig.json)";
  const tsc = require.resolve("typescript/bin/tsc", { paths: [directory] });
  try {
    execFileSync(process.execPath, [tsc, "--noEmit", "-p", join(directory, "tsconfig.json")], { encoding: "utf8", stdio: "pipe" });
  } catch (error) {
    throw new Error(`TypeScript found errors:\n${(error.stdout || error.stderr || "").trim()}\nFix them, or pass --no-check to build anyway.`);
  }
  return "passed";
}

async function buildCommand(positionals, values) {
  if (positionals.length > 2) throw new UsageError("build takes at most a plugin source and an output directory.");
  const directory = process.cwd();
  const source = resolve(positionals[0] ?? "src/plugin.ts");
  const output = resolve(positionals[1] ?? "dist");
  if (!existsSync(source)) throw new UsageError(`No plugin source at ${relative(directory, source) || source}. Pass its path, or create src/plugin.ts.`);
  const workerSource = values.worker ? resolve(values.worker) : existsSync(resolve("src/worker.ts")) && !positionals[0] ? resolve("src/worker.ts") : undefined;
  const types = values.check ? typecheck(directory) : "skipped (--no-check)";
  const bundled = workerSource && /\.[cm]?ts$/.test(workerSource) ? await bundle(workerSource) : undefined;
  try {
    const { buildPackage } = await import("./build-package.mjs");
    const { CONTRACT_REVISION } = await import("../dist/generated/broker-methods.js");
    const { manifest, assets, report } = await buildPackage(source, output, bundled?.outfile ?? workerSource);
    const result = {
      package: manifest.id, version: manifest.version, output,
      operations: manifest.operations.length, kinds: manifest.kinds.length, apps: manifest.apps.length,
      worker: manifest.worker ? { path: manifest.worker.path, bytes: assets.find(asset => asset.path === manifest.worker.path).bytes,
        source: workerSource && relative(directory, workerSource), bundled: Boolean(bundled) } : null,
      assets, contract: CONTRACT_REVISION, checks: { types, contract: "passed" }, report: report.trim().split("\n").filter(Boolean),
    };
    if (values.json) return JSON.stringify(result, null, 2);
    return [
      `Built ${result.package} ${result.version} into ${relative(directory, output) || "."}`,
      table([
        ["Manifest", `${count(result.operations, "operation")}, ${count(result.kinds, "kind")}, ${count(result.apps, "app")}`],
        ...(result.worker ? [["Worker", `${kib(result.worker.bytes)} ${result.worker.bundled ? `bundled from ${result.worker.source}` : "as given"}`]] : []),
        ["Assets", `${count(assets.length, "file")} pinned by SHA-256`],
        ["Types", types],
        ["Contract", `${CONTRACT_REVISION}: manifest, templates, pins and cross-references passed`],
      ]),
      "Next: run your tests against the fake host from @calternal/plugin-sdk/testing.",
    ].join("\n");
  } finally {
    await bundled?.cleanup();
  }
}

async function limitsCommand(values) {
  const { LIMIT_SOURCES } = await import("../dist/limits.js");
  if (values.json) return JSON.stringify(LIMIT_SOURCES, null, 2);
  const rows = Object.entries(LIMIT_SOURCES).map(([name, { value, unit }]) => [name, `${value} ${unit}`]);
  return [`${rows.length} limits from the pinned contract. Import them as LIMITS from @calternal/plugin-sdk.`, table(rows),
    "Sources: calternal-plugin limits --json"].join("\n");
}

try {
  const { values, positionals } = parseArgs({
    allowPositionals: true,
    options: { json: { type: "boolean", default: false }, worker: { type: "string" }, check: { type: "boolean", default: true }, help: { type: "boolean", short: "h" } },
    allowNegative: true,
  });
  const [command, ...rest] = positionals;
  if (values.help || !command) console.log(usage);
  else if (command === "build") console.log(await buildCommand(rest, values));
  else if (command === "limits") console.log(await limitsCommand(values));
  else throw new UsageError(`Unknown command ${command}.`);
} catch (error) {
  if (error instanceof UsageError || error?.code === "ERR_PARSE_ARGS_UNKNOWN_OPTION") console.error(`${error.message}\n\n${usage}`);
  else console.error(error.message);
  process.exitCode = 1;
}

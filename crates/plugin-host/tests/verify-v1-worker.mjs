/**
 * Verify the committed v1 worker against the updated SDK before server cutover.
 * Copy its source from a git object to the default input path, then run from the
 * repository root on a dev VM with npm dependencies installed. Does one bundle,
 * one bounded disposable Node process and fixture I/O; it never edits a server pin.
 */
import { createRequire } from "node:module";
const { build } = createRequire(new URL("../../../plugins/notes-insights/package.json", import.meta.url))("esbuild");
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import assert from "node:assert/strict";

const source = process.argv[2] ?? "plugins/notes-insights/src/legacy-worker-verification.ts";
const worker = "plugins/notes-insights/dist/legacy-worker-verification.js";
await build({ entryPoints: [source], outfile: worker, bundle: true, platform: "node", format: "esm", target: "node22" });
const run = spawnSync(process.execPath, [worker], {
  input: readFileSync("contract/fixtures/worker-request.json"),
  encoding: "utf8",
  timeout: 3000,
  maxBuffer: 256 * 1024,
});
assert.equal(run.status, 0, run.stderr);
const response = JSON.parse(run.stdout);
assert.equal(response.protocol_version, 1);
assert.equal(response.invocation_id, "fixture-1");
assert.equal(response.ok, true);
assert.deepEqual(response.data.stats, { words: 2, characters: 12, lines: 1 });
assert.equal(response.data.view.genui_version, 1);
console.log("committed v1 worker against updated SDK: PASS (statistics and GenUI)");

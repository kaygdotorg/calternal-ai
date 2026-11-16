import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import test from "node:test";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const distDir = resolve(packageRoot, "dist");
const workerPath = resolve(distDir, "worker.js");
const fixturePath = resolve(packageRoot, "../../contract/fixtures/worker-request.json");

function invokeWorker(input, { args = [], timeout = 3000 } = {}) {
  return new Promise((resolvePromise, reject) => {
    const child = spawn(process.execPath, ["--permission", `--allow-fs-read=${distDir}`, ...args, workerPath], {
      cwd: distDir,
      env: { LANG: "C.UTF-8" },
      stdio: ["pipe", "pipe", "pipe"],
    });
    const stdout = [];
    const stderr = [];
    child.stdout.on("data", (chunk) => stdout.push(chunk));
    child.stderr.on("data", (chunk) => stderr.push(chunk));
    const timer = setTimeout(() => {
      child.kill("SIGKILL");
      reject(new Error("worker did not exit"));
    }, timeout);
    child.once("error", (error) => {
      clearTimeout(timer);
      reject(error);
    });
    child.once("close", (code, signal) => {
      clearTimeout(timer);
      resolvePromise({
        code,
        signal,
        stdout: Buffer.concat(stdout),
        stderr: Buffer.concat(stderr),
      });
    });
    child.stdin.end(input);
  });
}

test("compiled worker reads one JSON object to EOF and writes one protocol object", async () => {
  const request = await readFile(fixturePath);
  const result = await invokeWorker(request);
  assert.equal(result.code, 0, result.stderr.toString("utf8"));
  assert.equal(result.signal, null);
  assert.equal(result.stderr.byteLength, 0);
  const response = JSON.parse(result.stdout.toString("utf8"));
  assert.equal(response.protocol_version, 1);
  assert.equal(response.invocation_id, "fixture-1");
  assert.equal(response.ok, true);
  assert.deepEqual(response.data.stats, { words: 2, characters: 12, lines: 1 });
  assert.equal(response.data.view.genui_version, 1);
  assert.equal(response.data.view.root.type, "stack");
  assert.equal(response.data.view.root.children.length, 3);
  assert.throws(() => JSON.parse(`${result.stdout.toString("utf8")} {}`));
});

test("worker rejects malformed or oversized input without protocol details on stderr", async () => {
  const malformed = await invokeWorker('{"protocol_version":1} {}');
  assert.notEqual(malformed.code, 0);
  assert.equal(malformed.stdout.byteLength, 0);
  assert.equal(malformed.stderr.toString("utf8"), "notes-insights worker protocol failure\n");

  const oversized = await invokeWorker(Buffer.alloc(2 * 1024 * 1024 + 1));
  assert.notEqual(oversized.code, 0);
  assert.equal(oversized.stdout.byteLength, 0);
  assert.equal(oversized.stderr.toString("utf8"), "notes-insights worker protocol failure\n");
});

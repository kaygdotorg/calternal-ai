import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import test from "node:test";

import {
  ContractValidationError,
  parseOperationRequest,
  validateGenUiView,
  validateOperationRequest,
  validateManifestV1,
} from "../dist/index.js";

const testDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(testDir, "../../..");
const fixture = async (relativePath) => JSON.parse(await readFile(resolve(repoRoot, relativePath), "utf8"));
const expectInvalid = (fn, pattern) => assert.throws(fn, (error) => {
  assert.ok(error instanceof ContractValidationError);
  if (pattern) assert.match(error.message, pattern);
  return true;
});

test("operation request fixture corpus has the same accept/reject outcomes", async () => {
  const cases = await fixture("contract/fixtures/request-cases.json");
  for (const item of cases) {
    if (item.valid) assert.doesNotThrow(() => validateOperationRequest(item.value), item.name);
    else expectInvalid(() => validateOperationRequest(item.value));
  }
  assert.deepEqual(await parseOperationRequest(await readFile(resolve(repoRoot, "contract/fixtures/operation-request.json"))), cases[0].value);
  expectInvalid(() => parseOperationRequest("{\"request_id\":\"one\"} {}"), /malformed/);
  expectInvalid(() => parseOperationRequest(Buffer.alloc(2 * 1024 * 1024 + 1)), /exceeds/);
});

test("repo Notes manifest satisfies manifest v1", async () => {
  const notes = await fixture("plugins/calternal-notes/manifest.json");
  assert.equal(validateManifestV1(notes).id, "calternal-notes");
});

test("GenUI views are versioned, bounded generated-UI source", () => {
  const view = { genui_version: 2, source: 'Row([Metric("Words", "3")])' };
  assert.deepEqual(validateGenUiView(view), view);
  expectInvalid(() => validateGenUiView({ ...view, genui_version: 1 }), /version/);
  expectInvalid(() => validateGenUiView({ ...view, extra: true }), /unknown field/);
  expectInvalid(() => validateGenUiView({ genui_version: 2, source: "x".repeat(64 * 1024 + 1) }), /64 KiB/);
  // The retired JSON tree is not a view.
  expectInvalid(() => validateGenUiView({ genui_version: 1, root: { type: "text", id: "t", text: "x" } }));
});

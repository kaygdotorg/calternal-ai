import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import test from "node:test";

import {
  ContractValidationError,
  parseOperationRequest,
  parseWorkerRequest,
  parseWorkerResponse,
  validateExtensionView,
  validateGenUiView,
  validateOperationRequest,
  validatePluginManifest,
  validateWorkerRequest,
  validateWorkerResponse,
} from "../dist/index.js";

const testDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(testDir, "../../..");
const fixture = async (relativePath) => JSON.parse(await readFile(resolve(repoRoot, relativePath), "utf8"));
const expectInvalid = (fn, pattern) => assert.throws(fn, (error) => {
  assert.ok(error instanceof ContractValidationError);
  if (pattern) assert.match(error.message, pattern);
  return true;
});

test("GenUI fixture corpus has the same accept/reject outcomes", async () => {
  const cases = await fixture("contract/fixtures/genui-cases.json");
  for (const item of cases) {
    if (item.valid) assert.doesNotThrow(() => validateGenUiView(item.value), item.name);
    else expectInvalid(() => validateGenUiView(item.value));
  }
  const validView = await fixture("contract/fixtures/genui.json");
  assert.doesNotThrow(() => validateGenUiView(validView));
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

test("repo Notes and extension manifests satisfy the frozen SDK schema", async () => {
  const notes = await fixture("plugins/notes/manifest.json");
  const insights = await fixture("plugins/notes-insights/manifest.json");
  assert.equal(validatePluginManifest(notes).id, "notes");
  assert.equal(validatePluginManifest(insights).id, "notes-insights");

  expectInvalid(() => validatePluginManifest({ ...insights, browser_code: "alert(1)" }), /unknown field/);
  expectInvalid(() => validatePluginManifest({ ...insights, api_version: 2 }), /plugin API/);
  expectInvalid(() => validatePluginManifest({ ...insights, operations: [{ ...insights.operations[0], mutates: true }] }));
});

test("extension actions must be declared, schema-valid and host-read-only", async () => {
  const insights = await fixture("plugins/notes-insights/manifest.json");
  const resourceId = "11111111-1111-4111-8111-111111111111";
  const getManifest = {
    ...insights,
    operations: [{ id: "notes.get", description: "Read a note", mutates: false }],
  };
  const getView = {
    genui_version: 1,
    root: {
      type: "button",
      id: "read-note",
      label: "Read",
      action: { operation: "notes.get", input: { space_id: "personal", id: resourceId } },
    },
  };
  assert.doesNotThrow(() => validateExtensionView(getView, getManifest));
  assert.doesNotThrow(() => validatePluginManifest({ ...getManifest, views: [{ id: "detail", extension_point: "notes.detail", view: getView }] }));

  const undeclared = structuredClone(getView);
  undeclared.root.action.operation = "notes.history";
  expectInvalid(() => validateExtensionView(undeclared, getManifest), /not declared/);

  const invalidInput = structuredClone(getView);
  invalidInput.root.action.input.untrusted = true;
  expectInvalid(() => validateExtensionView(invalidInput, getManifest), /unknown field/);

  const mutatingManifest = {
    ...insights,
    operations: [{ id: "notes.update", description: "Claimed read-only", mutates: false }],
    views: [{
      id: "detail",
      extension_point: "notes.detail",
      view: {
        genui_version: 1,
        root: {
          type: "button",
          id: "mutate-note",
          label: "Update",
          action: {
            operation: "notes.update",
            input: { space_id: "personal", id: resourceId, base_revision: "rev-1", markdown: "changed" },
          },
        },
      },
    }],
  };
  expectInvalid(() => validatePluginManifest(mutatingManifest), /read-only/);
});

test("GenUI enforces depth, node, text, string-budget and serialized-size limits", () => {
  let deep = { type: "text", id: "leaf", text: "x" };
  for (let i = 0; i < 8; i += 1) deep = { type: "stack", id: `stack-${i}`, children: [deep] };
  expectInvalid(() => validateGenUiView({ genui_version: 1, root: deep }), /depth/);

  const oversizedText = { genui_version: 1, root: { type: "text", id: "long", text: "a".repeat(8193) } };
  expectInvalid(() => validateGenUiView(oversizedText), /8 KiB/);

  const largeStrings = {
    genui_version: 1,
    root: {
      type: "stack",
      id: "many-texts",
      children: Array.from({ length: 5 }, (_, index) => ({ type: "text", id: `text-${index}`, text: "x".repeat(7000) })),
    },
  };
  expectInvalid(() => validateGenUiView(largeStrings), /string budget/);

  const largeKeys = Object.fromEntries(Array.from({ length: 10 }, (_, index) => [`k${index}${"x".repeat(7000)}`, 0]));
  const oversizedSerializedView = {
    genui_version: 1,
    root: {
      type: "button",
      id: "large-keys",
      label: "Run",
      action: { operation: "notes.get", input: largeKeys },
    },
  };
  expectInvalid(() => validateGenUiView(oversizedSerializedView), /64 KiB/);

  let manyNodes = { type: "stack", id: "many", children: [] };
  manyNodes.children = Array.from({ length: 128 }, (_, index) => ({ type: "text", id: `n-${index}`, text: "x" }));
  expectInvalid(() => validateGenUiView({ genui_version: 1, root: manyNodes }), /128 nodes/);
});

test("worker fixtures, strict framing, identity, stats and size limits validate", async () => {
  const requestValue = await fixture("contract/fixtures/worker-request.json");
  const success = await fixture("contract/fixtures/worker-success.json");
  const failure = await fixture("contract/fixtures/worker-failure.json");
  assert.deepEqual(parseWorkerRequest(JSON.stringify(requestValue)), requestValue);
  assert.equal(parseWorkerResponse(JSON.stringify(success), "fixture-1").ok, true);
  assert.equal(parseWorkerResponse(JSON.stringify(failure), "fixture-1").ok, false);

  expectInvalid(() => validateWorkerRequest({ ...requestValue, injected: true }), /unknown field/);
  expectInvalid(() => parseWorkerRequest(Buffer.alloc(2 * 1024 * 1024 + 1)), /exceeds/);
  expectInvalid(() => parseWorkerRequest('{"protocol_version":1} {}'), /malformed/);
  expectInvalid(() => validateWorkerResponse(success, "different-invocation"), /mismatch/);
  expectInvalid(() => parseWorkerResponse(Buffer.alloc(256 * 1024 + 1), "fixture-1"), /exceeds/);
  expectInvalid(() => validateWorkerResponse({
    ...success,
    data: { ...success.data, stats: { ...success.data.stats, words: -1 } },
  }, "fixture-1"), /non-negative/);
  expectInvalid(() => validateWorkerResponse({ ...success, unexpected: "field" }, "fixture-1"), /unknown field/);
});

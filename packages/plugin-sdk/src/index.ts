import type {
  GenUiNode,
  GenUiView,
  OperationRequest,
  PluginManifest,
  WorkerRequest,
  WorkerResponse,
} from "@calternal/contracts";

/** Frozen preview operation/snapshot-worker version; worker-v2 envelopes carry version 2. */
export const PROTOCOL_VERSION = 1 as const;
/** Portable declarative view version supported by the SDK. */
export const GENUI_VERSION = 1 as const;
/** Maximum UTF-8 Markdown size, in bytes (1 MiB). */
export const MAX_DOCUMENT_BYTES = 1024 * 1024;
/** Maximum serialized operation request size, in bytes (2 MiB). */
export const MAX_BODY_BYTES = 2 * 1024 * 1024;
/** Maximum worker standard-input size, in bytes (2 MiB), checked before JSON decoding. */
export const MAX_WORKER_INPUT_BYTES = 2 * 1024 * 1024;
/** Maximum worker standard-output size, in bytes (256 KiB), checked before JSON decoding. */
export const MAX_WORKER_OUTPUT_BYTES = 256 * 1024;
/** Host diagnostic capture budget, in bytes (16 KiB); never include private request content. */
export const MAX_WORKER_STDERR_BYTES = 16 * 1024;
/** Host worker execution timeout, in milliseconds (3 seconds). */
export const WORKER_TIMEOUT_MS = 3000;
/** Maximum serialized portable view size, in UTF-8 bytes (64 KiB). */
export const MAX_GENUI_SERIALIZED_BYTES = 64 * 1024;
/** Combined portable view string-value budget, in UTF-8 bytes (32 KiB). */
export const MAX_GENUI_STRING_BYTES = 32 * 1024;
/** Maximum literal text-node size, in UTF-8 bytes (8 KiB). */
export const MAX_GENUI_TEXT_BYTES = 8 * 1024;
/** Maximum nodes per portable view, including its root (128). */
export const MAX_GENUI_NODES = 128;
/** Maximum portable view depth, counting the root as depth one (8). */
export const MAX_GENUI_DEPTH = 8;

const ID_PATTERN = /^[A-Za-z0-9_.:-]{1,80}$/;
const UUID_PATTERN = /^(?:[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}|[0-9a-f]{32}|\{[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\}|urn:uuid:[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})$/i;
const MUTATING_HOST_OPERATIONS = new Set([
  "notes.create",
  "notes.update",
  "plugins.enable",
  "plugins.disable",
]);

/** Contract violation raised by SDK parsers and validators; contains a safe diagnostic. */
export class ContractValidationError extends Error {
  /** Create a validation failure; callers must supply a diagnostic without credentials or private input. */
  constructor(message: string) {
    super(message);
    this.name = "ContractValidationError";
  }
}

type JsonRecord = Record<string, unknown>;
type StringBudget = { bytes: number };

function fail(message: string): never {
  throw new ContractValidationError(message);
}

function isRecord(value: unknown): value is JsonRecord {
  if (value === null || typeof value !== "object" || Array.isArray(value)) return false;
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}

function ownKeys(value: JsonRecord): string[] {
  if (Object.getOwnPropertySymbols(value).length > 0) fail("symbol properties are not JSON");
  return Object.getOwnPropertyNames(value);
}

function exactKeys(value: JsonRecord, required: readonly string[], optional: readonly string[] = []): void {
  const allowed = new Set([...required, ...optional]);
  const present = ownKeys(value);
  for (const key of present) {
    if (!allowed.has(key)) fail(`unknown field: ${key}`);
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (!descriptor || !("value" in descriptor) || !descriptor.enumerable) fail("non-JSON object property");
  }
  for (const key of required) {
    if (!Object.prototype.hasOwnProperty.call(value, key)) fail(`missing field: ${key}`);
  }
}

function wellFormedUnicode(value: string): boolean {
  for (let index = 0; index < value.length; index += 1) {
    const unit = value.charCodeAt(index);
    if (unit >= 0xd800 && unit <= 0xdbff) {
      const next = value.charCodeAt(index + 1);
      if (!(next >= 0xdc00 && next <= 0xdfff)) return false;
      index += 1;
    } else if (unit >= 0xdc00 && unit <= 0xdfff) {
      return false;
    }
  }
  return true;
}

function utf8Bytes(value: string): number {
  if (!wellFormedUnicode(value)) fail("string contains invalid Unicode");
  return new TextEncoder().encode(value).byteLength;
}

function requireString(value: unknown, message = "expected string"): string {
  if (typeof value !== "string" || !wellFormedUnicode(value)) fail(message);
  return value;
}

function countScalars(value: string): number {
  return Array.from(value).length;
}

function validateId(value: unknown): asserts value is string {
  if (typeof value !== "string" || !ID_PATTERN.test(value)) {
    fail("identifier must be 1..80 ASCII letters, digits, _, ., :, -");
  }
}

function validateResourceId(value: unknown): asserts value is string {
  if (typeof value !== "string" || !UUID_PATTERN.test(value)) fail("resource id must be a UUID");
}

function validateTitle(value: unknown): asserts value is string {
  const title = requireString(value, "title must be a string");
  if (title.trim().length === 0 || countScalars(title) > 256 || /[\u0000-\u001f\u007f-\u009f]/u.test(title)) {
    fail("title must have 1..256 characters and no controls");
  }
}

function validateMarkdown(value: unknown): asserts value is string {
  const markdown = requireString(value, "markdown must be a string");
  if (utf8Bytes(markdown) > MAX_DOCUMENT_BYTES) fail("document exceeds 1 MiB");
}

function validateSpace(value: unknown): asserts value is string {
  validateId(value);
}

function isJsonValue(value: unknown, active: WeakSet<object> = new WeakSet()): value is null | boolean | number | string | JsonRecord | unknown[] {
  if (value === null || typeof value === "boolean") return true;
  if (typeof value === "string") return wellFormedUnicode(value);
  if (typeof value === "number") return Number.isFinite(value);
  if (Array.isArray(value)) {
    if (active.has(value)) return false;
    active.add(value);
    const valid = value.every((entry) => isJsonValue(entry, active));
    active.delete(value);
    return valid;
  }
  if (!isRecord(value)) return false;
  if (active.has(value)) return false;
  active.add(value);
  const valid = ownKeys(value).every((key) => {
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    return wellFormedUnicode(key) && !!descriptor && "value" in descriptor && descriptor.enumerable && isJsonValue(descriptor.value, active);
  });
  active.delete(value);
  return valid;
}

function addString(budget: StringBudget, value: unknown): asserts value is string {
  const string = requireString(value);
  budget.bytes += utf8Bytes(string);
  if (budget.bytes > MAX_GENUI_STRING_BYTES) fail("GenUI string budget exceeds 32 KiB");
}

function addJsonValueStrings(value: unknown, budget: StringBudget): void {
  if (typeof value === "string") {
    addString(budget, value);
    return;
  }
  if (Array.isArray(value)) {
    for (const item of value) addJsonValueStrings(item, budget);
    return;
  }
  if (isRecord(value)) {
    for (const key of ownKeys(value)) {
      if (!wellFormedUnicode(key)) fail("string contains invalid Unicode");
      const descriptor = Object.getOwnPropertyDescriptor(value, key);
      if (!descriptor || !("value" in descriptor) || !descriptor.enumerable) fail("non-JSON object property");
      addJsonValueStrings(descriptor.value, budget);
    }
  }
}

function validateLabel(value: unknown, budget: StringBudget): asserts value is string {
  const label = requireString(value, "label must be a string");
  if (label.length === 0 || countScalars(label) > 256) fail("label must have 1..256 characters");
  addString(budget, label);
}

function validateNode(
  value: unknown,
  depth: number,
  ids: Set<string>,
  budget: StringBudget,
): asserts value is GenUiNode {
  if (depth > MAX_GENUI_DEPTH) fail("GenUI depth exceeds 8");
  if (!isRecord(value)) fail("invalid GenUI node");
  const type = requireString(value.type, "invalid GenUI node type");
  const id = value.id;
  validateId(id);
  if (ids.has(id)) fail("duplicate GenUI id");
  ids.add(id);
  if (ids.size > MAX_GENUI_NODES) fail("GenUI exceeds 128 nodes");
  addString(budget, type);
  addString(budget, id);

  switch (type) {
    case "stack": {
      exactKeys(value, ["type", "id", "children"]);
      if (!Array.isArray(value.children)) fail("stack children must be an array");
      for (const child of value.children) validateNode(child, depth + 1, ids, budget);
      return;
    }
    case "text": {
      exactKeys(value, ["type", "id", "text"]);
      const text = requireString(value.text, "text must be a string");
      if (utf8Bytes(text) > MAX_GENUI_TEXT_BYTES) fail("text exceeds 8 KiB");
      addString(budget, text);
      return;
    }
    case "button": {
      exactKeys(value, ["type", "id", "label", "action"]);
      validateLabel(value.label, budget);
      if (!isRecord(value.action)) fail("button action must be an object");
      exactKeys(value.action, ["operation", "input"]);
      const operation = requireString(value.action.operation, "action operation must be a string");
      if (operation.length === 0 || utf8Bytes(operation) > 128) fail("invalid action operation");
      addString(budget, operation);
      if (!isRecord(value.action.input) || !isJsonValue(value.action.input)) fail("action input must be a JSON object");
      addJsonValueStrings(value.action.input, budget);
      return;
    }
    case "stat": {
      exactKeys(value, ["type", "id", "label", "value"]);
      validateLabel(value.label, budget);
      addString(budget, value.value);
      return;
    }
    case "resource": {
      exactKeys(value, ["type", "id", "resource_id", "space_id", "label"]);
      validateResourceId(value.resource_id);
      validateSpace(value.space_id);
      validateLabel(value.label, budget);
      addString(budget, value.resource_id);
      addString(budget, value.space_id);
      return;
    }
    case "document-editor": {
      exactKeys(value, ["type", "id", "resource_id", "space_id"]);
      validateResourceId(value.resource_id);
      validateSpace(value.space_id);
      addString(budget, value.resource_id);
      addString(budget, value.space_id);
      return;
    }
    default:
      fail("unknown GenUI node type");
  }
}

function jsonBytes(value: unknown): number {
  let serialized: string | undefined;
  try {
    serialized = JSON.stringify(value);
  } catch {
    fail("value is not JSON serializable");
  }
  if (serialized === undefined) fail("value is not JSON serializable");
  return utf8Bytes(serialized);
}

/**
 * Validate version, unique IDs, node shapes and byte/depth budgets without rendering. Traversal and temporary JSON storage cost O(view bytes).
 * @returns The validated value without an additional clone.
 * @throws {@link ContractValidationError} if decoding, shape, value, correlation or budget checks fail.
 */
export function validateGenUiView(value: unknown): GenUiView {
  if (!isRecord(value)) fail("invalid GenUI view");
  exactKeys(value, ["genui_version", "root"]);
  if (value.genui_version !== GENUI_VERSION) fail("unsupported GenUI version");
  const budget: StringBudget = { bytes: 0 };
  const ids = new Set<string>();
  validateNode(value.root, 1, ids, budget);
  if (jsonBytes(value) > MAX_GENUI_SERIALIZED_BYTES) fail("GenUI exceeds 64 KiB");
  return value as unknown as GenUiView;
}

function validateOperationInput(operation: string, input: JsonRecord): void {
  const keys = (required: string[], optional: string[] = []) => exactKeys(input, required, optional);
  const spaceAndResource = () => {
    keys(["space_id", "id"]);
    validateSpace(input.space_id);
    validateResourceId(input.id);
  };

  switch (operation) {
    case "notes.list":
    case "plugins.list":
      keys(["space_id"]);
      validateSpace(input.space_id);
      return;
    case "notes.get":
    case "notes-insights.summarize":
      spaceAndResource();
      return;
    case "notes.create":
      keys(["space_id", "title", "markdown"]);
      validateSpace(input.space_id);
      validateTitle(input.title);
      validateMarkdown(input.markdown);
      return;
    case "notes.update":
      keys(["space_id", "id", "base_revision", "markdown"], ["title"]);
      validateSpace(input.space_id);
      validateResourceId(input.id);
      if (typeof input.base_revision !== "string" || input.base_revision.length === 0 || utf8Bytes(input.base_revision) > 256) fail("invalid base_revision");
      if (Object.prototype.hasOwnProperty.call(input, "title")) validateTitle(input.title);
      validateMarkdown(input.markdown);
      return;
    case "notes.history":
      keys(["space_id", "id"], ["limit", "cursor"]);
      validateSpace(input.space_id);
      validateResourceId(input.id);
      if (Object.prototype.hasOwnProperty.call(input, "limit") && (!Number.isInteger(input.limit) || (input.limit as number) < 1 || (input.limit as number) > 100)) fail("history limit must be 1..100");
      if (Object.prototype.hasOwnProperty.call(input, "cursor") && (typeof input.cursor !== "string" || input.cursor.length === 0 || utf8Bytes(input.cursor) > 256)) fail("invalid history cursor");
      return;
    case "plugins.enable":
    case "plugins.disable":
    case "plugins.preview":
      keys(["space_id", "plugin_id"]);
      validateSpace(input.space_id);
      validateId(input.plugin_id);
      return;
    default:
      fail("unknown operation");
  }
}

/**
 * Validate an operation envelope, its known input schema and the 2 MiB serialized budget; does not authorize the caller. Costs O(request bytes). Unknown operations are rejected.
 * @returns The validated value without an additional clone.
 * @throws {@link ContractValidationError} if decoding, shape, value, correlation or budget checks fail.
 */
export function validateOperationRequest(value: unknown): OperationRequest {
  if (!isRecord(value)) fail("malformed operation envelope");
  exactKeys(value, ["request_id", "operation", "input"]);
  const requestId = requireString(value.request_id, "invalid request_id");
  if (countScalars(requestId) < 1 || countScalars(requestId) > 128) fail("request_id must have 1..128 characters");
  const operation = requireString(value.operation, "invalid operation identifier");
  if (operation.length === 0 || utf8Bytes(operation) > 128) fail("invalid operation identifier");
  if (!isRecord(value.input) || !isJsonValue(value.input)) fail("input must be a JSON object");
  validateOperationInput(operation, value.input);
  if (jsonBytes(value) > MAX_BODY_BYTES) fail("request exceeds 2 MiB");
  return value as unknown as OperationRequest;
}

function decodeJson(input: string | Uint8Array, maxBytes: number, description: string): unknown {
  const bytes = typeof input === "string" ? new TextEncoder().encode(input) : input;
  if (bytes.byteLength > maxBytes) fail(`${description} exceeds ${maxBytes / 1024} KiB`);
  let text: string;
  try {
    text = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } catch {
    fail(`${description} is not valid UTF-8`);
  }
  try {
    return JSON.parse(text);
  } catch {
    fail(`${description} is malformed JSON`);
  }
}

/**
 * Decode at most 2 MiB of strict UTF-8 JSON, then validate the operation envelope and input. Allocates O(input bytes).
 * @returns The validated value without an additional clone.
 * @throws {@link ContractValidationError} if decoding, shape, value, correlation or budget checks fail.
 */
export function parseOperationRequest(input: string | Uint8Array): OperationRequest {
  return validateOperationRequest(decodeJson(input, MAX_BODY_BYTES, "request"));
}

/**
 * @deprecated Use validateManifestV1 for new plugin menus; this frozen preview remains for existing callers.
 * Validate plugin declarations, unique operation/view IDs and all read-only contributions. Does not load code or grant capabilities. Cost includes contributed views and action-input validation.
 * @returns The validated value without an additional clone.
 * @throws {@link ContractValidationError} if decoding, shape, value, correlation or budget checks fail.
 */
export function validatePluginManifest(value: unknown): PluginManifest {
  if (!isRecord(value)) fail("invalid plugin manifest");
  exactKeys(value, ["id", "version", "api_version", "name", "description", "dependencies", "capabilities", "operations", "views"]);
  validateId(value.id);
  if (value.api_version !== PROTOCOL_VERSION) fail("unsupported plugin API");
  const version = requireString(value.version, "plugin version must be a string");
  const name = requireString(value.name, "plugin name must be a string");
  if (version.length === 0 || name.length === 0) fail("plugin version/name required");
  requireString(value.description, "plugin description must be a string");
  if (!Array.isArray(value.dependencies) || !Array.isArray(value.capabilities) || !Array.isArray(value.operations) || !Array.isArray(value.views)) {
    fail("plugin manifest arrays are invalid");
  }
  for (const dependency of value.dependencies) {
    if (!isRecord(dependency)) fail("invalid plugin dependency");
    exactKeys(dependency, ["id", "version"]);
    validateId(dependency.id);
    const dependencyVersion = requireString(dependency.version, "dependency version must be a string");
    if (dependencyVersion.length === 0) fail("dependency version required");
  }
  for (const capability of value.capabilities) requireString(capability, "capability must be a string");
  const operations = new Set<string>();
  for (const operation of value.operations) {
    if (!isRecord(operation)) fail("invalid plugin operation");
    exactKeys(operation, ["id", "description", "mutates"]);
    validateId(operation.id);
    if (operations.has(operation.id)) fail("duplicate plugin operation");
    operations.add(operation.id);
    requireString(operation.description, "operation description must be a string");
    if (operation.mutates !== false) fail("extension operations must be read-only");
  }
  const viewIds = new Set<string>();
  for (const contribution of value.views) {
    if (!isRecord(contribution)) fail("invalid plugin view");
    exactKeys(contribution, ["id", "extension_point", "view"]);
    validateId(contribution.id);
    if (viewIds.has(contribution.id)) fail("duplicate plugin view");
    viewIds.add(contribution.id);
    if (contribution.extension_point !== "notes.detail") fail("unknown extension point");
    validateExtensionView(contribution.view, value as unknown as PluginManifest);
  }
  return value as unknown as PluginManifest;
}

/**
 * Validate a portable tree and require each action to be declared and read-only. Does not grant dispatch authority. Costs O(view bytes + declared operations).
 * @returns The validated value without an additional clone.
 * @throws {@link ContractValidationError} if decoding, shape, value, correlation or budget checks fail.
 */
export function validateExtensionView(value: unknown, manifest: PluginManifest): GenUiView {
  const view = validateGenUiView(value);
  const declared = new Set(manifest.operations.map((operation) => operation.id));
  const visit = (node: GenUiNode): void => {
    if (node.type === "stack") {
      for (const child of node.children) visit(child);
    } else if (node.type === "button") {
      if (!declared.has(node.action.operation)) fail("extension action is not declared");
      if (MUTATING_HOST_OPERATIONS.has(node.action.operation)) fail("extension actions must be read-only");
      validateOperationInput(node.action.operation, node.action.input);
    }
  };
  visit(view.root);
  return view;
}

function validateWorkerResource(value: unknown): void {
  if (!isRecord(value)) fail("invalid worker resource");
  exactKeys(value, ["id", "space_id", "revision", "title", "markdown"]);
  validateResourceId(value.id);
  validateSpace(value.space_id);
  validateTitle(value.title);
  validateMarkdown(value.markdown);
  const revision = requireString(value.revision, "missing resource revision");
  if (revision.length === 0) fail("missing resource revision");
}

/**
 * Validate a notes-insights.summarize invocation and its authorized snapshot. Does not check access or issue storage capabilities. Costs O(snapshot bytes).
 * @returns The validated value without an additional clone.
 * @throws {@link ContractValidationError} if decoding, shape, value, correlation or budget checks fail.
 */
export function validateWorkerRequest(value: unknown): WorkerRequest {
  if (!isRecord(value)) fail("invalid worker request");
  exactKeys(value, ["protocol_version", "invocation_id", "operation", "resource"]);
  if (value.protocol_version !== PROTOCOL_VERSION) fail("unsupported worker protocol");
  validateId(value.invocation_id);
  if (value.operation !== "notes-insights.summarize") fail("unknown worker operation");
  validateWorkerResource(value.resource);
  return value as unknown as WorkerRequest;
}

/**
 * Decode at most 2 MiB of strict UTF-8 JSON and validate a worker invocation. Allocates O(input bytes).
 * @returns The validated value without an additional clone.
 * @throws {@link ContractValidationError} if decoding, shape, value, correlation or budget checks fail.
 */
export function parseWorkerRequest(input: string | Uint8Array): WorkerRequest {
  return validateWorkerRequest(decodeJson(input, MAX_WORKER_INPUT_BYTES, "worker input"));
}

/**
 * Validate response correlation, version, safe failure shape or successful statistics and portable view. Costs O(response bytes). The expected invocationId must come from the host.
 * @returns The validated value without an additional clone.
 * @throws {@link ContractValidationError} if decoding, shape, value, correlation or budget checks fail.
 */
export function validateWorkerResponse(value: unknown, invocationId: string): WorkerResponse {
  if (!isRecord(value)) fail("invalid worker response");
  if (value.protocol_version !== PROTOCOL_VERSION) fail("worker protocol/invocation mismatch");
  if (value.invocation_id !== invocationId) fail("worker protocol/invocation mismatch");
  if (value.ok === true) {
    exactKeys(value, ["protocol_version", "invocation_id", "ok", "data"]);
    if (!isRecord(value.data)) fail("invalid worker result");
    exactKeys(value.data, ["stats", "view"]);
    if (!isRecord(value.data.stats)) fail("invalid statistics");
    exactKeys(value.data.stats, ["words", "characters", "lines"]);
    for (const key of ["words", "characters", "lines"] as const) {
      const number = value.data.stats[key];
      if (typeof number !== "number" || !Number.isInteger(number) || number < 0 || number > 0xffff_ffff) fail("statistics must be non-negative integers");
    }
    validateGenUiView(value.data.view);
  } else if (value.ok === false) {
    exactKeys(value, ["protocol_version", "invocation_id", "ok", "error"]);
    if (!isRecord(value.error)) fail("invalid worker error");
    exactKeys(value.error, ["code", "message"]);
    if (value.error.code !== "invalid_input" && value.error.code !== "internal") fail("invalid worker error code");
    requireString(value.error.message, "worker error message must be a string");
  } else {
    fail("invalid worker response");
  }
  return value as unknown as WorkerResponse;
}

/**
 * Decode at most 256 KiB of strict UTF-8 JSON and validate response correlation with the host-issued invocationId. Allocates O(input bytes).
 * @returns The validated value without an additional clone.
 * @throws {@link ContractValidationError} if decoding, shape, value, correlation or budget checks fail.
 */
export function parseWorkerResponse(input: string | Uint8Array, invocationId: string): WorkerResponse {
  const response = validateWorkerResponse(decodeJson(input, MAX_WORKER_OUTPUT_BYTES, "worker output"), invocationId);
  return response;
}


/** Manifest v1: validateManifestV1; validation requests no authority and performs no I/O. */
export { validateManifestV1 } from "./manifest-v1.js";

/** Manifest v1: HostCallKind; validation requests no authority and performs no I/O. */
export { HostCallKind } from "./manifest-v1.js";

/** Worker v2: parseWorkerFrameV2; bounded transport and host-authorized effects. */
export { parseWorkerFrameV2 } from "./worker-v2.js";

/** Worker v2: encodeWorkerFrameV2; bounded transport and host-authorized effects. */
export { encodeWorkerFrameV2 } from "./worker-v2.js";

/** Worker v2: MAX_V2_FRAME_BYTES; bounded transport and host-authorized effects. */
export { MAX_V2_FRAME_BYTES } from "./worker-v2.js";

/** Worker v2: MAX_V2_PENDING; bounded transport and host-authorized effects. */
export { MAX_V2_PENDING } from "./worker-v2.js";

/** Worker v2: InvocationAuthority; bounded transport and host-authorized effects. */
export { InvocationAuthority } from "./worker-v2.js";

/** Worker v2: WorkerMessageV2; bounded transport and host-authorized effects. */
export { WorkerMessageV2 } from "./worker-v2.js";

/** Worker v2: WorkerFrameV2; bounded transport and host-authorized effects. */
export { WorkerFrameV2 } from "./worker-v2.js";

/** Worker v2: WorkerContextV2; bounded transport and host-authorized effects. */
export { WorkerContextV2 } from "./worker-v2.js";

/** Worker v2: WorkerHandlerV2; bounded transport and host-authorized effects. */
export { WorkerHandlerV2 } from "./worker-v2.js";

/** Worker v2: WorkerRuntimeV2; bounded transport and host-authorized effects. */
export { WorkerRuntimeV2 } from "./worker-v2.js";

/** Provider-neutral stdio adapter protocol; Rust is the authoritative type source. */
export type { ProviderContext, ProviderCommand, ProviderRequest, ProviderEvent, ProviderEventKind } from '@calternal/contracts';
/** Stable provider failure and billing categories for durable retry decisions. */
export type { ProviderFailureClass, ProviderBilling } from '@calternal/contracts';

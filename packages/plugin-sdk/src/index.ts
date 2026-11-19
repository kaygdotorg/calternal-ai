import type {
  GenUiView,
  OperationRequest,
} from "@calternal/contracts";

/** Frozen preview operation/snapshot-worker version; worker-v2 envelopes carry version 2. */
export const PROTOCOL_VERSION = 1 as const;
/** Portable declarative view version supported by the SDK. */
export const GENUI_VERSION = 2 as const;
/** Maximum UTF-8 Markdown size, in bytes (1 MiB). */
export const MAX_DOCUMENT_BYTES = 1024 * 1024;
/** Maximum serialized operation request size, in bytes (2 MiB). */
export const MAX_BODY_BYTES = 2 * 1024 * 1024;
/** Host diagnostic capture budget, in bytes (16 KiB); never include private request content. */
export const MAX_WORKER_STDERR_BYTES = 16 * 1024;
/** Host worker execution timeout, in milliseconds (3 seconds). */
export const WORKER_TIMEOUT_MS = 3000;
/** Maximum generated-UI source size, in UTF-8 bytes (64 KiB). */
export const MAX_GENUI_SERIALIZED_BYTES = 64 * 1024;

const ID_PATTERN = /^[A-Za-z0-9_.:-]{1,80}$/;
const UUID_PATTERN = /^(?:[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}|[0-9a-f]{32}|\{[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\}|urn:uuid:[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})$/i;

/** Contract violation raised by SDK parsers and validators; contains a safe diagnostic. */
export class ContractValidationError extends Error {
  /** Create a validation failure; callers must supply a diagnostic without credentials or private input. */
  constructor(message: string) {
    super(message);
    this.name = "ContractValidationError";
  }
}

type JsonRecord = Record<string, unknown>;

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
 * Validate a generated-UI view's shape, version and 64 KiB source budget without compiling it; the host compiles the source (`@calternal/genui`) before delivery. Costs O(source bytes).
 * @returns The validated value without an additional clone.
 * @throws {@link ContractValidationError} if the shape, version or budget check fails.
 */
export function validateGenUiView(value: unknown): GenUiView {
  if (!isRecord(value)) fail("invalid GenUI view");
  exactKeys(value, ["genui_version", "source"]);
  if (value.genui_version !== GENUI_VERSION) fail("unsupported GenUI version");
  const source = requireString(value.source, "GenUI source must be a string");
  if (utf8Bytes(source) > MAX_GENUI_SERIALIZED_BYTES) fail("GenUI exceeds 64 KiB");
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

/** List one indexed page of a kind owned by the invoking worker. */
export { listItems } from "./worker-v2.js";
/** Prepare a sealed frame from a committed item revision and a declared content source. */
export { prepareContent } from "./worker-v2.js";
/** Read the effective value of a setting declared in this package. */
export { readSetting } from "./worker-v2.js";

/** Worker v2: WorkerHandlerV2; bounded transport and host-authorized effects. */
export { WorkerHandlerV2 } from "./worker-v2.js";

/** Worker v2: WorkerRuntimeV2; bounded transport and host-authorized effects. */
export { WorkerRuntimeV2 } from "./worker-v2.js";

/** Provider-neutral stdio adapter protocol; Rust is the authoritative type source. */
export type { ProviderContext, ProviderCommand, ProviderRequest, ProviderEvent, ProviderEventKind } from '@calternal/contracts';
/** Stable provider failure and billing categories for durable retry decisions. */
export type { ProviderFailureClass, ProviderBilling } from '@calternal/contracts';

/** Drive the worker-v2 runtime over bounded streams, for subprocesses and fixtures. */
export { serveWorker } from "./stdio-worker.js";

/** Bind operation handlers to identifiers and input types from a literal manifest. */
export { typedHandlers } from "./typed-handlers.js";

/** Durably park a replayable job on a typed signal or deadline. */
export { parkJob, startJob } from "./job-waits.js";
/** Signal and checkpoint contracts used by durable job handlers. */
export type { JobPark, JobSignal, JobWaitSource, ParkedJob } from "./job-waits.js";

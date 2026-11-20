/**
 * Operations declared once: the manifest entry, the handler's input and
 * output types and the broker methods its context may call all come from one
 * `defineOperation` call. The returned value is the explicit manifest entry.
 * @module
 */
import { createHash } from "node:crypto";
import type {
  AlarmDelivery, AuthorContracts, AuthorInvocationContext, ExecutionJobContext, FormField, ManifestV2,
} from "../generated/author-types.js";
import { CATALOGUE_ITEM_FIELDS } from "./generated/broker-methods.js";
import { ContractValidationError } from "./index.js";
import { LIMITS } from "./limits.js";
import type { Schema, SchemaType } from "./schema.js";

/** Every broker method with a pinned catalogue exchange, keyed by name. */
export type Broker = NonNullable<AuthorContracts["broker"]>;
/** One broker method name with a pinned exchange. */
export type BrokerMethod = keyof Broker & string;
/** Catalogue input of one broker method. */
export type BrokerInput<M extends BrokerMethod> = Broker[M]["input"];
/** Catalogue reply of one broker method. */
export type BrokerOutput<M extends BrokerMethod> = Broker[M]["output"];
/** Every method name the manifest lets an operation declare. */
export type HostMethod = ManifestV2["operations"][number]["methods"][number];
/** One operation entry exactly as the manifest stores it. */
export type ManifestOperation = ManifestV2["operations"][number];

/**
 * What a handler receives. `call` accepts only the methods its operation
 * declares, with catalogue-typed input and reply. `key` derives a
 * deterministic subeffect request ID under the invocation's stable request
 * ID, so a replay reuses every receipt.
 */
export interface OperationContext<M extends string = BrokerMethod> {
  /** Stable request ID, actor and authored time stamped by the host. */
  readonly author: AuthorInvocationContext;
  /** Checked job metadata when this invocation runs an admitted job. */
  readonly job?: ExecutionJobContext;
  /** Delivery evidence when an alarm admitted this job. */
  readonly alarm?: AlarmDelivery;
  /** Local cancellation; effects still require host reconciliation. */
  readonly signal?: AbortSignal;
  /**
   * The broker methods this operation declares. Also makes a context that
   * lacks a method unassignable to a helper that needs it.
   */
  readonly methods: Readonly<Record<M, true>>;
  /** Call a declared broker method. Cost: one bounded broker round trip. */
  call<K extends M & BrokerMethod>(method: K, input: BrokerInput<K>): Promise<BrokerOutput<K>>;
  /** Deterministic request ID for one named subeffect of this invocation. Cost: one SHA-256. */
  key(purpose: string): string;
}

/** A handler for one declared operation. */
export type OperationHandler<I = unknown, O = unknown, M extends string = BrokerMethod> =
  (input: I, context: OperationContext<M>) => Promise<O> | O;

const handlerKey = Symbol.for("calternal.plugin-sdk.handler");

/** A manifest operation entry carrying its handler under a symbol key, which JSON serialization omits. */
export type DefinedOperation<I = unknown, O = unknown, M extends string = BrokerMethod> = ManifestOperation & {
  /** The handler, when the operation has worker code. */
  readonly [handlerKey]?: OperationHandler<I, O, M>;
};

type OperationDeclaration<I, O, M extends readonly HostMethod[]> =
  Omit<ManifestOperation, "input_schema" | "output_schema" | "methods" | "target"> & {
    /** Closed JSON Schema of the handler input; its static type becomes the handler's input type. */
    input_schema: I;
    /** JSON Schema of the handler result; its static type becomes the handler's return type. */
    output_schema: O;
    /** Broker methods the handler may call; the context's `call` accepts exactly these. */
    methods: M;
    /** The target kind, with the field inferred from the single ItemRef input, or an explicit target. */
    target?: string | ManifestOperation["target"];
    /** Worker implementation. Omit it for an operation without worker code. */
    handler?: OperationHandler<SchemaType<I>, SchemaType<O>, M[number]>;
  };

const catalogueRef = /(?:^|\/)host-catalogue-v2\.json#\/\$defs\/([A-Za-z0-9_]+)$/;
const refName = (value: unknown) => {
  const target = value && typeof value === "object" ? (value as { $ref?: unknown }).$ref : undefined;
  return typeof target === "string" ? catalogueRef.exec(target)?.[1] : undefined;
};

function itemRefFields(schema: unknown): readonly string[] {
  const named = refName(schema);
  if (named) return CATALOGUE_ITEM_FIELDS[named] ?? [];
  const properties = schema && typeof schema === "object" && "properties" in schema ? schema.properties : undefined;
  if (!properties || typeof properties !== "object") return [];
  return Object.entries(properties).filter(([, value]) => refName(value) === "ItemRef").map(([name]) => name);
}

/**
 * Declare an operation and, optionally, its handler. A string `target` names
 * the target kind; the field is the input's single ItemRef property. Throws at
 * authoring time when that field is missing or ambiguous. The return value is
 * the full manifest entry, with the inferred target written out.
 */
export function defineOperation<const I extends Schema, const O extends Schema, const M extends readonly HostMethod[]>(
  declaration: OperationDeclaration<I, O, M>,
): DefinedOperation<SchemaType<I>, SchemaType<O>, M[number]> {
  const { handler, target, methods, ...entry } = declaration;
  let resolved: ManifestOperation["target"];
  if (typeof target === "string") {
    const fields = itemRefFields(entry.input_schema);
    if (fields.length !== 1) {
      throw new ContractValidationError(`Operation ${entry.id}: target ${target} needs exactly one ItemRef input property, found ${fields.length}. Declare target as { kind, field }.`);
    }
    resolved = { kind: target, field: fields[0]! };
  } else resolved = target;
  const operation = { ...entry, methods: [...methods], ...(resolved && { target: resolved }) } as ManifestOperation;
  if (handler) Object.defineProperty(operation, handlerKey, { value: handler, enumerable: false });
  return operation as DefinedOperation<SchemaType<I>, SchemaType<O>, M[number]>;
}

/** The handler map a worker exports, built from defined operations. Operations without a handler are skipped. */
export function handlersOf(operations: readonly ManifestOperation[]): Record<string, OperationHandler> {
  const handlers: Record<string, OperationHandler> = {};
  for (const operation of operations) {
    const handler = (operation as DefinedOperation)[handlerKey];
    if (handler) handlers[operation.id] = handler as OperationHandler;
  }
  return handlers;
}

/**
 * Deterministic request ID for a subeffect: the first 32 hex characters of
 * SHA-256 over the JSON pair of the parent request ID and the purpose.
 */
export function subeffectKey(request: string, purpose: string): string {
  const key = createHash("sha256").update(JSON.stringify([request, purpose])).digest("hex").slice(0, 32);
  return key.slice(0, LIMITS.REF_CHARACTERS);
}

/**
 * Adapt an authenticated, validated host invoke to the handler context. The
 * caller supplies the operation's declared methods and a broker `call`
 * already bound to that invocation. Freezes the stable author data.
 */
export function authorContext<M extends string = BrokerMethod>(
  message: { author_context: AuthorInvocationContext; job_context?: ExecutionJobContext; alarm?: AlarmDelivery },
  methods: readonly M[],
  call: OperationContext<M>["call"],
  signal?: AbortSignal,
): OperationContext<M> {
  const { author_context: author, job_context: job, alarm } = message;
  return Object.freeze({
    methods: Object.freeze(Object.fromEntries(methods.map(method => [method, true]))) as Readonly<Record<M, true>>,
    author: Object.freeze({ ...author }),
    ...(job && { job: Object.freeze({ ...job }) }),
    ...(alarm && { alarm: Object.freeze({ ...alarm }) }),
    ...(signal && { signal }),
    call,
    key: (purpose: string) => subeffectKey(author.request_id, purpose),
  });
}

type FieldOverrides = Partial<Omit<FormField, "id">> & Pick<FormField, "label">;

/**
 * A description form field for one operation input property. The control,
 * `required`, `maxLength` and choice options come from the input schema;
 * `label` and any presentation keys come from `overrides`. Throws when the
 * property is not declared.
 */
export function formField(operation: ManifestOperation, property: string, overrides: FieldOverrides): FormField {
  const schema = operation.input_schema as { properties?: Record<string, Record<string, unknown>>; required?: string[] };
  const field = schema.properties?.[property];
  if (!field) throw new ContractValidationError(`Operation ${operation.id} has no input property ${property}.`);
  const enumValues = Array.isArray(field.enum) ? field.enum.filter((value): value is string => typeof value === "string") : undefined;
  const input: FormField["input"] = enumValues ? "choice"
    : field.type === "boolean" ? "toggle"
    : field.type === "integer" || field.type === "number" ? "number"
    : field.format === "uri" ? "url"
    : field.format === "email" ? "email"
    : field.format === "date-time" ? "date"
    : "text";
  const maxLength = typeof field.maxLength === "number" ? Math.min(field.maxLength, 4000) : undefined;
  return {
    id: property, input,
    ...(schema.required?.includes(property) && { required: true }),
    ...(maxLength !== undefined && input !== "choice" && { maxLength }),
    ...(enumValues && { options: enumValues.map(value => ({ value, label: value })) }),
    ...overrides,
  } as FormField;
}

/** The operation input type of a defined operation, for helpers and tests. */
export type InputOf<D> = D extends DefinedOperation<infer I, unknown, string> ? I : never;
/** The operation result type of a defined operation. */
export type OutputOf<D> = D extends DefinedOperation<unknown, infer O, string> ? O : never;

// Generated from host serde declarations by generate-operation-types.mjs.
import type { Fields, ItemId, RecordBody } from "./operation-api.js";

/** A kind query or a stored collection query accepted by the standalone host. */
export type Query =
  | { "from": string; "where"?: string; "sort"?: readonly string[]; "cursor"?: string; "limit"?: number }
  | { "collection": string; "cursor"?: string; "limit"?: number };

/** Changes supported by the host's atomic write route. */
export type Change =
  | { "add": { "kind": string; "fields"?: Fields; "body"?: string } }
  | { "change": { "item": ItemId; "fields"?: Fields; "body"?: string; "base"?: number } }
  | { "remove": { "item": ItemId } }
  | { "append record": { "item": ItemId; "record": RecordBody } }
  | { "change record": { "item": ItemId; "anchor": string; "record": RecordBody } }
  | { "remove record": { "item": ItemId; "anchor": string } };

/** Change discriminants accepted by the host. */
export const operationChanges = ["add","change","remove","append record","change record","remove record"] as const;

/** Callback wire names accepted by the host. */
export const operationMethods = {
  query: "s.query",
  read: "s.read",
  write: "s.write",
  state: "s.state",
  call: "s.call",
  run: "s.run",
  runSend: "s.run.send",
  runStop: "s.run.stop",
} as const;

/** One callback wire name. */
export type OperationMethod = typeof operationMethods[keyof typeof operationMethods];

/**
 * The typed model of the description contract. Every shape is generated from `contract/description-v2.schema.json` and `contract/host-catalogue-v2.json`
 * (`scripts/generate-types.mjs`); nothing here is written a second time. Only names that read better in this package are aliased.
 */
export type * from "./generated/model";
import type { WindowRow as GeneratedWindowRow, WindowResult, WindowPlanError, ActionContexts } from "./generated/model";

/** A host reply to a window request: rows or a typed refusal (the catalogue's `WindowReply`). */
export type WindowReply = WindowResult | WindowPlanError;
/** A row's fields as the host sent them (the catalogue bounds them at 32; their values are the kind's own scalars). */
export type RowFields = Record<string, unknown>;
/** A row of a window: the host-issued id, its fields and the parent refs of its child binds. (The schema bounds `fields` without naming them, so they are typed here as the kind's own values.) */
export type WindowRow = Omit<GeneratedWindowRow, "fields"> & { fields: RowFields };
export type { ActionContexts };

/** Whether a reply is the typed plan refusal (HTTP 422, code `unsupported_query_plan`). */
export function isPlanError(r: WindowReply): r is WindowPlanError {
  return (r as WindowPlanError).code === "unsupported_query_plan";
}

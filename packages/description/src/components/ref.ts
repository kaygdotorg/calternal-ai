import type { ValueSource } from "../model";
import type { AppContext } from "../invoke/context";

/**
 * The typed reference a component's config names (a thread, a request): from the route parameter, from a bind's drawn selection, or from a field of a bind's first row (an item or the open request). The value is a
 * host-issued ref; it is read, never made up. Undefined while nothing is chosen.
 */
export function resolveRef(source: ValueSource | undefined, app: AppContext | undefined): string | undefined {
  if (!source || !app) return undefined;
  if ("route" in source && typeof source.route === "string") return app.route[source.route];
  if ("selection" in source) return app.selectionFor(source.selection).drawn ?? undefined;
  if ("bind" in source) { const r = app.storeFor(source.bind)?.row(0); return r ? (source.field === "id" ? r.id : (r.fields[source.field] as string | undefined) ?? r.id) : undefined; }
  return undefined;
}

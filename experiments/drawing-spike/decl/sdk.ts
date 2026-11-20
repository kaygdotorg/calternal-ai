// A data-only stand-in for `@calternal/sdk` (authoring.md section 1): every builder returns plain data, so a plugin's
// manifest serialises to the JSON a host would check at install and hand to clients. Handlers are replaced by `"code"`:
// the client never receives code, only the fact that the operation runs in the plugin's worker. Throwaway spike code.

/** One declared field: a type plus the options the builders add. Serialises to `{ type, ... }`. */
export class Field {
  type: string; extra: Record<string, unknown>;
  constructor(type: string, extra: Record<string, unknown> = {}) { this.type = type; this.extra = extra; }
  /** Marks the field as indexed (queryable). */
  index(): this { this.extra.indexed = true; return this; }
  /** Marks an operation input as optional. */
  optional(): this { this.extra.optional = true; return this; }
  /** SPIKE ADDITION (change D3): the label a derived form or an inspector shows; without it the field's name is shown. */
  label(text: string): this { this.extra.label = text; return this; }
  /** SPIKE ADDITION (change D4): names for a field's values, used by a board's columns and a grouped list's headings. */
  values(names: Record<string, string>): this { this.extra.values = names; return this; }
  toJSON() { return { type: this.type, ...this.extra }; }
}
export const text = () => new Field("text");
export const url = () => new Field("url");
export const bool = (initial?: boolean) => new Field("bool", initial === undefined ? {} : { default: initial });
export const number = () => new Field("number");
export const date = () => new Field("date");
export const datetime = () => new Field("datetime");
export const link = () => new Field("link");
export const person = () => new Field("person");
export const file = () => new Field("file");
export const list = (of: Field) => new Field("list", { of });

export const plugin = <T extends object>(p: T) => p;
export const kind = <T extends object>(k: T) => k;
export const collection = (title: string, q: object) => ({ collection: title, ...q });
export const facet = (title: string, q: object) => ({ facet: title, ...q });
export const slot = (s: object) => ({ slot: true, ...s });
/** An operation whose behaviour is code in the plugin's worker; the handler itself never leaves the package. */
export const operation = (o: { handler?: unknown } & object) => ({ ...o, handler: "code" });
/** Declarative "create a file of this kind from these inputs" (no handler). The third argument is SPIKE ADDITION D2. */
export const create = (kind: string, o: object, show?: object) => ({ create: kind, ...o, ...show });
/** Declarative "set these fields on the item" (no handler). The third argument is SPIKE ADDITION D1. */
export const set = (kind: string, fields: object, show?: object) => ({ set: kind, fields, ...show });

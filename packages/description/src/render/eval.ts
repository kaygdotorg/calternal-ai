/**
 * What a row template says, evaluated against one row's fields: title, leading icon, marks. Pure functions: the renderer draws exactly what they return and adds or drops nothing. Values that come from
 * the viewer, the route or the selection are the host's to fill and never appear in a row's fields, so a predicate naming them cannot hold here and is false.
 */
import type { Icon, Marks, Predicate, RowTemplate } from "../model";
import type { RowFields } from "../model";

/** The row's title: the template's field, else its fallback, else empty (the row still draws, so no row is dropped). */
export function titleOf(t: RowTemplate, fields: RowFields): string {
  const v = fields[t.title.field];
  if (v === undefined || v === null || v === "") return t.title.fallback ?? "";
  return String(v);
}

const num = (v: unknown): number | null => (typeof v === "number" ? v : null);

/** Evaluates a `when` predicate on the row's own fields. Only literal operands can be compared on the device. */
export function holds(p: Predicate | undefined, fields: RowFields): boolean {
  if (!p) return true;
  const { field, op } = p, value = p.value as { literal?: unknown } | undefined;
  const have = fields[field];
  if (op === "exists") return have !== undefined && have !== null;
  if (!value || !("literal" in value)) return false;
  const want = value.literal;
  switch (op) {
    case "eq": return have === want;
    case "ne": return have !== want;
    case "in": return Array.isArray(want) && want.includes(have);
    case "lt": return num(have) !== null && num(want) !== null && num(have)! < num(want)!;
    case "lte": return num(have) !== null && num(want) !== null && num(have)! <= num(want)!;
    case "gt": return num(have) !== null && num(want) !== null && num(have)! > num(want)!;
    case "gte": return num(have) !== null && num(want) !== null && num(have)! >= num(want)!;
    case "prefix": return typeof have === "string" && typeof want === "string" && have.startsWith(want);
    default: return false;
  }
}

/** A trailing mark ready to draw. */
export interface MarkView { kind: "dot" | "count" | "text" | "icon"; text?: string; icon?: string; tone?: "good" | "warn" | "bad" }

/** The marks of a row, at most three, those whose `when` holds. A count of zero draws nothing, as a system badge does. */
export function marksOf(marks: Marks | undefined, fields: RowFields): MarkView[] {
  const out: MarkView[] = [];
  for (const m of marks ?? []) {
    if (out.length >= 3 || !holds(m.when, fields)) continue;
    const have = m.field ? fields[m.field] : undefined;
    switch (m.kind) {
      case "dot": if (m.when || have) out.push({ kind: "dot", tone: m.tone }); break;
      case "count": { const n = num(have); if (n !== null && n > 0) out.push({ kind: "count", text: String(n), tone: m.tone }); break; }
      case "text": { const t = m.value ?? have; if (t !== undefined && t !== null && t !== "") out.push({ kind: "text", text: String(t), tone: m.tone }); break; }
      case "icon": if (m.icon) out.push({ kind: "icon", icon: typeof m.icon === "string" ? m.icon : undefined, tone: m.tone }); break;
    }
  }
  return out;
}

/** The leading icon name: a literal name, a field's value (through `map`), or the kind's own icon. Undefined means the template declares none (the column is then not drawn). */
export function iconOf(icon: Icon | undefined, fields: RowFields, kindIcon: string | undefined): string | undefined {
  if (icon === undefined) return undefined;
  if (typeof icon === "string") return icon;
  if (icon.kindIcon) return kindIcon;
  if (icon.field) { const v = String(fields[icon.field] ?? ""); return icon.map?.[v] ?? kindIcon; } // an unmapped value draws the kind's icon (the contract's rule)
  return icon.name;
}

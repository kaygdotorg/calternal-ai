/**
 * What a row template says, evaluated against one row's fields: title, leading icon, marks. Pure functions: the renderer draws exactly what they return and adds or drops nothing. Values that come from
 * the viewer, the route or the selection are the host's to fill and never appear in a row's fields, so a predicate naming them cannot hold here and is false.
 */
import type { Icon, Marks, Predicate, RowTemplate } from "../model";

type Secondary = { field: string; format: "relative_date" | "short_date" | "text" };
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
export interface MarkView { kind: "dot" | "count" | "text" | "icon"; text?: string; icon?: string; tone?: "good" | "warn" | "bad" | "quiet" }

const RFC3339 = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?(Z|[+-]\d{2}:\d{2})$/;

/** The marks of a row, at most three, those whose `when` holds. A count of zero draws nothing, as a system badge does. */
export function marksOf(marks: Marks | undefined, fields: RowFields): MarkView[] {
  const out: MarkView[] = [];
  for (const m of marks ?? []) {
    if (out.length >= 3 || !holds(m.when, fields)) continue;
    const have = m.field ? fields[m.field] : undefined;
    switch (m.kind) {
      case "dot": if (m.when || have) out.push({ kind: "dot", tone: m.tone }); break;
      case "count": { const n = num(have); if (n !== null && n > 0) out.push({ kind: "count", text: String(n), tone: m.tone }); break; }
      case "text": { const t = m.value ?? have; if (t !== undefined && t !== null && t !== "") { const when = typeof t === "string" && RFC3339.test(t) ? ms(t) : null; out.push({ kind: "text", text: when !== null ? relativeDate(when, Date.now()) : String(t), tone: m.tone }); } break; } // (a Timestamp field in a text mark reads as a relative date, as in the row's line before: a one-line row keeps its time)
      case "icon": if (m.icon) out.push({ kind: "icon", icon: typeof m.icon === "string" ? m.icon : undefined, tone: m.tone }); break;
    }
  }
  return out;
}

/** The leading icon name: a literal name, a field's value (through `map`), or the kind's own icon. Undefined means the template declares none (the column is then not drawn). */
export function iconOf(icon: Icon | undefined | { image: unknown }, fields: RowFields, kindIcon: string | undefined): string | undefined {
  if (icon === undefined) return undefined;
  if (typeof icon === "string") return icon;
  if ("image" in icon) return undefined; // an image icon is drawn by RowImage
  if (icon.kindIcon) return kindIcon;
  if (icon.field) { const v = String(fields[icon.field] ?? ""); return icon.map?.[v] ?? kindIcon; } // an unmapped value draws the kind's icon (the contract's rule)
  return icon.name;
}

/** A Timestamp field as milliseconds: an ISO string, or a number in seconds (or milliseconds when already large). */
function ms(v: unknown): number | null {
  if (typeof v === "number") return v < 1e12 ? v * 1000 : v;
  if (typeof v === "string") { const t = Date.parse(v); return Number.isNaN(t) ? null : t; }
  return null;
}

const rtf = typeof Intl !== "undefined" ? new Intl.RelativeTimeFormat(undefined, { numeric: "auto" }) : null;
/** "2 hours ago", "yesterday", "3 weeks ago" against the clock passed in (the local one when drawing); no host push is needed to keep it right. */
export function relativeDate(t: number, now: number): string {
  const d = (t - now) / 1000, a = Math.abs(d);
  const units: [number, Intl.RelativeTimeFormatUnit][] = [[60, "second"], [3600, "minute"], [86400, "hour"], [604800, "day"], [2629800, "week"], [31557600, "month"], [Infinity, "year"]];
  const div = [1, 60, 3600, 86400, 604800, 2629800, 31557600];
  for (let i = 0; i < units.length; i++) if (a < units[i]![0]) return rtf ? rtf.format(Math.round(d / div[i]!), units[i]![1]) : new Date(t).toLocaleDateString();
  return new Date(t).toLocaleDateString();
}

/** The secondary line as its parts (the same rules as `secondaryOf`), so a row can ellipsise the first and keep the last (the time) whole. */
export function secondaryParts(parts: readonly Secondary[] | undefined, fields: RowFields, now: number = Date.now()): string[] {
  const s = secondaryOf(parts, fields, now);
  if (!s) return [];
  const out: string[] = [];
  for (const p of (parts ?? []).slice(0, 3)) { const one = secondaryOf([p], fields, now); if (one) out.push(one); }
  return out;
}

/** The secondary line: the declared parts, in order, each from its projected field; an absent or malformed part (and its separator) is omitted; joined by a space, a middle dot and a space. Undefined when the template declares none. */
export function secondaryOf(parts: readonly Secondary[] | undefined, fields: RowFields, now: number = Date.now()): string | undefined {
  if (!parts?.length) return undefined;
  const out: string[] = [];
  for (const p of parts.slice(0, 3)) {
    const v = fields[p.field];
    if (v === undefined || v === null || v === "") continue;
    if (p.format === "text") { if (typeof v === "string") out.push(v.replace(/\s+/g, " ").trim()); continue; }
    const t = ms(v); if (t === null) continue;
    out.push(p.format === "relative_date" ? relativeDate(t, now) : new Date(t).toLocaleDateString(undefined, { month: "short", day: "numeric" }));
  }
  return out.length ? out.join(" \u00b7 ") : "";
}

/** The image icon of a template (`{image: {field, fallback?}}`), or null. */
export function imageIconOf(icon: unknown): { field: string; fallback?: string } | null {
  const i = icon as { image?: { field: string; fallback?: string } } | undefined;
  return i && typeof i === "object" && i.image ? i.image : null;
}

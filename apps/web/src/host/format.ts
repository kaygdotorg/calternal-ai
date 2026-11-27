/**
 * Generic formatting, predicate evaluation, and action derivation from kind declarations.
 * Contains no hardcoded knowledge of any specific plugin or kind.
 */
import type { KindDecl, OperationDecl, RowData } from "./types";

/** Convert a declaration identifier (`mark read`, `read through`) into a sentence-case label. */
export function humanize(name: string): string {
  const clean = name.replace(/[_-]+/g, " ").trim();
  if (!clean) return "";
  return clean[0]!.toUpperCase() + clean.slice(1);
}

/** Format an ISO timestamp or epoch milliseconds into `HH:MM`. */
export function formatClock(at: string | number): string {
  const d = typeof at === "number" ? new Date(at) : new Date(at);
  if (Number.isNaN(d.getTime())) return "";
  return new Intl.DateTimeFormat(undefined, { hour: "2-digit", minute: "2-digit" }).format(d);
}

/** Calendar day key (`YYYY-MM-DD`) in the viewer's local timezone for grouping records. */
export function dayKey(at: string | number): string {
  const d = typeof at === "number" ? new Date(at) : new Date(at);
  if (Number.isNaN(d.getTime())) return "";
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

/** Human-readable day heading (`Today`, `Yesterday`, or full weekday and date). */
export function dayLabel(at: string | number): string {
  const d = typeof at === "number" ? new Date(at) : new Date(at);
  if (Number.isNaN(d.getTime())) return "";
  const now = new Date();
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
  const target = new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  const diffDays = Math.round((today - target) / 86_400_000);
  if (diffDays === 0) return "Today";
  if (diffDays === 1) return "Yesterday";
  return new Intl.DateTimeFormat(undefined, { weekday: "long", day: "numeric", month: "long" }).format(d);
}

/** Read a field value from a row's `fields`, `computed`, or `state`. */
export function readRowValue(row: RowData, key: string): unknown {
  if (key.startsWith("state.")) {
    const k = key.slice("state.".length);
    return row.state?.[k] ?? row.computed?.[key];
  }
  if (key in row.fields) return row.fields[key];
  if (key in row.computed) return row.computed[key];
  return undefined;
}

/**
 * Evaluate a simple `when` predicate from a kind operation declaration
 * (e.g. `state.pinned`, `not state.pinned`, `archived`, `not archived`).
 */
export function evalWhen(when: string | undefined, row: RowData): boolean {
  if (!when) return true;
  const trimmed = when.trim();
  const neg = trimmed.startsWith("not ");
  const expr = neg ? trimmed.slice(4).trim() : trimmed;
  const val = readRowValue(row, expr);
  const truthy = Boolean(val) && val !== "no" && val !== "false";
  return neg ? !truthy : truthy;
}

/** Pick the primary display title of a row using the kind's `row:` list (first text field, fallback `title` or ID). */
export function rowTitle(row: RowData, kind?: KindDecl): string {
  const candidates = kind?.row?.length ? kind.row : ["title"];
  for (const col of candidates) {
    const decl = kind?.fields[col];
    if (decl && decl.type !== "text" && decl.type !== "long text") continue;
    const v = readRowValue(row, col);
    if (typeof v === "string" && v.trim()) return v.trim();
  }
  const fallback = row.fields.title ?? row.fields.name;
  if (typeof fallback === "string" && fallback.trim()) return fallback.trim();
  return "Untitled";
}

/** Extract a list of people names from a row if the kind declares a `people` field in `row:`. */
export function rowPeople(row: RowData, kind?: KindDecl): string[] {
  if (!kind) return [];
  for (const col of kind.row) {
    if (kind.fields[col]?.type === "people") {
      const v = readRowValue(row, col);
      if (Array.isArray(v)) {
        return v.map((p) => (typeof p === "string" ? p : String((p as { name?: string })?.name ?? ""))).filter(Boolean);
      }
    }
  }
  return [];
}

/** Extract a numeric badge count (e.g. unread) from computed numeric fields listed in `row:`. */
export function rowCount(row: RowData, kind?: KindDecl): number {
  if (!kind) return 0;
  for (const col of kind.row) {
    const decl = kind.fields[col];
    if (decl?.formula || decl?.type === "number") {
      const v = readRowValue(row, col);
      if (typeof v === "number" && v > 0) return v;
    }
  }
  return 0;
}

/** Derived UI action ready to be rendered in a menu or toolbar. */
export interface DerivedAction {
  id: string;
  label: string;
  operation: OperationDecl;
  kind: KindDecl;
  danger?: boolean;
}

/** Return the menu/item operations applicable to `row` according to its kind declaration. */
export function itemMenuActions(row: RowData, kind: KindDecl): DerivedAction[] {
  const out: DerivedAction[] = [];
  for (const op of Object.values(kind.operations)) {
    if (op.form === "add" || op.form === "change_record" || op.form === "remove_record") continue;
    if (op.form === "code" && !op.placement.includes("menu") && !op.placement.includes("primary")) continue;
    if (op.form === "set" && !op.placement.includes("menu") && !op.placement.includes("primary")) continue;
    if (!evalWhen(op.when, row)) continue;
    if (op.form === "change") {
      for (const field of op.fields) {
        const fdecl = kind.fields[field];
        if (fdecl?.type === "yes/no") {
          const cur = Boolean(row.fields[field]);
          out.push({
            id: `change:${field}:${cur ? "no" : "yes"}`,
            label: cur ? `Un${field}` : humanize(field),
            operation: { ...op, target: field, value: !cur },
            kind,
          });
        }
      }
      continue;
    }
    out.push({
      id: op.name,
      label: humanize(op.name),
      operation: op,
      kind,
      danger: op.form === "remove",
    });
  }
  return out;
}

/** Return the initials (1–2 characters) for a person's display name. */
export function initials(name: string): string {
  const parts = name.trim().split(/\s+/).filter(Boolean);
  if (!parts.length) return "?";
  if (parts.length === 1) return parts[0]!.slice(0, 2).toUpperCase();
  return (parts[0]![0]! + parts[parts.length - 1]![0]!).toUpperCase();
}

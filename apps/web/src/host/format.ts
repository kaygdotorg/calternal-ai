/**
 * Generic formatting, server-evaluated `when` filtering, computed field lookup,
 * slot filler resolution, and form/action derivation from `s.declarations`.
 * Contains zero hardcoded knowledge of any specific plugin or kind.
 */
import type { FieldDecl, KindDecl, OperationDecl, PluginDecl, RowData, SlotDecl } from "./types";

/** Convert a declaration identifier (`mark read`, `read through`) into a sentence-case label. */
export function humanize(name: string): string {
  const clean = name.replace(/[_-]+/g, " ").trim();
  if (!clean) return "";
  return clean[0]!.toUpperCase() + clean.slice(1);
}

/** Format an ISO timestamp or epoch milliseconds into a locale-aware clock time (12h or 24h per browser locale). */
export function formatClock(at: string | number): string {
  const d = typeof at === "number" ? new Date(at) : new Date(at);
  if (Number.isNaN(d.getTime())) return "";
  return new Intl.DateTimeFormat(undefined, { hour: "numeric", minute: "2-digit" }).format(d);
}

/**
 * Format an ISO timestamp or epoch milliseconds into a compact relative label for sidebar/list rows
 * (`"now"`, `"5m"`, `"11h"`, `"Yesterday"`, `"3d"`, or a short locale date like `"23 Oct"`).
 */
export function formatRelativeTime(at: string | number): string {
  const d = typeof at === "number" ? new Date(at) : new Date(at);
  if (Number.isNaN(d.getTime())) return "";
  const now = new Date();
  const diffMs = Math.max(0, now.getTime() - d.getTime());
  const diffMin = Math.floor(diffMs / 60_000);
  if (diffMin < 1) return "now";
  if (diffMin < 60) return `${diffMin}m`;
  const diffHr = Math.floor(diffMin / 60);
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
  const target = new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  const diffDays = Math.round((today - target) / 86_400_000);
  if (diffHr < 24 && diffDays === 0) return `${diffHr}h`;
  if (diffDays === 1) return "Yesterday";
  if (diffDays > 1 && diffDays < 7) return `${diffDays}d`;
  return new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short" }).format(d);
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
 * Check whether `operationName` is currently permitted on `row` according to the
 * host's server-evaluated `row.when` map.
 */
export function isOperationAllowed(operationName: string, row: RowData): boolean {
  if (row.when && operationName in row.when) {
    return Boolean(row.when[operationName]);
  }
  return true;
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

/** Extract a numeric badge from fields explicitly flagged `badge: true` in `s.declarations`. */
export function rowBadge(row: RowData, kind?: KindDecl): { value: number; name: string } | null {
  if (!kind) return null;
  for (const col of kind.row) {
    const decl = kind.fields[col];
    if (decl?.badge) {
      const v = readRowValue(row, col);
      if (typeof v === "number" && v > 0) return { value: v, name: col };
    }
  }
  return null;
}

/** Derived UI action ready to be rendered in a menu or toolbar. */
export interface DerivedAction {
  id: string;
  label: string;
  operation: OperationDecl;
  kind: KindDecl;
  danger?: boolean;
}

/** Return the menu/item operations applicable to `row` according to `kind.operations` and `row.when`. */
export function itemMenuActions(row: RowData, kind: KindDecl): DerivedAction[] {
  const out: DerivedAction[] = [];
  for (const op of Object.values(kind.operations)) {
    if (op.form === "add" || op.form === "change_record" || op.form === "remove_record" || op.form === "append_record") {
      continue;
    }
    if (op.form === "code" && !op.placement.includes("menu") && !op.placement.includes("primary")) continue;
    if (op.form === "set" && !op.placement.includes("menu") && !op.placement.includes("primary")) continue;
    if (!isOperationAllowed(op.name, row)) continue;
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

/** One form field derived strictly from `OperationDecl.inputs` (excluding the `item` link target). */
export interface DerivedFormField {
  name: string;
  label: string;
  type: string;
  required: boolean;
  value: string;
}

/**
 * Derive user-facing form fields from a coded operation's `inputs` declaration.
 * Automatically omits the `item` target link (which is always supplied as `{ item: row.id }`).
 */
export function operationFormFields(op: OperationDecl, row?: RowData): DerivedFormField[] {
  const out: DerivedFormField[] = [];
  for (const [name, decl] of Object.entries<FieldDecl>(op.inputs ?? {})) {
    if (name === "item") continue;
    const raw = row?.fields[name];
    out.push({
      name,
      label: humanize(name),
      type: decl.type || "text",
      required: Boolean(decl.required),
      value: typeof raw === "string" ? raw : "",
    });
  }
  return out;
}

/** Resolved composer slot metadata from `s.declarations` (r10 `slot.composer` bindings). */
export interface ResolvedComposer {
  slot: SlotDecl;
  operation: OperationDecl;
  label: string;
  textInput: string;
  draftState: string | null;
  quoteInput: string | null;
}

/** Resolved person-picked secondary slot (e.g. a runner/harness slot) and its installed fillers. */
export interface ResolvedPickerSlot {
  slot: SlotDecl;
  label: string;
  fillers: { plugin: string; label: string; target: string }[];
}

/**
 * Find the composer operation for `kind` from `plugin.slots` using the host's
 * checked `slot.composer` bindings (r10). Zero heuristics or hardcoded names.
 */
export function resolveComposerSlot(plugin: PluginDecl, kind: KindDecl): ResolvedComposer | null {
  for (const slot of Object.values(plugin.slots)) {
    if (slot.cardinality !== "single" || !slot.composer) continue;
    const filler =
      slot.fillers.find((f) => f.plugin === slot.recommended && f.kind === "operation") ??
      slot.fillers.find((f) => f.kind === "operation");
    if (!filler) continue;
    const opName = filler.target.includes(".") ? filler.target.split(".").pop()! : filler.target;
    const op = kind.operations[opName];
    if (!op) continue;
    return {
      slot,
      operation: op,
      label: slot.label?.trim() || humanize(slot.name),
      textInput: slot.composer.text,
      draftState: slot.composer.draft,
      quoteInput: slot.composer.quote,
    };
  }
  return null;
}

/**
 * Find any other person-picked single slot on `plugin` that has installed operation fillers,
 * using the slot's declared `label` and each filler's declared `as` `label`.
 */
export function resolvePickerSlot(
  plugin: PluginDecl,
  excludeSlotName?: string,
): ResolvedPickerSlot | null {
  for (const slot of Object.values(plugin.slots)) {
    if (slot.name === excludeSlotName) continue;
    if (slot.cardinality !== "single" || slot.pickedBy !== "person") continue;
    const opFillers = slot.fillers.filter((f) => f.kind === "operation");
    if (!opFillers.length) continue;
    return {
      slot,
      label: slot.label?.trim() || humanize(slot.name),
      fillers: opFillers.map((f) => ({
        plugin: f.plugin,
        label: f.label?.trim() || f.plugin,
        target: f.target,
      })),
    };
  }
  return null;
}

/**
 * Find the unambiguous viewer-state read-position binding on `kind`: a `set` operation with a
 * single `ask` assignment targeting `state.<key>` whose declared type is `"record anchor"`.
 * Returns `null` if none or more than one exists (never matches on operation or field names).
 */
export function resolveReadPositionStateKey(kind: KindDecl): string | null {
  const keys = new Set<string>();
  for (const op of Object.values(kind.operations)) {
    if (op.form !== "set") continue;
    const assigns = op.assignments ?? (op.target ? [{ target: op.target, value: op.value, ask: op.value === "ask" }] : []);
    if (assigns.length !== 1) continue;
    const a = assigns[0]!;
    if (!a.ask || !a.target.startsWith("state.")) continue;
    const key = a.target.slice("state.".length);
    if (kind.state[key]?.type === "record anchor") {
      keys.add(key);
    }
  }
  return keys.size === 1 ? [...keys][0]! : null;
}

/** Return the initials (1–2 characters) for a person's display name. */
export function initials(name: string): string {
  const parts = name.trim().split(/\s+/).filter(Boolean);
  if (!parts.length) return "?";
  if (parts.length === 1) return parts[0]!.slice(0, 2).toUpperCase();
  return (parts[0]![0]! + parts[parts.length - 1]![0]!).toUpperCase();
}

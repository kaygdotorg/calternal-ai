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

/** Extract a numeric badge from computed fields listed in `row:` using `FieldDecl.computed` and its declared field name. */
export function rowBadge(row: RowData, kind?: KindDecl): { value: number; name: string } | null {
  if (!kind) return null;
  for (const col of kind.row) {
    const decl = kind.fields[col];
    if (decl?.computed || decl?.formula || decl?.type === "number") {
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

/** Resolved composer slot metadata from `s.declarations`. */
export interface ResolvedComposer {
  slot: SlotDecl;
  operation: OperationDecl;
  textInput: string;
  quoteInput: string | null;
}

/** Resolved person-picked secondary slot (e.g. a runner/harness slot) and its installed fillers. */
export interface ResolvedPickerSlot {
  slot: SlotDecl;
  fillers: { plugin: string; label: string; target: string }[];
}

/**
 * Find the composer operation for `kind` from `plugin.slots` (any single slot whose installed
 * operation filler targets an operation declared on `kind`). Zero hardcoded slot or plugin names.
 */
export function resolveComposerSlot(plugin: PluginDecl, kind: KindDecl): ResolvedComposer | null {
  for (const slot of Object.values(plugin.slots)) {
    if (slot.cardinality !== "single") continue;
    const filler =
      slot.fillers.find((f) => f.plugin === slot.recommended && f.kind === "operation") ??
      slot.fillers.find((f) => f.kind === "operation");
    if (!filler) continue;
    const opName = filler.target.includes(".") ? filler.target.split(".").pop()! : filler.target;
    const op = kind.operations[opName];
    if (!op) continue;
    const inputs = Object.values(op.inputs ?? {});
    const textField =
      inputs.find((f) => f.name !== "item" && (f.type === "long text" || f.type === "text") && f.required) ??
      inputs.find((f) => f.name !== "item" && (f.type === "long text" || f.type === "text"));
    const quoteField = inputs.find((f) => f.type.startsWith("link(") && f.type.includes("records"));
    return {
      slot,
      operation: op,
      textInput: textField?.name ?? "text",
      quoteInput: quoteField?.name ?? null,
    };
  }
  return null;
}

/**
 * Find any other person-picked single slot on `plugin` that has installed operation fillers
 * (e.g. an optional agent runner slot) and resolve each filler's display name from `allPlugins`.
 */
export function resolvePickerSlot(
  plugin: PluginDecl,
  allPlugins: PluginDecl[],
  excludeSlotName?: string,
): ResolvedPickerSlot | null {
  for (const slot of Object.values(plugin.slots)) {
    if (slot.name === excludeSlotName) continue;
    if (slot.cardinality !== "single" || slot.pickedBy !== "person") continue;
    const opFillers = slot.fillers.filter((f) => f.kind === "operation");
    if (!opFillers.length) continue;
    return {
      slot,
      fillers: opFillers.map((f) => {
        const owner = allPlugins.find((p) => p.id === f.plugin);
        return {
          plugin: f.plugin,
          label: owner?.name || humanize(f.plugin),
          target: f.target,
        };
      }),
    };
  }
  return null;
}

/** Return the initials (1–2 characters) for a person's display name. */
export function initials(name: string): string {
  const parts = name.trim().split(/\s+/).filter(Boolean);
  if (!parts.length) return "?";
  if (parts.length === 1) return parts[0]!.slice(0, 2).toUpperCase();
  return (parts[0]![0]! + parts[parts.length - 1]![0]!).toUpperCase();
}

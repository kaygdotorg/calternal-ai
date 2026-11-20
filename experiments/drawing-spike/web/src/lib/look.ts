// Turns a kind's look (named places that name fields) and a row's filled values into what a view draws. Generic: the only
// vocabulary is the look places (title, detail, leading, trailing, image), the marks (dot, count), `when` predicates, and
// the value shapes the host sends. Also derives the actions and forms a kind's operations imply. Throwaway spike code.
import { kindOf, pluginOf, viewer, type Json, type Row, type Value, type Person } from "./model";

const tz = viewer.tz, now = new Date(viewer.now);
const day = (d: Date) => new Intl.DateTimeFormat("en-CA", { timeZone: tz, year: "numeric", month: "2-digit", day: "2-digit" }).format(d);
const daysAgo = (d: Date) => Math.round((Date.parse(day(now)) - Date.parse(day(d))) / 86_400_000);

/** A time as a list shows it: the clock today, "Yesterday", a weekday this week, else a date. */
export function shortTime(iso: string): string {
  const d = new Date(iso), n = daysAgo(d);
  if (n === 0) return clock(iso);
  if (n === 1) return "Yesterday";
  if (n < 7) return new Intl.DateTimeFormat("en-GB", { timeZone: tz, weekday: "short" }).format(d);
  return new Intl.DateTimeFormat("en-GB", { timeZone: tz, day: "numeric", month: "short", ...(d.getFullYear() !== now.getFullYear() ? { year: "numeric" } : {}) }).format(d);
}
/** The clock time of an instant in the viewer's zone. */
export const clock = (iso: string) => new Intl.DateTimeFormat("en-GB", { timeZone: tz, hour: "2-digit", minute: "2-digit" }).format(new Date(iso));
/** The day line of a timeline: "Today", "Yesterday", or the full date. */
export function dayLine(iso: string): string {
  const d = new Date(iso), n = daysAgo(d);
  return n === 0 ? "Today" : n === 1 ? "Yesterday" : new Intl.DateTimeFormat("en-GB", { timeZone: tz, weekday: "long", day: "numeric", month: "long" }).format(d);
}
/** The calendar day of an instant in the viewer's zone, for grouping. */
export const dayKey = (iso: string) => day(new Date(iso));

/** People other than the viewer: a person list in a look is about who else is there. */
export const others = (v: Value): Person[] => (v && typeof v === "object" && "people" in v ? v.people.filter((p) => !p.me) : []);
/** A value as text, or "" when it has nothing to show. */
export function text(v: Value | undefined): string {
  if (v == null || v === false) return "";
  if (typeof v === "object") return "at" in v ? shortTime(v.at) : others(v).map((p) => p.name).join(", ");
  return v === true ? "" : String(v);
}
/** A one-field predicate: `field`, `!field`, or `own` (the viewer wrote it). */
export function when(cond: string | undefined, values: Record<string, Value>, own = false): boolean {
  if (!cond) return true;
  const neg = cond.startsWith("!"), name = neg ? cond.slice(1) : cond;
  const v = name === "own" ? own : !!values[name];
  return neg ? !v : v;
}

export type Mark = { dot: true } | { count: number; quiet: boolean } | { text: string };
export type Drawn = { title: string; detail: string[]; avatars: Person[]; icon: string; trailing: Mark[]; image?: string };

/** Fills one look of a row's kind. A missing look falls back as drawing.md 1.3 says: card from row, row from title. */
export function draw(row: Row, look: "row" | "card" | "cell"): Drawn {
  const k = kindOf(row.kind) ?? {}, l: Json = k[look] ?? k.row ?? { title: "title" }, v = row.values;
  const names = Array.isArray(l.title) ? l.title : [l.title ?? "title"];
  const title = names.map((n: string) => text(v[n])).find((t: string) => t) || "Untitled";
  const detail = (l.detail ?? []).map((n: string) => text(v[n])).filter((t: string) => t);
  const avatars = l.leading?.avatar ? others(v[l.leading.avatar]) : [];
  const trailing: Mark[] = [];
  for (const t of l.trailing ?? []) {
    if (typeof t === "string") { const s = text(v[t]); if (s) trailing.push({ text: s }); }
    else if (t.dot && when(t.dot.when, v)) trailing.push({ dot: true });
    else if (t.count) { const n = Number(v[t.count] ?? 0); if (n > 0) trailing.push({ count: n, quiet: !!v[t.quiet] }); }
  }
  return { title, detail, avatars, icon: pluginOf(row.kind)?.icon ?? "file", trailing, image: l.image ? text(v[l.image]) : undefined };
}

export type Action = { id: string; label: string; icon?: string; place: string; op: string; inputs: Record<string, Json>; confirm?: string; defaults?: Record<string, string>; pick?: Record<string, string>; compose?: Record<string, string> };

/** The row actions a kind's declarative `set` operations imply, filtered by their `when` on this row. */
export function rowActions(row: Row): Action[] {
  const [pid, kname] = row.kind.split("/"), ops = pluginOf(row.kind)?.operations ?? {};
  return Object.entries<Json>(ops).filter(([, o]) => o.set === kname && when(o.when, row.values))
    .map(([id, o]) => ({ id, label: o.title ?? id, icon: o.icon, place: o.place ?? "menu", op: `${pid}/${id}`, inputs: { item: row.item } }));
}
/** The "New" action of a collection: the app's declarative `create` operation for the collection's kind, if any. */
export function newAction(app: Json, c: Json): Action | null {
  const hit = Object.entries<Json>(app.operations ?? {}).find(([, o]) => o.create && o.create === c.kind);
  return hit ? { id: hit[0], label: hit[1].title ?? "New", icon: "plus", place: "primary", op: `${app.id}/${hit[0]}`, inputs: {} } : null;
}
/** The fields a derived form asks for: the operation's inputs not already supplied, typed from the operation or the kind. */
export function formFields(op: Json, supplied: Record<string, Json>, kind?: string): { name: string; type: string; label: string; optional: boolean }[] {
  const k = kind ? kindOf(kind) : null;
  const decl: Record<string, Json> = op.input ?? Object.fromEntries((op.inputs ?? []).map((n: string) => [n, k?.fields?.[n] ?? { type: "text" }]));
  return Object.entries(decl).filter(([n]) => !(n in supplied)).map(([name, f]) => ({ name, type: f.type, optional: !!f.optional, label: f.label ?? name[0]!.toUpperCase() + name.slice(1) }));
}

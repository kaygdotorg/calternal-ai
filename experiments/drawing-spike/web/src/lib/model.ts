// The client's view of what the host hands it: the checked declarations of the installed plugins, and windows of values
// filled in for the viewer. Nothing here knows any plugin: names are looked up, never written in. Throwaway spike code.
import declarations from "../../../data/declarations.json";
import fixtures from "../../../data/fixtures.json";

export type Json = any; // declarations are data checked by the host's validator; the spike trusts them

/** A value filled in by the host: a scalar, an instant, or a list of people (with `me` on the viewer). */
export type Value = string | number | boolean | null | { at: string } | { people: Person[] };
export type Person = { id: string; name: string; me?: boolean };
/** One item in a window: its file, its kind (`plugin/kind`) and the values its looks name. */
export type Row = { item: string; kind: string; values: Record<string, Value> };
export type Stamp = { person: Person; run?: { name: string; icon: string }; at: string };
export type Rec = { anchor: string; body: string; fields: Record<string, Json>; stamp: Stamp; edited?: boolean; redacted?: boolean };
export type Ref = { author: string; excerpt: string } | { gone: true };
export type RecPage = { records: Rec[]; refs: Record<string, Ref>; before: string | null };
export type RecordWindow = { position: string; unread: number; page: RecPage; pages: Record<string, RecPage> };

const plugins: Json[] = declarations.plugins;
export const viewer = fixtures.viewer as Person & { tz: string; now: string };
export const slots = fixtures.slots as Record<string, Json>;

/** The plugin that declared an id (`reading-list`), or the owner of a qualified name (`reading-list/reading`). */
export const pluginOf = (name: string): Json => plugins.find((p) => p.id === name.split("/")[0]);
/** A kind's declaration by qualified name. */
export const kindOf = (kind: string): Json => pluginOf(kind)?.kinds?.[kind.split("/")[1]!];
/** An operation's declaration by qualified name (`calternal-chat/send`). */
export const opOf = (op: string): Json => pluginOf(op)?.operations?.[op.split("/")[1]!];
/** The apps: plugins that ship a sidebar. */
export const apps = (): Json[] => plugins.filter((p) => p.sidebar);
/** A collection's id: the owning plugin and its title (a package collection has no file of its own yet). */
export const collectionId = (app: Json, c: Json) => `${app.id}/${c.collection}`;
/** The host's window for a collection, as the viewer. */
export const windowOf = (id: string): { rows: Row[]; count?: number } => (fixtures.windows as Json)[id] ?? { rows: [] };
/** The record window of a record file, as the viewer, or null when it has no records. */
export const recordsOf = (item: string): RecordWindow | null => (fixtures.records as Json)[item] ?? null;
/** The part the viewer picked for a slot (`plugin/slot`), and the declaration it fills it with. */
export const partFor = (slot: string): Json => {
  const id = slots[slot];
  return plugins.find((p) => p.id === id)?.fills?.[slot] ?? null;
};
/** Every field a kind declares, with the record fields when it is a record kind. */
export const fieldOf = (kind: string, name: string): Json => kindOf(kind)?.fields?.[name];

/**
 * The person's arrangement of sidebars and collections: the one piece of state the mockup saves besides the apps' order. Everything a person can do to a sidebar is a method here, so every sidebar (every app's, and the ones that belong to no app) works the same.
 *  - A SIDEBAR is an ordered list of sections; a SECTION is a collection (by id) with the person's own title, small view, sort, filter, fold and hidden flags. A sidebar is itself a collection of collections: `sidebarFile` writes it as the Markdown file it would be.
 *  - A COLLECTION's items are in the order the person left them; an item can be moved to another collection (dragging a row between sections), pinned (copied onto a pinned strip) or unpinned.
 *  - New sections are collections the person made; each Cal's icon and colour are theirs; a card's status on a board is theirs.
 */
import { COLLS, DEFAULT_SIDEBARS, ITEMS, SEED_CUSTOM, type Coll, type Item } from "./data/collections";

export interface SecRef { cid: string; /** The person's own title for this section. */ name?: string; view?: "list" | "strip"; shut?: boolean; /** Hidden from the sidebar (kept in "Hidden sections", with a way back). */ hidden?: boolean; sort?: "manual" | "title"; /** Show only unread or open items. */ unread?: boolean }
export interface CalStyle { icon: string; hue: number }
type Saved = { side: Record<string, SecRef[]>; custom: { id: string; name: string }[]; order: Record<string, string[]>; extra: Record<string, Coll>; cal: Record<string, CalStyle>; status: Record<string, string>; fresh?: boolean };
const KEY = "calternal.mockup.arrangement2";
const q = new URLSearchParams(typeof location === "undefined" ? "" : location.search);
const defaults = (): Saved => ({
  side: { ...Object.fromEntries(Object.entries(DEFAULT_SIDEBARS).map(([k, v]) => [k, v.map((cid) => ({ cid }))])), [SEED_CUSTOM.id]: SEED_CUSTOM.sections.map((cid) => ({ cid })) },
  custom: [{ id: SEED_CUSTOM.id, name: SEED_CUSTOM.name }], order: {}, extra: {}, cal: { "c-rc": { icon: "claude", hue: 28 }, "c-rs": { icon: "claude", hue: 205 } }, status: {},
});
function load(): Saved {
  if (q.has("fresh")) return defaults();
  try { const a = JSON.parse(localStorage.getItem(KEY) ?? "null"); if (a?.side && a.custom && a.order && a.extra) return { ...defaults(), ...a }; } catch { /* defaults */ }
  return defaults();
}

class Arrange {
  private s = $state<Saved>(load());
  private save() { try { localStorage.setItem(KEY, JSON.stringify(this.s)); } catch { /* private mode */ } }
  get custom() { return this.s.custom; }
  get cal() { return this.s.cal; }
  /** The ids of the collections the person made. */
  extraIds(): Record<string, true> { return Object.fromEntries(Object.keys(this.s.extra).map((k) => [k, true as const])); }
  /** The collection by id: one the person made, else a built-in. */
  coll(cid: string): Coll | undefined { return this.s.extra[cid] ?? COLLS[cid]; }
  sections(sid: string): SecRef[] { return this.s.side[sid] ?? []; }
  /** The items of a collection in the person's order (its own order until they move something), after the section's sort and filter. */
  items(cid: string, ref?: SecRef): Item[] {
    const c = this.coll(cid); if (!c) return [];
    let list = (this.s.order[cid] ?? c.items.map((i) => i.id)).map((id) => ITEMS[id]!).filter(Boolean);
    if (ref?.unread) list = list.filter((i) => i.unread || i.n || (i.kind === "task" && i.status !== "Done"));
    if (ref?.sort === "title") list = [...list].sort((a, b) => a.title.localeCompare(b.title));
    return list.map((i) => (this.s.status[i.id] ? { ...i, status: this.s.status[i.id] as Item["status"] } : i));
  }
  ids(cid: string): string[] { const c = this.coll(cid); return this.s.order[cid] ?? c?.items.map((i) => i.id) ?? []; }
  statusOf(id: string): Item["status"] | undefined { return (this.s.status[id] as Item["status"]) ?? ITEMS[id]?.status; }
  setStatus(id: string, st: string) { this.s.status[id] = st; this.save(); }

  /** Puts a collection into a sidebar as a section at `index` (one already there moves instead: a sidebar holds each once). */
  insertSection(sid: string, index: number, cid: string) {
    const list = [...this.sections(sid)], from = list.findIndex((x) => x.cid === cid), ref = from >= 0 ? list[from]! : { cid };
    if (from >= 0) { list.splice(from, 1); if (from < index) index--; }
    list.splice(Math.max(0, Math.min(list.length, index)), 0, { ...ref, hidden: false }); this.s.side[sid] = list; this.save();
  }
  removeSection(sid: string, cid: string) { this.s.side[sid] = this.sections(sid).filter((x) => x.cid !== cid); this.save(); }
  patchSection(sid: string, cid: string, patch: Partial<SecRef>) { this.s.side[sid] = this.sections(sid).map((x) => (x.cid === cid ? { ...x, ...patch } : x)); this.save(); }
  /** A new empty section: a collection of the person's own, added at the end of the sidebar. */
  addSection(sid: string, name = "New section"): string {
    const id = `own-${Date.now().toString(36)}${Object.keys(this.s.extra).length}`;
    this.s.extra[id] = { id, name, hue: 205, small: "list", big: "list", items: [] }; this.s.side[sid] = [...this.sections(sid), { cid: id }]; this.save(); return id;
  }
  /** A new, empty sidebar that belongs to no app, named by the person. */
  addSidebar(name: string): string { const id = `cs-${Date.now().toString(36)}`; this.s.custom = [...this.s.custom, { id, name: name.trim() || "New sidebar" }]; this.s.side[id] = []; this.save(); return id; }
  /** Replaces a sidebar's sections wholesale (resetting to the space's sidebar, or taking it back after an undo). */
  setSections(sid: string, list: SecRef[]) { this.s.side[sid] = list.map((x) => ({ ...x })); this.save(); }
  /** Moves a section within a sidebar (its place in the sidebar's file). */
  moveSection(sid: string, from: number, to: number) { const l = [...this.sections(sid)], [x] = l.splice(from, 1); l.splice(Math.max(0, Math.min(l.length, to)), 0, x!); this.s.side[sid] = l; this.save(); }

  /** Moves a row to `index` of another collection (or within its own). A pinned strip copies instead (pinning keeps the item where it was), and a row dragged off a pinned strip is unpinned. */
  moveItem(id: string, from: string, to: string, index: number, copy = false) {
    const src = this.ids(from).filter((x) => x !== id), dst = from === to ? src : this.ids(to).filter((x) => x !== id);
    if (!copy && from !== to) this.s.order[from] = src; else if (from === to) this.s.order[from] = src;
    dst.splice(Math.max(0, Math.min(dst.length, index)), 0, id); this.s.order[to] = dst; this.save();
  }
  /** Takes an item out of a collection (unpin, or move it to nowhere). */
  dropItem(id: string, from: string) { this.s.order[from] = this.ids(from).filter((x) => x !== id); this.save(); }
  /** A new item at the top of a collection (the section's "add"): of the kind the collection mostly holds. */
  addItem(cid: string): string {
    const c = this.coll(cid)!, kind = (this.items(cid)[0]?.kind ?? "note") as Item["kind"], id = `new-${Date.now().toString(36)}`;
    const labels: Record<string, string> = { note: "Untitled note", mail: "New message", chat: "New chat", task: "New task", file: "New file", photo: "New photo", event: "New event", contact: "New contact", transaction: "New transaction", workout: "New workout", label: "New label", setting: "New setting" };
    ITEMS[id] = { id, kind, title: labels[kind] ?? "New item", sub: "Just now", when: "Now", hue: c.hue, status: kind === "task" ? "To do" : undefined, body: "" };
    this.s.order[cid] = [id, ...this.ids(cid)]; this.save(); return id;
  }
  setCal(id: string, patch: Partial<CalStyle>) { this.s.cal[id] = { ...(this.s.cal[id] ?? { icon: "spark", hue: 205 }), ...patch }; this.save(); }
  hiddenCount(sid: string) { return this.sections(sid).filter((x) => x.hidden).length; }
  /** Called by `registerApp` for an app's sidebar: sets it up when the person has no arrangement for it yet, or still has the generic stand-in's (`old`); an arrangement the person changed is left alone. Safe to call after the arrangement exists. */
  seed(sid: string, ids: string[], old?: string[]) {
    const cur = this.s.side[sid], same = cur && old && cur.length === old.length && cur.every((x, i) => x.cid === old[i] && !x.name && !x.view && !x.hidden && !x.shut);
    if (!cur || same) this.s.side[sid] = ids.map((cid) => ({ cid }));
  }
  reset() { this.s = defaults(); this.save(); }

  /** The Markdown file a sidebar is: frontmatter, then an ordered list of links to its collections (the person's own title and small view after each). Written from the live arrangement, so it is always what is stored. */
  sidebarFile(sid: string): { path: string; text: string } {
    const name = this.s.custom.find((c) => c.id === sid)?.name ?? sid[0]!.toUpperCase() + sid.slice(1), slug = name.toLowerCase().replace(/[^a-z0-9]+/g, "-");
    const rows = this.sections(sid).map((r) => { const c = this.coll(r.cid)!; const bits = [r.name ? `title: ${r.name}` : "", `view: ${r.view ?? c.small}`, r.shut ? "folded" : "", r.hidden ? "hidden" : "", r.sort === "title" ? "sort: title" : "", r.unread ? "filter: unread" : ""].filter(Boolean).join(", "); return `- [${c.name}](../collections/${c.id}.md) (${bits})`; });
    return { path: `sidebars/${slug}.md`, text: ["---", "kind: sidebar", `name: ${name}`, `for: ${this.s.custom.some((c) => c.id === sid) ? "none" : "app:" + sid}`, "updated: 2026-11-20", "---", "", `# ${name}`, "", "The sections of this sidebar, in order. Each link is a collection, kept as its own file.", "", ...rows, ""].join("\n") };
  }
}
export const arr = new Arrange();

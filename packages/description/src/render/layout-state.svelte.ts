/**
 * The person's own arrangement of a sidebar (DRAFT shape, docs: issue #99 and #78): which sections and subsections are folded or hidden, the order of sections within their parent, the order of rows in a section (and on the
 * pinned strip), titles the person gave, and folders (subsections the person made) with the rows placed in them. It is the person's alone and the host stores it for them (`layout.state` reads it, `layout.apply` writes the
 * whole of it); a description only says what may be changed (`customise`). Everything here is small and bounded: at most 64 sections or folders, 200 ordered row ids per section.
 */
import type { HostSession } from "../wire/session";

/** What is stored for one person and one sidebar. */
export interface LayoutSnapshot {
  /** Section ids in the person's order, by parent id (`""` for the top level). */
  order: Record<string, string[]>;
  collapsed: Record<string, boolean>;
  hidden: Record<string, boolean>;
  titles: Record<string, string>;
  /** Folders the person made: a subsection of `parent`. */
  folders: { id: string; title: string; parent: string }[];
  /** Row ids in the person's order, by section, folder or strip id. */
  rows: Record<string, string[]>;
  /** Which folder holds a row (a row is in one place). */
  placed: Record<string, string>;
}

const MAX_SECTIONS = 64, MAX_ROWS = 200;

/** The reactive arrangement and its saving. */
export class LayoutState implements LayoutSnapshot {
  order = $state<Record<string, string[]>>({});
  collapsed = $state<Record<string, boolean>>({});
  hidden = $state<Record<string, boolean>>({});
  titles = $state<Record<string, string>>({});
  folders = $state<{ id: string; title: string; parent: string }[]>([]);
  rows = $state<Record<string, string[]>>({});
  placed = $state<Record<string, string>>({});
  #timer = 0;
  #session: HostSession | null = null;
  /** Called once the stored arrangement has been read (the sidebar then draws it). */
  loaded = $state(false);

  /** Reads the person's stored arrangement; a host without one leaves the description's own order and folds. */
  async load(session: HostSession) {
    this.#session = session;
    try { const s = await session.exchange<Partial<LayoutSnapshot>>("layout.state", { type: "layout_state" }); this.#take(s); } catch { /* none stored: the description's defaults stand */ }
    this.loaded = true;
  }
  #take(s: Partial<LayoutSnapshot>) {
    this.order = s.order ?? {}; this.collapsed = s.collapsed ?? {}; this.hidden = s.hidden ?? {}; this.titles = s.titles ?? {}; this.folders = s.folders ?? []; this.rows = s.rows ?? {}; this.placed = s.placed ?? {};
  }
  snapshot(): LayoutSnapshot { return JSON.parse(JSON.stringify({ order: this.order, collapsed: this.collapsed, hidden: this.hidden, titles: this.titles, folders: this.folders, rows: this.rows, placed: this.placed })); }
  /** Saves the arrangement shortly after the last change (one write for a burst). */
  save() { clearTimeout(this.#timer); this.#timer = window.setTimeout(() => { void this.#session?.exchange("layout.apply", { type: "layout_apply", layout: this.snapshot() }).catch(() => {}); }, 350); }

  /** The ids in the person's order, any not yet ordered following in their own order. */
  sorted(parent: string, ids: string[]): string[] {
    const o = this.order[parent] ?? [];
    return [...ids].sort((a, b) => { const x = o.indexOf(a), y = o.indexOf(b); return (x < 0 ? 1e6 : x) - (y < 0 ? 1e6 : y) || ids.indexOf(a) - ids.indexOf(b); });
  }
  /** Places `id` among `siblings` (all ids of that parent now shown) before `before` (or last). */
  moveSection(parent: string, siblings: string[], id: string, before: string | null) {
    const cur = this.sorted(parent, siblings).filter((x) => x !== id), at = before ? cur.indexOf(before) : -1;
    cur.splice(at < 0 ? cur.length : at, 0, id); this.order[parent] = cur.slice(0, MAX_SECTIONS); this.save();
  }
  /** Places a row among `ids` (the rows shown in that section, in order) before `before` (or last). */
  moveRow(section: string, ids: string[], id: string, before: string | null) {
    const cur = ids.filter((x) => x !== id), at = before ? cur.indexOf(before) : -1;
    cur.splice(at < 0 ? cur.length : at, 0, id); this.rows[section] = cur.slice(0, MAX_ROWS); this.save();
  }
  /** Rows in the person's order for a section. */
  rowOrder(section: string, ids: string[]): string[] { const o = this.rows[section] ?? []; return [...ids].sort((a, b) => { const x = o.indexOf(a), y = o.indexOf(b); return (x < 0 ? 1e6 : x) - (y < 0 ? 1e6 : y) || ids.indexOf(a) - ids.indexOf(b); }); }
  fold(id: string, to: boolean) { this.collapsed[id] = to; this.save(); }
  hide(id: string, to: boolean) { if (to) this.hidden[id] = true; else delete this.hidden[id]; this.save(); }
  rename(id: string, title: string) { const t = title.trim().slice(0, 60); if (t) this.titles[id] = t; else delete this.titles[id]; this.save(); }
  addFolder(parent: string, title: string): string {
    if (this.folders.length >= MAX_SECTIONS) return "";
    const id = `f-${crypto.randomUUID().slice(0, 8)}`; this.folders.push({ id, title: title.trim().slice(0, 60) || "Folder", parent }); this.save(); return id;
  }
  removeFolder(id: string) { this.folders = this.folders.filter((f) => f.id !== id); for (const [r, f] of Object.entries(this.placed)) if (f === id) delete this.placed[r]; delete this.rows[id]; delete this.collapsed[id]; this.save(); }
  place(row: string, folder: string | null) { if (folder) this.placed[row] = folder; else delete this.placed[row]; this.save(); }
}

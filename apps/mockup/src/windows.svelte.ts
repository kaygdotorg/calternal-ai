/**
 * The canvas's windows (docs/ui-shell-status.md, "the middle area is the canvas"): a strip of columns, each a window that holds an app's screen, a collection shown big, or one item. One window per address; a window opens immediately
 * right of the focused one and takes focus; closing gives focus to the neighbour on the right, else the left. Choosing from the card navigates the focused window; opening an item of another kind than the focused window's opens a second window
 * beside it, and the sidebar does not change. The mockup keeps the model small: no stacked windows in a column, no preset widths beyond "one fills, two share, more scroll".
 */
import { ITEMS, KIND_APP, type Item } from "./data/collections";

export type WinKind = "app" | "collection" | "item";
export interface Win { id: string; kind: WinKind; ref: string; /** In an app window, the item open in it (a chat thread, a note). */ sel: string | null }
let seq = 0;

class Windows {
  list = $state<Win[]>([]);
  focus = $state<string | null>(null);
  /** The window shown full screen, hiding the others (they stay open). */
  full = $state<string | null>(null);
  /** The first window's share of the canvas when two fit: windows alternate between it and the rest, so ANY two neighbours fill the canvas exactly (the one divider changes it). */
  split = $state(0.5);
  get focused(): Win | undefined { return this.list.find((w) => w.id === this.focus); }
  /** What the sidebar marks as selected: the item open in the focused window. */
  get selected(): string | null { const f = this.focused; return f ? (f.kind === "item" ? f.ref : f.sel) : null; }
  private find(kind: WinKind, ref: string) { return this.list.find((w) => w.kind === kind && w.ref === ref); }
  private make(kind: WinKind, ref: string, sel: string | null = null): Win { return { id: `w${++seq}`, kind, ref, sel }; }
  /** Opens the address in the focused window (a card choice), or focuses the window that already shows it; with no window, makes one. */
  navigate(kind: WinKind, ref: string) {
    const have = this.find(kind, ref); if (have) { this.focus = have.id; return; }
    const f = this.focused;
    if (f) { f.kind = kind; f.ref = ref; f.sel = null; } else { const w = this.make(kind, ref); this.list = [w]; this.focus = w.id; }
    this.full = null;
  }
  /** Opens the address in a new window right of the focused one; without `force` an address already open is focused instead. */
  beside(kind: WinKind, ref: string, force = false) {
    const have = this.find(kind, ref); if (have && !force) { this.focus = have.id; return; }
    const w = this.make(kind, ref), at = this.list.findIndex((x) => x.id === this.focus) + 1;
    this.list.splice(at, 0, w); this.focus = w.id; this.full = null;
  }
  /** An item chosen in a sidebar section or a big view: in place when the focused window is that kind's own app (or another item of that app), else in a window beside. */
  openItem(item: Item) {
    const app = KIND_APP[item.kind], f = this.focused, have = this.find("item", item.id);
    if (have) { this.focus = have.id; return; }
    if (f && app && f.kind === "app" && f.ref === app) { f.sel = item.id; return; }
    if (f && f.kind === "item" && app && KIND_APP[ITEMS[f.ref]?.kind ?? "label"] === app) { f.ref = item.id; return; }
    if (!f) { this.navigate("item", item.id); return; }
    this.beside("item", item.id);
  }
  close(id: string) {
    const i = this.list.findIndex((w) => w.id === id); if (i < 0) return;
    const next = this.list[i + 1] ?? this.list[i - 1];
    this.list.splice(i, 1); if (this.focus === id) this.focus = next?.id ?? null; if (this.full === id) this.full = null;
  }
  /** Moves a window to column `to` (dragging its title, or Mod+Alt+Shift+Left/Right). */
  moveTo(id: string, to: number) { const i = this.list.findIndex((w) => w.id === id); if (i < 0) return; const [w] = this.list.splice(i, 1); this.list.splice(Math.max(0, Math.min(this.list.length, to)), 0, w!); }
  clear() { this.list = []; this.focus = null; this.full = null; }
  /** Focuses the window left or right of the focused one (Alt+Shift+Left/Right). */
  step(d: -1 | 1) { const i = this.list.findIndex((w) => w.id === this.focus), n = this.list[i + d]; if (n) this.focus = n.id; }
}
export const wins = new Windows();

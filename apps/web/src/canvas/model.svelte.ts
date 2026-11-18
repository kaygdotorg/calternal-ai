/**
 * The canvas's state (docs/ui-shell-status.md, "The canvas experiment"): the columns and their windows, which window has focus, which is maximised, and the strip's scroll position. The shell's window area (Appearance, Windows turns it on or off; `?canvas=0` and `?canvas=1` override it for a visit); the arrangement is kept in the browser (`cal.canvas`, JSON because it is throwaway experiment state, not the person's userdata). Pure placement arithmetic is in layout.ts.
 *
 * Addresses: a window's address is `/<kind>/<item>` and there is one window per address, so a deep link always has exactly one window to open: `show` focuses it, or adds one beside the focused column. The address bar follows
 * the focused window (Canvas.svelte navigates on focus), so the sidebars, which are chosen by the address's kind, follow the focused window's app with no code of their own.
 */
import { serverMode } from "../server/client";
import { dragBoundary, nextPreset, type Col, type Win } from "./layout";
import { canvasPrefs } from "./prefs.svelte";

const KEY = "cal.canvas";
/**
 * Whether windows are shown on the canvas now: the person's setting (Appearance, Windows; the build decides the default, see canvas/prefs.svelte.ts), unless the address says otherwise for this visit: `?canvas=0` (one card) or `?canvas=1` (the canvas), kept for the tab so
 * tests and links behave the same after the first navigation drops the query. Reactive: switching the setting changes the shell's window area at once.
 */
export function canvasActive(): boolean {
  return override() ?? canvasPrefs.enabled;
}
let seen: boolean | null | undefined;
function override(): boolean | null {
  if (seen !== undefined) return seen;
  try {
    const q = new URLSearchParams(location.search).get("canvas");
    if (q === "1") sessionStorage.setItem(KEY + ".on", "1");
    if (q === "0") sessionStorage.setItem(KEY + ".on", "0");
    const v = sessionStorage.getItem(KEY + ".on");
    seen = v === "1" ? true : v === "0" ? false : null;
  } catch { seen = null; }
  return seen;
}

let n = 0;
const uid = (p: string) => `${p}${Date.now().toString(36)}${(n++).toString(36)}`;

class CanvasModel {
  cols = $state<Col[]>([]);
  /** The focused window's id. */
  focus = $state<string | null>(null);
  /** The maximised window's id: it fills the outer card (merged into it) until restored. */
  max = $state<string | null>(null);
  /** The scroll position, kept here so a reload restores it; written by Canvas.svelte at rest. */
  x = 0;

  constructor() {
    try {
      const s = JSON.parse(localStorage.getItem(KEY) ?? "null") as { cols: Col[]; focus: string | null; x?: number } | null;
      if (s?.cols?.length && s.cols.every((c) => c && "p" in c && Array.isArray(c.wins) && c.wins.every((w) => w.id))) { this.cols = s.cols; this.focus = s.focus; this.x = s.x ?? 0; }
    } catch { /* nothing kept */ }
  }
  save(): void { try { localStorage.setItem(KEY, JSON.stringify({ cols: this.cols, focus: this.focus, x: this.x })); } catch { /* storage is off */ } }

  all(): Win[] { return this.cols.flatMap((c) => c.wins); }
  win(id: string | null): Win | undefined { return id ? this.all().find((w) => w.id === id) : undefined; }
  colIndexOf(id: string | null): number { return this.cols.findIndex((c) => c.wins.some((w) => w.id === id)); }
  /** The window of each app that had focus last (so choosing an app in the capsule, which has no item, goes back to where the person was in it). */
  private lastByKind: Record<string, string> = {};
  /** Remembers that this window now has focus. */
  noteFocus(id: string | null): void { const w = this.win(id); if (w) this.lastByKind[w.kind] = w.id; }
  /** The window of `kind` that had focus last, if it is still open. */
  lastOfKind(kind: string): Win | undefined { return this.win(this.lastByKind[kind] ?? null) ?? this.all().find((w) => w.kind === kind); }
  find(kind: string, item: string): Win | undefined { return this.all().find((w) => w.kind === kind && w.item === item); }
  address(w: Win): string { return w.item ? `/${w.kind}/${w.item}` : `/${w.kind}`; }

  private make(kind: string, item: string): Win { return { id: uid("w"), kind, item, weight: 1, locked: false }; }

  /** Opens an address: its window gets focus, or a new column opens right of the focused one. Returns the window. One window per address. */
  show(kind: string, item: string): Win {
    // Settings is one window: moving to another settings page changes what it shows (its address is `/settings/<page>`), it never opens a second Settings window.
    const have = kind === "settings" ? this.all().find((w) => w.kind === "settings") : this.find(kind, item);
    if (have) { if (kind === "settings") have.item = item; this.focus = have.id; this.save(); return have; }
    return this.addRight(kind, item);
  }
  /** A new column immediately right of the focused one (or at the end) holding one window; it gets focus (niri: a new window opens next to the focused column). */
  addRight(kind: string, item: string): Win {
    const w = this.make(kind, item);
    const at = this.colIndexOf(this.focus) + 1 || this.cols.length;
    const next = [...this.cols];
    next.splice(at, 0, { id: uid("c"), p: canvasPrefs.width, wins: [w] });
    this.cols = next; this.focus = w.id; this.max = null; this.save();
    return w;
  }
  /** A new window below the focused one, in the same column; it gets focus. */
  addBelow(kind: string, item: string): Win {
    const i = this.colIndexOf(this.focus);
    if (i < 0) return this.addRight(kind, item);
    const w = this.make(kind, item);
    const at = this.cols[i]!.wins.findIndex((x) => x.id === this.focus) + 1;
    this.cols = this.cols.map((c, k) => (k === i ? { ...c, wins: [...c.wins.slice(0, at), w, ...c.wins.slice(at)] } : c));
    this.focus = w.id; this.max = null; this.save();
    return w;
  }
  /** Closes a window; an empty column goes with it; focus goes to the column on its right, else on its left (the same row of the stack where there is one). */
  close(id: string): void {
    const i = this.colIndexOf(id);
    if (i < 0) return;
    const was = this.cols[i]!;
    const next = this.cols.map((c) => ({ ...c, wins: c.wins.filter((w) => w.id !== id) })).filter((c) => c.wins.length);
    this.cols = next;
    if (this.max === id) this.max = null;
    if (this.focus === id) {
      const stay = was.wins.length > 1 ? next.find((c) => c.id === was.id) : undefined;
      const k = Math.min(i, next.length - 1);
      this.focus = (stay ?? next[k])?.wins[0]?.id ?? null;
    }
    this.save();
  }
  setFocus(id: string): void { if (this.focus !== id) { this.focus = id; this.save(); } }
  /** Focus the column `by` places left (-1) or right (1) of the focused one, on the same row of its stack where there is one. Returns the window, or null at the end. */
  focusColumn(by: -1 | 1): Win | null {
    const i = this.colIndexOf(this.focus);
    const c = this.cols[i + by];
    if (!c) return null;
    const row = Math.min(this.cols[i]?.wins.findIndex((w) => w.id === this.focus) ?? 0, c.wins.length - 1);
    this.focus = c.wins[row]!.id; this.save();
    return c.wins[row]!;
  }
  /** Focus the window above (-1) or below (1) in the focused column. */
  focusRow(by: -1 | 1): Win | null {
    const c = this.cols[this.colIndexOf(this.focus)];
    const k = (c?.wins.findIndex((w) => w.id === this.focus) ?? -1) + by;
    const w = c?.wins[k];
    if (!w) return null;
    this.focus = w.id; this.save();
    return w;
  }
  /** Moves the focused window's column one place left or right (niri's move-column). */
  moveColumn(by: -1 | 1): void {
    const i = this.colIndexOf(this.focus), j = i + by;
    if (i < 0 || j < 0 || j >= this.cols.length) return;
    const next = [...this.cols]; [next[i], next[j]] = [next[j]!, next[i]!];
    this.cols = next; this.save();
  }
  toggleMax(id: string): void { this.max = this.max === id ? null : id; this.focus = id; this.save(); }
  toggleLock(id: string): void { this.patch(id, (w) => ({ ...w, locked: !w.locked })); }
  private patch(id: string, f: (w: Win) => Win): void { this.cols = this.cols.map((c) => ({ ...c, wins: c.wins.map((w) => (w.id === id ? f(w) : w)) })); this.save(); }
  /** Sets a column's width as a proportion of the canvas. */
  setProportion(colId: string, p: number | null): void { this.cols = this.cols.map((c) => (c.id === colId ? { ...c, p } : c)); this.save(); }
  /** Cycles a column through the width presets (`current` is the proportion it has now). */
  cyclePreset(colId: string, current: number, back = false): void { this.setProportion(colId, nextPreset(current, back)); }
  /** Gives the windows of a column equal heights. */
  equalHeights(colId: string): void { this.cols = this.cols.map((c) => (c.id === colId ? { ...c, wins: c.wins.map((w) => ({ ...w, weight: 1 })) } : c)); this.save(); }
  /** Moves the boundary under window `i` of a column by `dy` px. */
  dragBoundary(colId: string, i: number, dy: number, h: number): void {
    this.cols = this.cols.map((c) => { if (c.id !== colId) return c; const ws = dragBoundary(c.wins, i, dy, h); return { ...c, wins: c.wins.map((w, k) => ({ ...w, weight: ws[k]! })) }; });
  }
}

/** The one canvas of the page. */
export const canvas = new CanvasModel();

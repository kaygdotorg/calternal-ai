/**
 * The canvas's state (docs/ui-shell-status.md, "The canvas experiment"): the columns and their windows, which window has focus, which is maximised, and the strip's scroll position. Experiment only, behind `?canvas=1` in the
 * fixtures build; the arrangement is kept in the browser (`cal.canvas`, JSON because it is throwaway experiment state, not the person's userdata). Pure placement arithmetic is in layout.ts.
 *
 * Addresses: a window's address is `/<kind>/<item>` and there is one window per address, so a deep link always has exactly one window to open: `show` focuses it, or adds one beside the focused column. The address bar follows
 * the focused window (Canvas.svelte navigates on focus), so the sidebars, which are chosen by the address's kind, follow the focused window's app with no code of their own.
 */
import { serverMode } from "../server/client";
import { dragBoundary, MIN_W, type Col, type Win } from "./layout";

const KEY = "cal.canvas";
/** Whether the experiment is on: `?canvas=1` once (remembered for the visit), `?canvas=0` to leave. Fixtures build only. */
export function canvasOn(live: boolean): boolean {
  if (live) return false;
  try {
    const q = new URLSearchParams(location.search).get("canvas");
    if (q === "1") sessionStorage.setItem(KEY + ".on", "1");
    if (q === "0") sessionStorage.removeItem(KEY + ".on");
    return sessionStorage.getItem(KEY + ".on") === "1";
  } catch { return false; }
}

/** Read once when the page loads, while the address still carries `?canvas=1` (the first navigation drops the query). */
export const canvasEnabled = canvasOn(serverMode());

let n = 0;
const uid = (p: string) => `${p}${Date.now().toString(36)}${(n++).toString(36)}`;

class CanvasModel {
  cols = $state<Col[]>([]);
  /** The focused window's id. */
  focus = $state<string | null>(null);
  /** The maximised window's id: it fills the outer card (merged into it) until restored. */
  max = $state<string | null>(null);
  /** A window frosted by default (the global toggle); a window's own `frosted` flips it. */
  frostAll = $state(false);
  /** The scroll position, kept here so a reload restores it; written by Canvas.svelte at rest. */
  x = 0;

  constructor() {
    try {
      const s = JSON.parse(localStorage.getItem(KEY) ?? "null") as { cols: Col[]; focus: string | null; frostAll?: boolean; x?: number } | null;
      if (s?.cols?.length) { this.cols = s.cols; this.focus = s.focus; this.frostAll = !!s.frostAll; this.x = s.x ?? 0; }
    } catch { /* nothing kept */ }
  }
  save(): void { try { localStorage.setItem(KEY, JSON.stringify({ cols: this.cols, focus: this.focus, frostAll: this.frostAll, x: this.x })); } catch { /* storage is off */ } }

  all(): Win[] { return this.cols.flatMap((c) => c.wins); }
  win(id: string | null): Win | undefined { return id ? this.all().find((w) => w.id === id) : undefined; }
  colIndexOf(id: string | null): number { return this.cols.findIndex((c) => c.wins.some((w) => w.id === id)); }
  find(kind: string, item: string): Win | undefined { return this.all().find((w) => w.kind === kind && w.item === item); }
  address(w: Win): string { return `/${w.kind}/${w.item}`; }

  private make(kind: string, item: string): Win { return { id: uid("w"), kind, item, weight: 1, frosted: false, locked: false }; }
  private width(vw: number): number { return vw < 760 ? Math.max(MIN_W, vw - 24) : Math.min(560, Math.max(MIN_W, Math.round(vw * 0.42))); }

  /** Opens an address: its window gets focus, or a new column opens to the right of the focused one. Returns the window. One window per address. */
  show(kind: string, item: string, vw: number): Win {
    const have = this.find(kind, item);
    if (have) { this.focus = have.id; this.save(); return have; }
    return this.addRight(kind, item, vw);
  }
  /** A new column to the right of the focused one (or at the end) holding one window; it gets focus. */
  addRight(kind: string, item: string, vw: number): Win {
    const w = this.make(kind, item);
    const at = this.colIndexOf(this.focus) + 1 || this.cols.length;
    const next = [...this.cols];
    next.splice(at, 0, { id: uid("c"), w: this.width(vw), wins: [w] });
    this.cols = next; this.focus = w.id; this.max = null; this.save();
    return w;
  }
  /** A new window below the focused one, in the same column; it gets focus. */
  addBelow(kind: string, item: string, vw: number): Win {
    const i = this.colIndexOf(this.focus);
    if (i < 0) return this.addRight(kind, item, vw);
    const w = this.make(kind, item);
    this.cols = this.cols.map((c, k) => (k === i ? { ...c, wins: [...c.wins.slice(0, c.wins.findIndex((x) => x.id === this.focus) + 1), w, ...c.wins.slice(c.wins.findIndex((x) => x.id === this.focus) + 1)] } : c));
    this.focus = w.id; this.max = null; this.save();
    return w;
  }
  /** Closes a window; an empty column goes with it; focus moves to a neighbour. */
  close(id: string): void {
    const i = this.colIndexOf(id);
    if (i < 0) return;
    const next = this.cols.map((c) => ({ ...c, wins: c.wins.filter((w) => w.id !== id) })).filter((c) => c.wins.length);
    this.cols = next;
    if (this.max === id) this.max = null;
    if (this.focus === id) this.focus = next[Math.min(i, next.length - 1)]?.wins[0]?.id ?? null;
    this.save();
  }
  setFocus(id: string): void { if (this.focus !== id) { this.focus = id; this.save(); } }
  toggleMax(id: string): void { this.max = this.max === id ? null : id; this.focus = id; this.save(); }
  toggleLock(id: string): void { this.patch(id, (w) => ({ ...w, locked: !w.locked })); }
  toggleFrost(id: string): void { this.patch(id, (w) => ({ ...w, frosted: !w.frosted })); }
  private patch(id: string, f: (w: Win) => Win): void { this.cols = this.cols.map((c) => ({ ...c, wins: c.wins.map((w) => (w.id === id ? f(w) : w)) })); this.save(); }
  /** Sets a column's width (never below `MIN_W`). */
  setWidth(colId: string, w: number): void { this.cols = this.cols.map((c) => (c.id === colId ? { ...c, w: Math.max(MIN_W, Math.round(w)) } : c)); }
  /** Moves the boundary under window `i` of a column by `dy` px. */
  dragBoundary(colId: string, i: number, dy: number, h: number): void {
    this.cols = this.cols.map((c) => { if (c.id !== colId) return c; const ws = dragBoundary(c.wins, i, dy, h); return { ...c, wins: c.wins.map((w, k) => ({ ...w, weight: ws[k]! })) }; });
  }
}

/** The one canvas of the page. */
export const canvas = new CanvasModel();

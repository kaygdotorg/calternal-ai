/**
 * Drag and drop for a sidebar, one controller for rows, the pinned strip and sections, by pointer (mouse, pen) and touch (a short hold starts a drag so a swipe still scrolls). It decides nothing about what a drop means:
 * it reads the data attributes of what is under the pointer (`data-row-id` + `data-in`, `data-strip`, `data-sec-head`, `data-sec`) and asks its owner whether the drop is allowed and to perform it. Motion: a ghost follows the
 * pointer on `transform` only; the owner draws the insertion line and the targets that open; `flip` moves what a drop rearranged from where it was to where it is (no jump). Every drag has a keyboard and menu equivalent in the
 * owner (Move up, Move down, Pin, Unpin, Hide), so nothing is only reachable by dragging. Durations and curves come from the motion tokens (1 ms under reduced motion).
 */
import { tick } from "svelte";
import { tokenEase, tokenMs } from "@calternal/ui";

/** What is being dragged. */
export type Source = { kind: "row"; id: string; section: string } | { kind: "section"; id: string; parent: string };
/** What the pointer is over. */
export type Over =
  | { type: "row"; in: string; id: string; after: boolean; rect: DOMRect }
  | { type: "strip"; section: string; rect: DOMRect }
  | { type: "head"; section: string; after: boolean; rect: DOMRect }
  | { type: "section"; section: string; rect: DOMRect };

/** What the owner decides. */
export interface Hooks {
  /** Whether dropping the source here is allowed (the target then shows as one). */
  accepts(s: Source, o: Over): boolean;
  /** Performs the drop. */
  drop(s: Source, o: Over): void | Promise<void>;
  /** The pointer rested on a collapsed section while dragging a row: open it. */
  open?(section: string): void;
}

const HOLD_MS = 260, MOVE_PX = 5;

/** The drag-and-drop controller of one sidebar tree: it listens on the tree root and asks its hooks what a drop may do (see the file comment). */
export class Dnd {
  /** The drag in progress: what, where the pointer is, and what it is over (null when over nothing). */
  active = $state<{ source: Source; x: number; y: number; over: Over | null; ok: boolean } | null>(null);
  #root: HTMLElement; #hooks: Hooks;
  #down: { id: number; x: number; y: number; source: Source; el: HTMLElement; touch: boolean; t: number } | null = null;
  #ghost: HTMLElement | null = null; #raf = 0; #scroll = 0; #hover: { sec: string; t: number } | null = null; #swallow = false;
  constructor(root: HTMLElement, hooks: Hooks) {
    this.#root = root; this.#hooks = hooks;
    root.addEventListener("pointerdown", this.#pd); root.addEventListener("click", this.#click, true);
    root.addEventListener("touchmove", this.#tm, { passive: false });
    addEventListener("pointermove", this.#pm); addEventListener("pointerup", this.#pu); addEventListener("pointercancel", this.#pc); addEventListener("keydown", this.#key);
  }
  destroy() {
    this.#root.removeEventListener("pointerdown", this.#pd); this.#root.removeEventListener("click", this.#click, true); this.#root.removeEventListener("touchmove", this.#tm);
    removeEventListener("pointermove", this.#pm); removeEventListener("pointerup", this.#pu); removeEventListener("pointercancel", this.#pc); removeEventListener("keydown", this.#key);
    this.#end(); cancelAnimationFrame(this.#scroll);
  }

  #sourceOf(t: EventTarget | null): { source: Source; el: HTMLElement } | null {
    const el = (t as HTMLElement | null)?.closest?.<HTMLElement>("[data-drag]"); if (!el) return null;
    if (el.dataset.drag === "row") return { el, source: { kind: "row", id: el.dataset.rowId!, section: el.dataset.in! } };
    return { el, source: { kind: "section", id: el.dataset.secId!, parent: el.dataset.parent ?? "" } };
  }
  #pd = (e: PointerEvent) => {
    if (e.button !== 0 || !e.isPrimary) return;
    const t = e.target as HTMLElement; if (t.closest("input, textarea, [data-nodrag]")) return;
    const s = this.#sourceOf(t); if (!s) return;
    this.#down = { id: e.pointerId, x: e.clientX, y: e.clientY, ...s, touch: e.pointerType === "touch", t: e.timeStamp };
    if (e.pointerType === "touch") { const d = this.#down; setTimeout(() => { if (this.#down === d && !this.active) this.#begin(e.clientX, e.clientY); }, HOLD_MS); }
  };
  #begin(x: number, y: number) {
    const d = this.#down; if (!d) return;
    this.active = { source: d.source, x, y, over: null, ok: false };
    const r = d.el.getBoundingClientRect(), g = d.el.cloneNode(true) as HTMLElement;
    g.removeAttribute("id"); g.setAttribute("aria-hidden", "true"); g.setAttribute("data-ghost", "");
    Object.assign(g.style, { position: "fixed", left: "0", top: "0", width: `${r.width}px`, margin: "0", pointerEvents: "none", zIndex: "1000", opacity: "0.96", boxShadow: "var(--shadow-float)", borderRadius: "var(--r-select)", background: "rgb(var(--tint) / 0.9)", transition: "none" });
    g.style.transform = `translate(${r.left}px, ${r.top}px) scale(1.02)`; (g as HTMLElement & { _o: [number, number] })._o = [x - r.left, y - r.top];
    document.body.appendChild(g); this.#ghost = g; d.el.setAttribute("data-dragging", ""); this.#root.setAttribute("data-dnd", d.source.kind);
    g.animate([{ transform: `translate(${r.left}px, ${r.top}px) scale(1)` }, { transform: `translate(${r.left}px, ${r.top}px) scale(1.02)` }], { duration: tokenMs("--dur-fast", 120), easing: tokenEase("--ease-out", "ease-out") });
    this.#frame(x, y);
  }
  #pm = (e: PointerEvent) => {
    const d = this.#down; if (!d || e.pointerId !== d.id) return;
    if (!this.active) {
      const moved = Math.hypot(e.clientX - d.x, e.clientY - d.y);
      if (d.touch) { if (moved > 8) this.#down = null; return; } // a swipe: the list scrolls
      if (moved >= MOVE_PX) this.#begin(e.clientX, e.clientY); else return;
    }
    e.preventDefault(); this.#frame(e.clientX, e.clientY);
  };
  #tm = (e: TouchEvent) => { if (this.active) e.preventDefault(); };
  #frame(x: number, y: number) {
    cancelAnimationFrame(this.#raf);
    this.#raf = requestAnimationFrame(() => {
      const a = this.active; if (!a) return;
      const g = this.#ghost as (HTMLElement & { _o?: [number, number] }) | null;
      if (g) g.style.transform = `translate(${x - (g._o?.[0] ?? 0)}px, ${y - (g._o?.[1] ?? 0)}px) scale(1.02)`;
      const over = this.#overAt(x, y); a.x = x; a.y = y; a.over = over; a.ok = !!over && this.#hooks.accepts(a.source, over);
      this.#track(over); this.#autoScroll(y);
    });
  }
  #overAt(x: number, y: number): Over | null {
    const t = document.elementFromPoint(x, y) as HTMLElement | null; if (!t) return null;
    const row = t.closest<HTMLElement>("[data-row-id][data-in]");
    if (row && !row.hasAttribute("data-ghost")) {
      const r = row.getBoundingClientRect(), strip = !!row.closest("[data-strip]");
      return { type: "row", in: row.dataset.in!, id: row.dataset.rowId!, after: strip ? x > r.left + r.width / 2 : y > r.top + r.height / 2, rect: r };
    }
    const st = t.closest<HTMLElement>("[data-strip]"); if (st) return { type: "strip", section: st.dataset.strip!, rect: st.getBoundingClientRect() };
    const h = t.closest<HTMLElement>("[data-sec-head]"); if (h) { const r = h.getBoundingClientRect(); return { type: "head", section: h.dataset.secHead!, after: y > r.top + r.height / 2, rect: r }; }
    const s = t.closest<HTMLElement>("[data-sec]"); if (s) return { type: "section", section: s.dataset.sec!, rect: s.getBoundingClientRect() };
    return null;
  }
  /** A collapsed section held over for half a second opens (a row being dragged to a folder inside it). */
  #track(o: Over | null) {
    const sec = o && (o.type === "head" || o.type === "section") ? o.section : null;
    if (!sec || this.active?.source.kind !== "row") { this.#hover = null; return; }
    if (this.#hover?.sec !== sec) { this.#hover = { sec, t: performance.now() }; return; }
    if (performance.now() - this.#hover.t > 500) { this.#hooks.open?.(sec); this.#hover = null; }
  }
  #autoScroll(y: number) {
    cancelAnimationFrame(this.#scroll);
    const sc = this.#root.closest<HTMLElement>(".scroll") ?? this.#root; const r = sc.getBoundingClientRect();
    const v = y < r.top + 36 ? -((r.top + 36 - y) / 6) : y > r.bottom - 36 ? (y - (r.bottom - 36)) / 6 : 0;
    if (!v) return;
    const step = () => { if (!this.active) return; sc.scrollTop += Math.max(-14, Math.min(14, v)); this.#frame(this.active.x, this.active.y); this.#scroll = requestAnimationFrame(step); };
    this.#scroll = requestAnimationFrame(step);
  }
  #pu = async (e: PointerEvent) => {
    const d = this.#down; if (!d || e.pointerId !== d.id) return;
    const a = this.active; this.#down = null;
    if (!a) return;
    this.#swallow = true; setTimeout(() => { this.#swallow = false; }, 0);
    cancelAnimationFrame(this.#raf);
    const px = d.touch ? a.x : e.clientX, py = d.touch ? a.y : e.clientY; // (a lifted touch carries no position of its own: the last move stands)
    const over = this.#overAt(px, py), ok = !!over && this.#hooks.accepts(a.source, over);
    if (ok) { this.#land(d.el); await this.#hooks.drop(a.source, over!); } else this.#back(d.el);
    this.#end();
  };
  #pc = (e: PointerEvent) => { const d = this.#down; if (d && e.pointerId === d.id) { if (this.active) this.#back(d.el); this.#down = null; this.#end(); } };
  #key = (e: KeyboardEvent) => { if (e.key === "Escape" && this.active) { const d = this.#down; if (d) this.#back(d.el); this.#down = null; this.#end(); } };
  #click = (e: MouseEvent) => { if (this.#swallow) { e.stopPropagation(); e.preventDefault(); } };
  /** A refused or cancelled drop: the ghost returns to where it started. */
  #back(el: HTMLElement) { const g = this.#ghost; if (!g) return; const r = el.getBoundingClientRect(); this.#ghost = null; const a = g.animate([{ transform: g.style.transform }, { transform: `translate(${r.left}px, ${r.top}px) scale(1)` }], { duration: tokenMs("--dur-base", 220), easing: tokenEase("--ease-out", "ease-out") }); a.onfinish = () => g.remove(); }
  /** An accepted drop: the ghost eases away into the place (the owner moves the real row; the rest FLIPs). */
  #land(_el: HTMLElement) { const g = this.#ghost; if (!g) return; this.#ghost = null; const a = g.animate([{ opacity: 0.96 }, { opacity: 0 }], { duration: tokenMs("--dur-fast", 120), easing: tokenEase("--ease-out", "ease-out") }); a.onfinish = () => g.remove(); }
  #end() {
    cancelAnimationFrame(this.#raf); cancelAnimationFrame(this.#scroll); this.#hover = null;
    this.#ghost?.remove(); this.#ghost = null; this.#root.removeAttribute("data-dnd");
    this.#root.querySelectorAll("[data-dragging]").forEach((n) => n.removeAttribute("data-dragging"));
    this.active = null;
  }
}

/**
 * Runs `change` (which rearranges what is drawn) and then moves every element carrying `data-flip` from where it was to where it now is, so a drop or a move shows no jump: first, last, invert, play (transform only, over
 * `--dur-base` on `--ease-out`, interruptible by the next change, none under reduced motion).
 */
export async function flip(root: HTMLElement, change: () => void) {
  const before = new Map<string, DOMRect>();
  for (const n of root.querySelectorAll<HTMLElement>("[data-flip]")) before.set(n.dataset.flip!, n.getBoundingClientRect());
  change(); await tick();
  const ms = tokenMs("--dur-base", 220), ease = tokenEase("--ease-out", "ease-out");
  if (ms <= 1) return;
  for (const n of root.querySelectorAll<HTMLElement>("[data-flip]")) {
    const was = before.get(n.dataset.flip!), now = n.getBoundingClientRect(); if (!was) continue;
    const dx = was.left - now.left, dy = was.top - now.top; if (Math.abs(dx) < 1 && Math.abs(dy) < 1) continue;
    n.getAnimations().forEach((a) => { if ((a as Animation & { id?: string }).id === "flip") a.cancel(); });
    const a = n.animate([{ transform: `translate(${dx}px, ${dy}px)` }, { transform: "translate(0, 0)" }], { duration: ms, easing: ease }); a.id = "flip";
  }
}

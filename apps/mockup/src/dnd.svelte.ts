/**
 * Drag and drop in the sidebars. One drag at a time, of one of three things: a COLLECTION from the card's Collections tab (it becomes a section where it is dropped), a SECTION by its grip or title (it moves, or is removed when released outside the
 * sidebar), or a ROW of a section (it moves to another place in its section or into another section; dropped on a pinned strip it is pinned, a copy; dragged off a pinned strip it is unpinned). The ghost follows the pointer; over the sidebar body a line
 * shows exactly where it would land. Near the body's top or bottom edge the body scrolls by itself (a speed that grows towards the edge, one step per frame) and the landing place is recomputed as it scrolls.
 * Mouse: a drag of 5 px or more starts it (a click stays a click, and a mouse drag elsewhere still selects text). Touch: a grip starts at once; a row starts after a half-second hold without moving (a quicker swipe scrolls), and from then the page does not scroll.
 * Listeners are on the window while a drag is pending or live, because the element under the finger moves in the list and a browser drops capture from a moved element. Escape cancels.
 */
import { arr } from "./arrange.svelte";

export interface DragState {
  /** What is dragged: a collection (id), a section (its collection id) or a row (`row` is the item, `cid` the collection it came from). */
  kind: "coll" | "section" | "row"; cid: string; row?: string; sid: string | null; x: number; y: number;
  /** Over the sidebar body. */ over: boolean;
  /** Where it would land: a section index (coll, section) or a row index in `target` (row). */ index: number; target: string | null;
  /** The landing line, in the body's own coordinates (it scrolls with the content). */ line: { left: number; top: number; width: number; height: number } | null;
}
const EDGE = 56, MAX_SPEED = 16, HOLD = 450;

class Dnd {
  drag = $state<DragState | null>(null);
  /** A collection or row just dropped, so the sidebar can flash it. */
  flash = $state<string | null>(null);
  /** True for a tick after a drag ends, so the click that follows a mouse drag is ignored. */
  quiet = false;
  /** The sidebar body that accepts a drop (null when the sidebar is not editable) and the sidebar it shows. */
  zone: { el: HTMLElement; sid: string } | null = null;
  private pending: { kind: DragState["kind"]; cid: string; row?: string; sid: string | null; x: number; y: number; pid: number; now: boolean; copy: boolean } | null = null;
  private raf = 0; private timer = 0;

  /** Called from a pointer down. `now` starts at once (a grip); `hold` lets a touch start after a hold (a row). */
  begin(e: PointerEvent, kind: DragState["kind"], cid: string, sid: string | null, o: { now?: boolean; hold?: boolean; row?: string } = {}) {
    if (e.button !== 0 || this.pending || this.drag) return;
    const mouse = e.pointerType === "mouse";
    if (!mouse && !o.now && !o.hold) return;
    this.pending = { kind, cid, row: o.row, sid, x: e.clientX, y: e.clientY, pid: e.pointerId, now: !!o.now, copy: e.altKey };
    addEventListener("pointermove", this.move); addEventListener("pointerup", this.up); addEventListener("pointercancel", this.cancel); addEventListener("keydown", this.key);
    if (o.now) this.activate(e);
    else if (!mouse) this.timer = window.setTimeout(() => { if (this.pending && !this.drag) this.activate({ clientX: this.pending.x, clientY: this.pending.y } as PointerEvent); }, HOLD);
  }
  private activate(e: { clientX: number; clientY: number }) {
    const p = this.pending!; clearTimeout(this.timer);
    this.drag = { kind: p.kind, cid: p.cid, row: p.row, sid: p.sid, x: e.clientX, y: e.clientY, over: false, index: 0, target: null, line: null }; this.locate(); this.raf = requestAnimationFrame(this.tick);
    document.documentElement.style.userSelect = "none"; addEventListener("touchmove", this.noScroll, { passive: false }); navigator.vibrate?.(8);
  }
  private noScroll = (e: TouchEvent) => { if (this.drag) e.preventDefault(); };
  private move = (e: PointerEvent) => {
    const p = this.pending; if (!p || e.pointerId !== p.pid) return;
    if (!this.drag) { if (Math.hypot(e.clientX - p.x, e.clientY - p.y) < (e.pointerType === "mouse" ? 5 : 8)) return; if (e.pointerType !== "mouse") { this.end(); return; } this.activate(e); }
    this.drag!.x = e.clientX; this.drag!.y = e.clientY; p.copy = e.altKey; this.locate();
  };
  /** Whether the pointer is over the body and where in it the dragged thing would land. */
  private locate() {
    const d = this.drag, z = this.zone; if (!d) return;
    if (!z) { d.over = false; d.line = null; return; }
    const r = z.el.getBoundingClientRect(), st = z.el.scrollTop;
    d.over = d.x >= r.left && d.x <= r.right && d.y >= r.top && d.y <= r.bottom; d.target = null; d.line = null;
    const secs = [...z.el.querySelectorAll<HTMLElement>("[data-sec]")];
    if (d.kind !== "row") {
      let i = secs.findIndex((s) => { const b = s.getBoundingClientRect(); return d.y < b.top + b.height / 2; }); if (i < 0) i = secs.length; d.index = i;
      const at = i < secs.length ? secs[i]!.getBoundingClientRect().top - 2 : secs.length ? secs[secs.length - 1]!.getBoundingClientRect().bottom + 2 : r.top + 6;
      d.line = { left: 8, top: at - r.top + st, width: r.width - 16, height: 3 }; return;
    }
    // a row: the section under the pointer (else the nearest), then the row boundary within it
    let sec = secs.find((s) => { const b = s.getBoundingClientRect(); return d.y >= b.top && d.y <= b.bottom; }) ?? secs.find((s) => d.y < s.getBoundingClientRect().top) ?? secs[secs.length - 1];
    if (!sec) return;
    d.target = sec.dataset.cid!;
    const strip = sec.querySelector<HTMLElement>(".strip"), rows = [...sec.querySelectorAll<HTMLElement>("[data-row]")].filter((x) => x.dataset.row !== d.row);
    if (strip) {
      let i = rows.findIndex((x) => { const b = x.getBoundingClientRect(); return d.x < b.left + b.width / 2; }); if (i < 0) i = rows.length; d.index = i;
      const sb = strip.getBoundingClientRect(), ref = rows[i] ?? rows[rows.length - 1]; const at = ref ? (i < rows.length ? ref.getBoundingClientRect().left - 2 : ref.getBoundingClientRect().right + 2) : sb.left + 4;
      d.line = { left: at - r.left, top: sb.top - r.top + st + 2, width: 3, height: sb.height - 4 };
    } else {
      let i = rows.findIndex((x) => { const b = x.getBoundingClientRect(); return d.y < b.top + b.height / 2; }); if (i < 0) i = rows.length; d.index = i;
      const sb = sec.getBoundingClientRect(); const at = rows.length ? (i < rows.length ? rows[i]!.getBoundingClientRect().top - 1 : rows[rows.length - 1]!.getBoundingClientRect().bottom + 1) : sb.top + 34;
      d.line = { left: 12, top: at - r.top + st, width: r.width - 24, height: 3 };
    }
  }
  private tick = () => {
    const d = this.drag, z = this.zone; if (!d) return;
    if (z && d.over) {
      const r = z.el.getBoundingClientRect(), top = d.y - r.top, bottom = r.bottom - d.y;
      if (top < EDGE && z.el.scrollTop > 0) z.el.scrollTop -= ((EDGE - top) / EDGE) * MAX_SPEED;
      else if (bottom < EDGE) z.el.scrollTop += ((EDGE - bottom) / EDGE) * MAX_SPEED;
      this.locate();
    }
    this.raf = requestAnimationFrame(this.tick);
  };
  private up = (e: PointerEvent) => {
    const p = this.pending; if (!p || e.pointerId !== p.pid) return;
    const d = this.drag, copy = p.copy; this.end();
    if (!d) return;
    this.quiet = true; setTimeout(() => (this.quiet = false), 0);
    const z = this.zone;
    if (d.kind === "row") {
      const from = arr.coll(d.cid);
      if (d.over && z && d.target) { const to = arr.coll(d.target); arr.moveItem(d.row!, d.cid, d.target, d.index, copy || (!!to?.pin && d.cid !== d.target)); this.flash = d.row!; }
      else if (from?.pin && d.sid) arr.dropItem(d.row!, d.cid);
    } else if (d.over && z) { arr.insertSection(z.sid, d.index, d.cid); this.flash = d.cid; }
    else if (d.kind === "section" && d.sid) arr.removeSection(d.sid, d.cid);
    const f = this.flash; if (f) setTimeout(() => { if (this.flash === f) this.flash = null; }, 900);
  };
  private cancel = () => this.end();
  private key = (e: KeyboardEvent) => { if (e.key === "Escape") this.end(); };
  private end() {
    removeEventListener("pointermove", this.move); removeEventListener("pointerup", this.up); removeEventListener("pointercancel", this.cancel); removeEventListener("keydown", this.key); removeEventListener("touchmove", this.noScroll);
    cancelAnimationFrame(this.raf); clearTimeout(this.timer); this.pending = null; this.drag = null; document.documentElement.style.userSelect = "";
  }
}
export const dnd = new Dnd();

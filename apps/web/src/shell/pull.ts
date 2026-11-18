/**
 * The rules of the sidebar pull: a horizontal swipe over the middle card brings a closed sidebar into view, following the finger one to one, and the opposite swipe closes it. One set of rules for a phone
 * (the card slides aside), a touch screen of any width and a trackpad (a horizontal two-finger swipe, `wheel` events). Pure: no DOM state is kept here; the shell moves the pieces.
 *
 * What a swipe never takes (docs/ui-shell.md, "Sidebar pull"):
 *  - a swipe that starts on something that scrolls sideways and can still scroll that way (a code block, a table, a text field wider than its box): that element keeps it, for the whole swipe;
 *  - a swipe that starts on a row's own swipe actions (`.ra`), a dialog or sheet, or anything marked `data-no-pull`;
 *  - a touch while text is selected (the handles drag the selection, not the card);
 *  - a mostly vertical movement (it must be 1.5 times more across than down, and 10 px across, before it counts);
 *  - a mouse (a mouse drag selects text); only touch and the trackpad's wheel pull.
 * A trackpad swipe that the page consumes also stops the browser taking it as Back (the wheel event's default is cancelled); one it does not consume (vertical, or inside a sideways scroller) is left alone.
 */
export type Side = "left" | "right";

/** What a swipe would do: bring `side` into view (`opening`) or put it away. */
export interface Target { side: Side; opening: boolean }

/** `dir` is +1 when the finger moves right, -1 when it moves left. Swiping right closes an open right sidebar, else opens the left one; swiping left closes an open left one, else opens the right (when the app has one). Null: nothing to do. */
export function pickTarget(dir: 1 | -1, s: { leftOpen: boolean; rightOpen: boolean; hasRight: boolean }): Target | null {
  if (dir > 0) return s.rightOpen ? { side: "right", opening: false } : s.leftOpen ? null : { side: "left", opening: true };
  return s.leftOpen ? { side: "left", opening: false } : s.rightOpen || !s.hasRight ? null : { side: "right", opening: true };
}

/** How many px of the sidebar a finger displacement `dx` (right positive) shows, given where it began: `from` is 0 when opening and the sidebar's width when closing. */
export function visible(t: Target, from: number, dx: number, width: number): number {
  const along = t.side === "left" ? dx : -dx;
  return Math.max(0, Math.min(width, from + along));
}

/** Release: open when the speed along the opening direction (px per ms) is a flick that way, closed when it is a flick the other way, else by whether more than half is showing. */
export function settleOpen(vis: number, width: number, velocityAlong: number, flick = 0.4): boolean {
  return velocityAlong > flick ? true : velocityAlong < -flick ? false : vis > width / 2;
}

/** Whether the pointer went down on something a pull must leave alone (see the rules above). `dir` is the swipe's direction once known, else 0 (only the direction-free rules apply). */
export function leavesAlone(target: Element | null, dir: 0 | 1 | -1, selecting: boolean): boolean {
  if (selecting) return true;
  if (!target) return false;
  if (target.closest(".ra, [role=dialog], [data-no-pull]")) return true;
  return dir !== 0 && scrollsSideways(target, dir);
}

/** The nearest ancestor (up to the middle card) that scrolls sideways and still can in the direction the content would move (`dir` +1: the finger moves right, so the content scrolls toward its start). */
export function scrollsSideways(from: Element, dir: 1 | -1): boolean {
  for (let e: Element | null = from; e && !e.matches(".mid, .shell, body"); e = e.parentElement) {
    if (e.scrollWidth <= e.clientWidth + 1) continue;
    if (!/(auto|scroll)/.test(getComputedStyle(e).overflowX)) continue;
    const max = e.scrollWidth - e.clientWidth;
    if (dir > 0 ? e.scrollLeft > 0 : e.scrollLeft < max - 1) return true;
  }
  return false;
}

/**
 * A trackpad swipe as a stream of `wheel` events: it starts at the first event after a quiet moment and ends when `IDLE` ms pass with none (the inertia after the fingers lift is part of it). Horizontal movement only: an event
 * more vertical than across (by the 1.5 rule) is not part of it. `dx` is the finger's travel, right positive (a wheel's `deltaX` is the opposite sign: content moving right is a negative delta).
 */
export class WheelSwipe {
  static readonly IDLE = 120;
  /** The finger's travel since the swipe began (px). */
  dx = 0;
  /** True from the first accepted event until `end`. */
  active = false;
  /** True when the swipe was handed to something else (a sideways scroller, a vertical scroll) and the rest of its events are not ours. */
  ignored = false;
  private samples: { t: number; x: number }[] = [];
  private lastT = -Infinity;
  /** Feeds one event; returns "take" (consume it, cancel its default), "start" (the first one taken), or "leave" (not ours). `scale` turns line deltas into px. */
  feed(e: { deltaX: number; deltaY: number; deltaMode: number; timeStamp: number }, ownsStart: (dir: 1 | -1) => boolean): "start" | "take" | "leave" {
    const k = e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? 400 : 1;
    const dx = -e.deltaX * k, dy = e.deltaY * k;
    const fresh = e.timeStamp - this.lastT > WheelSwipe.IDLE;
    this.lastT = e.timeStamp;
    if (fresh) { this.dx = 0; this.active = false; this.ignored = false; this.samples = []; }
    if (this.ignored) return "leave";
    if (!this.active) {
      if (Math.abs(dx) < 2 || Math.abs(dx) < Math.abs(dy) * 1.5) { if (Math.abs(dy) > Math.abs(dx)) this.ignored = true; return "leave"; }
      if (!ownsStart(dx > 0 ? 1 : -1)) { this.ignored = true; return "leave"; }
      this.active = true;
      this.dx = dx; this.samples = [{ t: e.timeStamp, x: this.dx }];
      return "start";
    }
    this.dx += dx;
    this.samples.push({ t: e.timeStamp, x: this.dx });
    if (this.samples.length > 8) this.samples.shift();
    return "take";
  }
  /** The finger's speed over the last 80 ms, px per ms, right positive. */
  velocity(): number {
    const last = this.samples.at(-1);
    if (!last) return 0;
    const first = this.samples.find((p) => last.t - p.t <= 80) ?? last;
    return last.t > first.t ? (last.x - first.x) / (last.t - first.t) : 0;
  }
  /** Ends the swipe. */
  end(): void { this.active = false; this.samples = []; }
}

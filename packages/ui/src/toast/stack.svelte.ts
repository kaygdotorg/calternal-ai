// Ported from the existing app (origin/dev packages/ui/src/primitives/toast), logic and motion unchanged except where noted.
// The toast stack, for screens with no bottom bar (#145): the list of toasts on screen and each
// one's clock. A toast's time stops while it is held (pointer on it, keyboard focus inside it, a
// finger resting on it), so you never miss an Undo. A hold belongs to one toast: a toast that
// changes in place (Undo became "Undone") or goes away starts again unheld.
// notify.svelte.ts decides what a toast says; Toasts.svelte draws this list.
import type { Presenter } from "./presenter";

type Clock = { left: number; since: number; timer?: ReturnType<typeof setTimeout> };

/** How many toasts are drawn at once; older ones stay alive (and keep time) out of sight. */
const VISIBLE = 3;

/** Toast ids on screen, oldest first (the newest sits lowest). */
export const ids = $state<string[]>([]);

/** The toasts that are drawn: the newest few. */
export const visible = () => ids.slice(-VISIBLE);

const clocks = new Map<string, Clock>();
const holds = new Map<string, Set<string>>(); // toast id -> why it is held ("mouse", "focus", "touch")
let onEnd: (id: string) => void = () => {};

function start(id: string, c: Clock) {
  clearTimeout(c.timer);
  c.timer = undefined;
  if (holds.get(id)?.size || !Number.isFinite(c.left)) return; // a wait stays until it settles
  c.since = performance.now();
  c.timer = setTimeout(() => stackPresenter.dismiss(id), Math.max(0, c.left));
}

/** The default toast presenter: a stack with a pausing clock per toast (it stops while hovered or focused and resumes with what was left). Costs one timer per visible toast. */
export const stackPresenter: Presenter = {
  show(id, duration) {
    let c = clocks.get(id);
    if (!c) {
      clocks.set(id, (c = { left: duration, since: 0 }));
      ids.push(id);
    } else {
      c.left = duration; // changed in place: its time starts again, unheld
      holds.delete(id);
    }
    start(id, c);
  },
  dismiss(id) {
    const c = clocks.get(id);
    if (!c) return;
    clearTimeout(c.timer);
    clocks.delete(id);
    holds.delete(id);
    ids.splice(ids.indexOf(id), 1);
    onEnd(id);
  },
};

/** Called when a toast is gone for good, so notify can forget it. Set once by notify. */
export function onStackEnd(f: (id: string) => void) {
  onEnd = f;
}

/** Holds (or lets go of) a toast's time for one reason. */
export function hold(id: string, why: "mouse" | "focus" | "touch", on: boolean) {
  const c = clocks.get(id);
  if (!c) return;
  const set = holds.get(id) ?? new Set<string>();
  const was = set.size > 0;
  if (on) set.add(why);
  else set.delete(why);
  holds.set(id, set);
  if (!was && set.size) {
    if (c.timer) {
      clearTimeout(c.timer);
      c.timer = undefined;
      c.left -= performance.now() - c.since;
    }
  } else if (was && !set.size) start(id, c);
}

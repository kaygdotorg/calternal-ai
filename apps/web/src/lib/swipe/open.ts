// Ported from the existing app (apps/web/src/rows/swipe-open.ts on origin/dev, #400); one change, marked below.
// One row open at a time (#400). Whichever row is open registers here; a tap elsewhere, a scroll
// or another swipe shuts it. The listeners exist only while a row is open.

type Open = { node: HTMLElement; shut: (animate?: boolean) => void };
let open: Open | null = null;
let unwatch: (() => void) | null = null;

/** Shuts the open row, unless it is `node` itself. */
export function shutOthers(node: HTMLElement) {
  if (open && open.node !== node) open.shut();
}

/** Shuts whichever row is open: on a spring after a tap, at once for keys. */
export function shutOpenRow(animate = true) {
  open?.shut(animate);
}

/** Registers `next` as the one open row and starts watching for the tap or scroll that shuts it; cheap, and the listeners go away in `markShut`. */
export function markOpen(next: Open) {
  open = next;
  unwatch ??= watchOutside();
}

/** Forgets `node` as the open row and drops the outside listeners; does nothing for a row that is not the open one. */
export function markShut(node: HTMLElement) {
  if (open?.node !== node) return;
  open = null;
  unwatch?.();
  unwatch = null;
}

function watchOutside() {
  const away = (e: Event) => {
    if (!open) return done();
    // Only a scroll of something the row sits in shuts it (changed from the existing app, which shut it on any scroll: a streaming chat scrolling elsewhere closed an open row).
    if (e.type === "scroll") return e.target instanceof Node && e.target.contains(open.node) ? open.shut() : undefined;
    if (open.node.contains(e.target as Node)) return;
    open.shut();
    // The tap that shuts a row does only that: it doesn't also open what it landed on.
    const eat = (c: Event) => {
      c.preventDefault();
      c.stopPropagation();
    };
    addEventListener("click", eat, { capture: true, once: true });
    setTimeout(() => removeEventListener("click", eat, true), 500);
  };
  const done = () => {
    removeEventListener("pointerdown", away, true);
    removeEventListener("scroll", away, true);
  };
  addEventListener("pointerdown", away, true);
  addEventListener("scroll", away, { capture: true, passive: true });
  return done;
}

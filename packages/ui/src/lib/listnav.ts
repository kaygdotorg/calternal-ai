/**
 * List navigation, one primitive for every list of rows (docs/native-feel.md): Up and Down move the selection one row (and keep moving at the system's key-repeat rate), Home and End jump to the ends, Page Up and Page Down move a
 * screenful; the row under the cursor paints as selected at once and is kept in view; the thing the row opens (a thread, a note, a page) is opened only when the cursor SETTLES (no key for `settle` ms) or at once on Return, so
 * opening is never what makes arrowing slow. Focus stays in the list: the cursor row takes focus. Modified keys (Alt, Ctrl, Meta, Shift) are left alone (reordering, selecting several), as is typing in a field.
 *
 * Two forms of the same behaviour: `listNav`, a Svelte action for lists whose rows are all in the DOM (a sidebar, Settings' pages), and `stepIndex`/`pageSize`, the pure arithmetic a windowed list (`VirtualList`) uses for rows
 * that are not drawn. Cost per key: one `querySelectorAll` cached until the list's children change, one focus, one scroll write.
 */

/** The keys the primitive handles. */
export const NAV_KEYS = ["ArrowUp", "ArrowDown", "Home", "End", "PageUp", "PageDown"] as const;
/** Whether `key` is one of the navigation keys. */
export const isNavKey = (key: string): boolean => (NAV_KEYS as readonly string[]).includes(key);

/** Rows in a screenful of `viewport` pixels at `rowHeight` each, at least 1 (what Page Up and Page Down move). */
export const pageSize = (viewport: number, rowHeight: number): number => Math.max(1, Math.floor(viewport / Math.max(1, rowHeight)) - 1);

/**
 * Where the cursor goes for `key` from index `i` (-1: none yet) in a list of `count` rows, `page` rows per Page key. The ends stop (no wrap: a list is not a ring); from nowhere Down goes to the first row and Up to the last.
 * Returns -1 for an empty list or a key that is not a navigation key.
 */
export function stepIndex(key: string, i: number, count: number, page: number): number {
  if (count <= 0) return -1;
  const clamp = (n: number) => Math.min(count - 1, Math.max(0, n));
  switch (key) {
    case "ArrowDown": return i < 0 ? 0 : clamp(i + 1);
    case "ArrowUp": return i < 0 ? count - 1 : clamp(i - 1);
    case "PageDown": return i < 0 ? clamp(page - 1) : clamp(i + page);
    case "PageUp": return i < 0 ? count - 1 : clamp(i - page);
    case "Home": return 0;
    case "End": return count - 1;
    default: return -1;
  }
}

/** Whether a key press belongs to the list: a navigation key, no modifier, and not aimed at a text field or an open menu. */
export function wantsKey(e: Pick<KeyboardEvent, "key" | "altKey" | "ctrlKey" | "metaKey" | "shiftKey" | "target" | "defaultPrevented">): boolean {
  if (!isNavKey(e.key) || e.altKey || e.ctrlKey || e.metaKey || e.shiftKey || e.defaultPrevented) return false;
  const t = e.target as HTMLElement | null;
  if (t && (/^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName) || t.isContentEditable || t.closest?.("[role=menu], [role=listbox][aria-expanded=true], [role=tablist], [role=radiogroup], [role=slider], [role=spinbutton], [role=combobox], [role=tree], [role=grid]"))) return false;
  return true;
}

/**
 * While a navigation key repeats, incoming list changes that are not the answer to a window request the cursor needs are HELD (docs/description-contract.md 2.3, docs/native-feel.md section 20): `navHold.held` is true from the first key of a
 * burst until the cursor settles, or for at most `HOLD_MAX_MS` (so a held key shares a frame with a list refresh at most twice a second). The change feeds (`perFrame`, apps/web/src/lib/per-frame.ts) ask it and apply what they held, coalesced
 * to the latest, once it lets go.
 */
export const HOLD_MAX_MS = 500;
export const navHold = (() => {
  let held = false, timer = 0;
  const subs = new Set<() => void>();
  const release = () => { if (timer) { clearTimeout(timer); timer = 0; } if (!held) return; held = false; for (const f of [...subs]) f(); };
  return {
    get held() { return held; },
    /** A key of a navigation burst: hold (once; the maximum runs from the first key). */
    begin() { if (held) return; held = true; timer = window.setTimeout(() => { timer = 0; release(); }, HOLD_MAX_MS); },
    release,
    /** Calls `f` each time a hold ends; returns the way to stop. */
    subscribe(f: () => void) { subs.add(f); return () => { subs.delete(f); }; },
    /** How many are subscribed (the tests check that none are left behind). */
    get subscribers() { return subs.size; },
  };
})();

/** The one place the settle rule lives (both the `listNav` action and `VirtualList`'s `nav` use it): `rest(fn, ms)` runs `fn` after `ms` without another call, and `cancel()` drops it. */
export function settler(): { rest(fn: () => void, ms?: number): void; cancel(): void } {
  let t = 0, raf = 0, painted = true, lastKey = 0, burstGap = 0;
  const clear = () => { if (t) { clearTimeout(t); t = 0; } if (raf) { cancelAnimationFrame(raf); raf = 0; } };
  const cancel = () => { clear(); navHold.release(); };
  // A busy page can run the timer before it has read the keys that came in meanwhile (each of them would have moved the timer): when input is waiting, the rest has not begun, so wait again.
  const pending = (): boolean => { try { return !!(navigator as Navigator & { scheduling?: { isInputPending?: (o?: { includeContinuous?: boolean }) => boolean } }).scheduling?.isInputPending?.({ includeContinuous: true }); } catch { return false; } };
  return {
    cancel,
    rest(fn, ms) {
      clear();
      navHold.begin();
      // The rest must be longer than any gap between two repeats of a held key (whatever the system's rate, and a slow page delivers them unevenly): 1.6 times the longest gap seen in this burst of keys (45 to 120 ms); a first or single key rests the base. A load must never start while keys are still repeating.
      const now = performance.now(), gap = now - lastKey;
      lastKey = now;
      if (gap < 250) burstGap = Math.max(burstGap, gap); else burstGap = 0;
      ms ??= Math.min(120, Math.max(SETTLE_MS, burstGap * 1.6));
      // The selection's last paint must commit before the pane's work starts: a frame is asked for with each key, and when the rest is over the open runs at once if that frame has been drawn (it has, unless the page is busy), else after it. The open is a task of its own. A key in between cancels all of it.
      painted = false;
      requestAnimationFrame(() => { painted = true; });
      const arm = () => { t = window.setTimeout(() => { t = 0; if (pending()) { arm(); return; } navHold.release(); if (painted) fn(); else raf = requestAnimationFrame(() => { raf = 0; t = window.setTimeout(() => { t = 0; fn(); }, 0); }); }, ms); };
      arm();
    },
  };
}
/** The least milliseconds without a key before the cursor's row is opened (more while a key is repeating: see `settler`). */
export const SETTLE_MS = 45;

/** Options of the `listNav` action. */
export interface ListNavOptions {
  /** CSS selector of the rows' focusable elements (links or buttons). */
  rows: string;
  /** Milliseconds without a key before the cursor's row is opened (clicked). Default 50 (above the gap between two repeats of a held key, below what a person perceives as waiting). */
  settle?: number;
  /** Called at once when the cursor moves to `row` (the selection has painted). */
  onmove?: (row: HTMLElement) => void;
  /** Opens a row when the cursor settles; default: click it. */
  open?: (row: HTMLElement) => void;
}

/** Marks the cursor row: `data-cursor` is what the row's style reads (the same look as selected). */
function mark(prev: HTMLElement | null, next: HTMLElement): void {
  if (prev && prev !== next) prev.removeAttribute("data-cursor");
  next.setAttribute("data-cursor", "");
}

/** The Svelte action (see the file's comment). */
export function listNav(node: HTMLElement, opts: ListNavOptions) {
  let o = opts, cache: HTMLElement[] | null = null, cursor: HTMLElement | null = null;
  const rest = settler();
  const mo = new MutationObserver(() => { cache = null; });
  mo.observe(node, { childList: true, subtree: true });
  const rows = () => (cache ??= [...node.querySelectorAll<HTMLElement>(o.rows)]);
  const scroller = (): HTMLElement | null => { let e = node.parentElement; while (e && e.scrollHeight <= e.clientHeight + 1) e = e.parentElement; return e; };
  const stop = () => rest.cancel();

  function onKey(e: KeyboardEvent) {
    if (!wantsKey(e) || (e.target as Element | null)?.closest?.("[data-listnav=own]")) return; // a windowed list (VirtualList) moves its own cursor through rows that are not drawn; this handler runs first (Svelte delegates its handlers to the root), so it must leave those keys alone
    const list = rows();
    if (!list.length) return;
    const here = document.activeElement as HTMLElement | null;
    let i = here ? list.findIndex((r) => r === here || r.contains(here)) : -1;
    if (i < 0) i = list.findIndex((r) => r.hasAttribute("aria-current") || r.hasAttribute("data-selected"));
    const sc = scroller(), rowH = list[Math.max(0, i)]!.offsetHeight || 40;
    const next = stepIndex(e.key, i, list.length, pageSize(sc?.clientHeight ?? 400, rowH));
    if (next < 0) return;
    e.preventDefault();
    const row = list[next]!;
    mark(cursor, row); cursor = row;
    row.focus({ preventScroll: true });
    row.scrollIntoView({ block: "nearest" });
    o.onmove?.(row);
    stop();
    rest.rest(() => (o.open ?? ((r: HTMLElement) => r.click()))(row), o.settle);
  }
  const onOut = (e: FocusEvent) => { if (!node.contains(e.relatedTarget as Node | null)) { stop(); cursor?.removeAttribute("data-cursor"); cursor = null; } };
  node.addEventListener("keydown", onKey);
  node.addEventListener("focusout", onOut);
  return {
    update(n: ListNavOptions) { o = n; cache = null; },
    destroy() { stop(); mo.disconnect(); node.removeEventListener("keydown", onKey); node.removeEventListener("focusout", onOut); },
  };
}

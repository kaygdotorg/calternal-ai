<script lang="ts">
  // A windowed list for rows of unknown height: only the rows near the viewport exist in the DOM, however long the list is.
  //
  // Technique (docs/ui-performance.md): a Fenwick tree holds every row's height (an estimate until measured), so the first row to draw
  // and the offset of the window come from two O(log n) lookups. The window is ordinary flow content translated to its offset inside a
  // sizer of the full height. One ResizeObserver measures the drawn rows; a frame's measurements are applied together, the tree is
  // updated, and the scroll position is corrected once so rows above the viewport changing height never move what the reader sees.
  // Scrolling reads `scrollTop` only (no layout read per row) and writes state only when the first or last row changes.
  // A list that was at the bottom stays at the bottom while it grows (streaming) and starts there.
  //
  // Two ways to scroll. By default the list is its own scroller (a conversation). With `inside` it is a block inside the nearest scrollable ancestor (a sidebar, a Settings page, a card) and the
  // window follows that ancestor's scroll; the rows above and below the list keep their normal place. `onrange` reports the rows drawn, so a list read a page at a time can fetch the next
  // page when the window nears the last loaded row. The DOM holds only the window (about the viewport plus `overscan` px each way) however many rows exist.
  import type { Snippet } from "svelte";
  import { untrack } from "svelte";
  import { Fenwick } from "../lib/fenwick";
  import { pageSize, stepIndex, wantsKey } from "../lib/listnav";

  let {
    count,
    estimate = 120,
    overscan = 700,
    label,
    row,
    startAtBottom = true,
    topInset = 0,
    bottomInset = 0,
    onrange,
    inside = false,
    rowRole = inside ? "listitem" : "article",
    nav,
  }: { /** Keyboard navigation of the rows (docs/native-feel.md, list navigation): Up, Down, Home, End, Page Up and Page Down move a cursor through ALL rows (drawn or not), the cursor row paints at once and keeps focus, and `open` (default: click the row's link or button) runs when the cursor settles. */ nav?: { settle?: number; open?: (i: number) => void; selected?: () => number; /** The stable key of row `i` and the row now holding a key (-1: gone), for a list that changes under the cursor (new rows at the top, rows moving): the cursor then follows its row instead of staying at its index. */ id?: (i: number) => string; indexOf?: (key: string) => number }; inside?: boolean; rowRole?: string; topInset?: number; bottomInset?: number; onrange?: (first: number, end: number) => void; count: number; estimate?: number; overscan?: number; label: string; row: Snippet<[number]>; startAtBottom?: boolean } = $props();

  let scroller = $state<HTMLDivElement>();
  let sizer = $state<HTMLDivElement>();
  /** The scrollable ancestor in `inside` mode. */
  let host: HTMLElement | null = null;
  /** What scrolls: the ancestor (`inside`) or the list's own element. */
  const view = (): HTMLElement | null | undefined => (inside ? host : scroller);
  /** Where the list's first row sits in the scrolled content: the insets when it is its own scroller, its measured place in the ancestor otherwise (one layout read per update). */
  const origin = (): number => (inside && host && sizer ? sizer.getBoundingClientRect().top - host.getBoundingClientRect().top + host.scrollTop : topInset);
  let heights = new Float64Array(0);
  let tree = new Fenwick(heights);
  let first = $state(0);
  let last = $state(0);
  let total = $state(0);
  let stuck = true;
  let raf = 0;
  /** When the list last scrolled itself (to the end), and when the person last moved it (wheel, finger, key, press): a scroll the list made, or one that followed content growing under it, must not un-stick it from the end; only the person does. */
  let driven = 0, intent = 0, drivenTop = 0; // when and where the list last scrolled itself to the end, and when the person last wheeled, touched or pressed a key
  const touch = () => { intent = performance.now(); };
  const rows = new Map<number, HTMLElement>();

  /** Rebuilds the tree for a new `count`, keeping every measured height. Called when the count changes. */
  function resize(n: number) {
    const next = new Float64Array(n);
    next.set(heights.subarray(0, Math.min(n, heights.length)));
    for (let i = heights.length; i < n; i++) next[i] = estimate;
    heights = next;
    tree = new Fenwick(heights);
    total = tree.prefix(n);
  }

  function update() {
    const v = view();
    if (!v) return;
    const top = Math.max(0, v.scrollTop - origin());
    const f = tree.find(Math.max(0, top - overscan));
    const l = Math.min(count - 1, tree.find(top + v.clientHeight + overscan));
    if (f !== first || l + 1 !== last) {
      first = f;
      last = l + 1;
      onrange?.(first, last);
    }
  }

  function onscroll() {
    const v = view();
    if (!v) return;
    if (inside) stuck = false;
    else if (v.scrollHeight - v.scrollTop - v.clientHeight < 24) stuck = true;
    // The person's own scroll lets go of the end. A scroll that comes within 100 ms of the list's own is only theirs if it went above where the list put itself (the wheel can arrive while a streaming turn is moving the end).
    else if (performance.now() - intent < 800 && (performance.now() - driven > 100 || v.scrollTop < drivenTop - 4)) stuck = false;
    if (!raf) raf = requestAnimationFrame(() => { raf = 0; update(); });
  }

  // Measuring: one observer for all drawn rows. Each callback batches every row that changed this frame.
  const ro = typeof ResizeObserver === "undefined" ? null : new ResizeObserver((entries) => {
    const v = view();
    if (!v) return;
    const top = v.scrollTop, o = origin();
    let shiftAbove = 0;
    let changed = false;
    for (const e of entries) {
      const i = Number((e.target as HTMLElement).dataset.i);
      const h = e.borderBoxSize?.[0]?.blockSize ?? (e.target as HTMLElement).offsetHeight;
      if (!(h > 0) || i >= heights.length || Math.abs(heights[i]! - h) < 0.5) continue;
      const d = h - heights[i]!;
      heights[i] = h;
      tree.add(i, d);
      changed = true;
      if (tree.prefix(i) + o < top && !stuck) shiftAbove += d;
    }
    if (!changed) return;
    total = tree.prefix(count);
    if (stuck) { queueMicrotask(toBottom); requestAnimationFrame(toBottom); } // twice: the sizer's height is written by the next state flush, and the end moves with it
    else if (shiftAbove) v.scrollTop = top + shiftAbove;
    update();
  });

  function measure(node: HTMLElement) {
    ro?.observe(node);
    const i = Number(node.dataset.i);
    rows.set(i, node);
    return { destroy: () => { ro?.unobserve(node); if (rows.get(i) === node) rows.delete(i); } };
  }

  // ---- Keyboard navigation (`nav`): the cursor is an index, so it can be on a row that is not drawn; the selection paints at once (the row's `data-cursor`), the list scrolls just enough, and what the row opens runs after the cursor settles.
  let cursor = $state(-1), cursorKey: string | null = null, navTimer = 0, navAt = 0;
  const focusCursor = () => { const row = rows.get(cursor); if (row && !row.contains(document.activeElement)) (row.querySelector<HTMLElement>("a, button") ?? row).focus({ preventScroll: true }); };
  // A list that changes under the cursor (churn): the cursor follows its row, and focus returns to it if the row was redrawn.
  $effect(() => {
    if (!nav?.indexOf || cursorKey === null) return;
    const i = nav.indexOf(cursorKey);
    void count;
    untrack(() => {
      // Its row is gone (removed by someone else): the row now at its place takes the cursor, so focus does not fall out of the list.
      if (i < 0) { cursor = Math.min(cursor, count - 1); cursorKey = cursor >= 0 ? nav.id?.(cursor) ?? null : null; } else if (i !== cursor) cursor = i;
      if (cursor >= 0) requestAnimationFrame(focusCursor);
    });
  });
  /** Scrolls the least that puts row `i` fully in view (no animation: a held key must not queue animations). */
  function ensureVisible(i: number) {
    const v = view();
    if (!v) return;
    const top = origin() + tree.prefix(i), bottom = top + (heights[i] ?? estimate);
    if (top < v.scrollTop + topInset) v.scrollTop = Math.max(0, top - topInset);
    else if (bottom > v.scrollTop + v.clientHeight - bottomInset) v.scrollTop = bottom - v.clientHeight + bottomInset;
    update();
  }
  function onNavKey(e: KeyboardEvent) {
    if (!nav || !wantsKey(e) || count === 0) return;
    const at = (e.target as HTMLElement | null)?.closest<HTMLElement>("[data-i]");
    const here = cursor >= 0 ? cursor : at ? Number(at.dataset.i) : nav.selected?.() ?? -1;
    const v = view();
    const next = stepIndex(e.key, here, count, pageSize(v?.clientHeight ?? 400, total / Math.max(1, count)));
    if (next < 0) return;
    e.preventDefault();
    cursor = next;
    cursorKey = nav.id?.(next) ?? null;
    navAt = performance.now();
    ensureVisible(next);
    // The row exists once the window has moved to it; focus its link or button then (one frame), so focus stays in the list.
    requestAnimationFrame(focusCursor);
    clearTimeout(navTimer);
    navTimer = window.setTimeout(() => { navTimer = 0; const at = cursorKey !== null && nav.indexOf ? nav.indexOf(cursorKey) : next; if (at < 0) return; if (nav.open) nav.open(at); else rows.get(at)?.querySelector<HTMLElement>("a, button")?.click(); }, nav.settle ?? 50);
  }
  function onNavOut(e: FocusEvent) { if (!e.relatedTarget && performance.now() - navAt < 300) return; /* the focused row left the window as the cursor moved away: not the person leaving */ if (!(e.currentTarget as HTMLElement).contains(e.relatedTarget as Node | null)) { clearTimeout(navTimer); cursor = -1; cursorKey = null; } }

  function toBottom() {
    if (scroller && stuck) { driven = performance.now(); scroller.scrollTop = scroller.scrollHeight; drivenTop = scroller.scrollTop; }
  }

  /**
   * Puts row `i` in view at once (no animation), without rendering what is between: the offset comes from the height tree (measured heights where there are any, the estimate elsewhere), so a jump over thousands of rows draws only the rows around the target. Once those are
   * mounted and measured, one correction moves the row to exactly where it was asked to be (`align`: the middle of the list, or just under the top inset). Cost: one prefix sum and one scroll write, then one layout read for the correction.
   */
  export function scrollToIndex(i: number, align: "centre" | "start" = "centre") {
    const v = view();
    if (!v || i < 0 || i >= count) return;
    stuck = false;
    const want = (rowH: number) => (align === "centre" ? Math.max(topInset, (v.clientHeight - rowH) / 2) : topInset);
    v.scrollTop = Math.max(0, origin() + tree.prefix(i) - want(heights[i] ?? estimate));
    update();
    let tries = 0;
    const settle = () => {
      const el = rows.get(i);
      if (!el) { if (++tries < 6) requestAnimationFrame(settle); return; }
      const top = el.getBoundingClientRect().top - v.getBoundingClientRect().top;
      const delta = top - want(el.getBoundingClientRect().height);
      if (Math.abs(delta) > 1) v.scrollTop += delta;
    };
    requestAnimationFrame(() => requestAnimationFrame(settle));
  }

  /** Whether the list is within `px` of its end (the person is reading the latest, not an older part). */
  export function nearEnd(px = 200): boolean {
    const v = view();
    return !v || v.scrollHeight - v.scrollTop - v.clientHeight < px;
  }

  /** Scrolls to the end and keeps following it while the list grows. */
  export function scrollToBottom() {
    stuck = true;
    toBottom();
    requestAnimationFrame(toBottom);
  }

  $effect(() => {
    // One rebuild per count change (the only dependency); at first draw also place the window at the end.
    const n = count;
    untrack(() => {
      const wasEmpty = heights.length === 0;
      resize(n);
      if (wasEmpty) stuck = !inside && startAtBottom;
      if (wasEmpty && startAtBottom && scroller) { driven = performance.now(); scroller.scrollTop = total + topInset + bottomInset; drivenTop = scroller.scrollTop; }
      update();
      onrange?.(first, last); // the count changed: a list read a page at a time asks again whether the window is near its end
    });
  });
  $effect(() => () => { cancelAnimationFrame(raf); ro?.disconnect(); });
  // `inside`: find the scrollable ancestor once and follow its scroll and size.
  $effect(() => {
    if (!inside || !sizer) return;
    let e: HTMLElement | null = sizer.parentElement;
    while (e && !/(auto|scroll)/.test(getComputedStyle(e).overflowY)) e = e.parentElement;
    host = e;
    if (!host) return;
    host.addEventListener("scroll", onscroll, { passive: true });
    const hostRo = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(() => update());
    hostRo?.observe(host);
    update();
    return () => { host?.removeEventListener("scroll", onscroll); hostRo?.disconnect(); host = null; };
  });

  const indices = $derived(Array.from({ length: Math.max(0, last - first) }, (_, k) => first + k));
  // The window sits at the sum of the heights above it; `total` changes whenever the tree does, so it is the dependency.
  const offset = $derived.by(() => { void total; return tree.prefix(first); });
</script>

{#snippet windowed()}
  <div class="window" style:transform="translateY({offset + (inside ? 0 : topInset)}px)">
    {#each indices as i (i)}
      <div class="row" data-i={i} data-cursor={cursor === i ? "" : undefined} role={rowRole} aria-posinset={i + 1} aria-setsize={count} use:measure>{@render row(i)}</div>
    {/each}
  </div>
{/snippet}
{#if inside}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div bind:this={sizer} class="sizer" data-listnav={nav ? "own" : undefined} role="list" aria-label={label} style:height="{total}px" onkeydown={onNavKey} onfocusout={onNavOut}>{@render windowed()}</div>
{:else}
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div bind:this={scroller} class="scroller edge-fade" data-listnav={nav ? "own" : undefined} style:--fade-t="{topInset}px" style:--fade-b="{bottomInset}px" role="feed" aria-label={label} aria-busy="false" tabindex="0" onscroll={onscroll} onwheel={touch} ontouchstart={touch} ontouchmove={touch} onpointerdown={touch} onkeydown={(e) => { touch(); onNavKey(e); }} onfocusout={onNavOut}>
    <div class="sizer" style:height="{total + topInset + bottomInset}px">{@render windowed()}</div>
  </div>
{/if}

<style>
  .scroller { position: absolute; inset: 0; border-radius: var(--r-card); overflow-y: auto; overflow-x: hidden; overscroll-behavior: contain; scrollbar-width: none; contain: strict; }
  .sizer { position: relative; width: 100%; }
  .window { position: absolute; top: 0; left: 0; right: 0; will-change: transform; }
  .row { contain: layout style; }
  /* The cursor row of keyboard navigation looks as selected as the row it wraps (Row's `aria-current`). */
  .row[data-cursor] :global(.row) { background: var(--accent-soft); }
  .row[data-cursor] :global(.row .ic) { color: var(--accent); }
</style>

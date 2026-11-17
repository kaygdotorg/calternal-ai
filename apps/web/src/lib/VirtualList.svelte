<script lang="ts">
  // A windowed list for rows of unknown height: only the rows near the viewport exist in the DOM, however long the list is.
  //
  // Technique (docs/ui-performance.md): a Fenwick tree holds every row's height (an estimate until measured), so the first row to draw
  // and the offset of the window come from two O(log n) lookups. The window is ordinary flow content translated to its offset inside a
  // sizer of the full height. One ResizeObserver measures the drawn rows; a frame's measurements are applied together, the tree is
  // updated, and the scroll position is corrected once so rows above the viewport changing height never move what the reader sees.
  // Scrolling reads `scrollTop` only (no layout read per row) and writes state only when the first or last row changes.
  // A list that was at the bottom stays at the bottom while it grows (streaming) and starts there.
  import type { Snippet } from "svelte";
  import { untrack } from "svelte";
  import { Fenwick } from "./fenwick";

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
  }: { topInset?: number; bottomInset?: number; onrange?: (first: number, end: number) => void; count: number; estimate?: number; overscan?: number; label: string; row: Snippet<[number]>; startAtBottom?: boolean } = $props();

  let scroller = $state<HTMLDivElement>();
  let heights = new Float64Array(0);
  let tree = new Fenwick(heights);
  let first = $state(0);
  let last = $state(0);
  let total = $state(0);
  let stuck = true;
  let raf = 0;
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
    if (!scroller) return;
    const top = Math.max(0, scroller.scrollTop - topInset);
    const f = tree.find(Math.max(0, top - overscan));
    const l = Math.min(count - 1, tree.find(top + scroller.clientHeight + overscan));
    if (f !== first || l + 1 !== last) {
      first = f;
      last = l + 1;
      onrange?.(first, last);
    }
  }

  function onscroll() {
    if (!scroller) return;
    stuck = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < 24;
    if (!raf) raf = requestAnimationFrame(() => { raf = 0; update(); });
  }

  // Measuring: one observer for all drawn rows. Each callback batches every row that changed this frame.
  const ro = typeof ResizeObserver === "undefined" ? null : new ResizeObserver((entries) => {
    if (!scroller) return;
    const top = scroller.scrollTop;
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
      if (tree.prefix(i) + topInset < top && !stuck) shiftAbove += d;
    }
    if (!changed) return;
    total = tree.prefix(count);
    if (stuck) queueMicrotask(toBottom);
    else if (shiftAbove) scroller.scrollTop = top + shiftAbove;
    update();
  });

  function measure(node: HTMLElement) {
    ro?.observe(node);
    return { destroy: () => ro?.unobserve(node) };
  }

  function toBottom() {
    if (scroller) scroller.scrollTop = scroller.scrollHeight;
  }

  /** Scrolls so row `i` is at the top. */
  export function scrollToIndex(i: number) {
    if (scroller) scroller.scrollTop = tree.prefix(i) + topInset;
  }
  /** Scrolls to the end and keeps following it while the list grows. */
  export function scrollToBottom() {
    stuck = true;
    toBottom();
  }

  $effect(() => {
    // One rebuild per count change (the only dependency); at first draw also place the window at the end.
    const n = count;
    untrack(() => {
      const wasEmpty = heights.length === 0;
      resize(n);
      if (wasEmpty) stuck = startAtBottom;
      if (wasEmpty && startAtBottom && scroller) scroller.scrollTop = total + topInset + bottomInset;
      update();
    });
  });
  $effect(() => () => { cancelAnimationFrame(raf); ro?.disconnect(); });

  const indices = $derived(Array.from({ length: Math.max(0, last - first) }, (_, k) => first + k));
  // The window sits at the sum of the heights above it; `total` changes whenever the tree does, so it is the dependency.
  const offset = $derived.by(() => { void total; return tree.prefix(first); });
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div bind:this={scroller} class="scroller edge-fade" style:--fade-t="{topInset}px" style:--fade-b="{bottomInset}px" role="feed" aria-label={label} aria-busy="false" tabindex="0" onscroll={onscroll}>
  <div class="sizer" style:height="{total + topInset + bottomInset}px">
    <div class="window" style:transform="translateY({offset + topInset}px)">
      {#each indices as i (i)}
        <div class="row" data-i={i} role="article" aria-posinset={i + 1} aria-setsize={count} use:measure>{@render row(i)}</div>
      {/each}
    </div>
  </div>
</div>

<style>
  .scroller { position: absolute; inset: 0; border-radius: var(--r-card); overflow-y: auto; overflow-x: hidden; overscroll-behavior: contain; contain: strict; }
  .sizer { position: relative; width: 100%; }
  .window { position: absolute; top: 0; left: 0; right: 0; will-change: transform; }
  .row { contain: layout style; }
</style>

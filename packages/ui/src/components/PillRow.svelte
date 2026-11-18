<script lang="ts" generics="T extends string">
  // A row of pills that measures itself and gives up words by a stated priority: the one place the "icons and text, then icons only as the room shrinks" rule is built (docs/ui-design-system.md, "Adaptive rows").
  // The caller draws each pill with `pill(id, compact, probe)`: icon and text when `compact` is false, icon only when true (its label stays as the tooltip and the accessible name: `Pill` does both). While `probe` is
  // true the pill is only being measured (hidden, inert, never interactive): draw it without bindings or menus. `drop` names the order words are given up in (the first named gives them up first), `keep` the ids that
  // give up theirs last (a pill holding a non-default choice), `floor` the row width (px) under which every other pill is icon only at once (a phone). The room is the row's own width, not the window's: put the row where its container is what narrows (a composer, a capsule).
  // Cost: two hidden copies of each pill, measured by one ResizeObserver per row; the decision is a few additions (lib/fit.ts) and the set changes only when the answer does.
  import type { Snippet } from "svelte";
  import { collapse, type PillWidths } from "../lib/fit";
  let { ids, drop, keep = [], floor = 0, label, pill, class: klass }: { ids: T[]; drop: T[]; keep?: T[]; floor?: number; label?: string; pill: Snippet<[T, boolean, boolean]>; class?: string } = $props();
  let row = $state<HTMLElement>();
  let fullProbe = $state<HTMLElement>();
  let compactProbe = $state<HTMLElement>();
  let compact = $state(new Set<T>());
  const same = (a: Set<T>, b: Set<T>) => a.size === b.size && [...a].every((x) => b.has(x));
  function fit() {
    if (!row || !fullProbe || !compactProbe) return;
    const f = [...fullProbe.children] as HTMLElement[], c = [...compactProbe.children] as HTMLElement[];
    if (f.length !== ids.length) return;
    const widths = {} as Record<T, PillWidths>;
    ids.forEach((id, i) => (widths[id] = { full: f[i]!.offsetWidth, compact: c[i]!.offsetWidth }));
    const gap = parseFloat(getComputedStyle(row).columnGap) || 0;
    const next = collapse(ids, widths, row.clientWidth, gap, drop, keep, floor);
    if (!same(next, compact)) compact = next;
  }
  $effect(() => {
    if (!row || !fullProbe || !compactProbe) return;
    void ids; void drop; void keep; // a change of the set or of the priority is a new decision
    fit();
    if (typeof ResizeObserver === "undefined") return; // no layout engine to measure with: every pill stays in full
    const ro = new ResizeObserver(fit);
    ro.observe(row); ro.observe(fullProbe); ro.observe(compactProbe);
    return () => ro.disconnect();
  });
</script>

<div class={["row", klass]} role="group" aria-label={label} bind:this={row}>
  {#each ids as id (id)}{@render pill(id, compact.has(id), false)}{/each}
  <div class="probe" aria-hidden="true" inert bind:this={fullProbe}>{#each ids as id (id)}{@render pill(id, false, true)}{/each}</div>
  <div class="probe" aria-hidden="true" inert bind:this={compactProbe}>{#each ids as id (id)}{@render pill(id, true, true)}{/each}</div>
</div>

<style>
  .row { position: relative; display: flex; flex-wrap: nowrap; align-items: center; gap: var(--s-2); min-width: 0; }
  /* The measuring copies take no room and are not seen; each sizes to its own content. */
  .probe { position: absolute; left: 0; top: 0; height: 0; overflow: hidden; visibility: hidden; pointer-events: none; display: flex; gap: var(--s-2); width: max-content; }
</style>

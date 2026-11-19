<script lang="ts">
  // A bound list: the windowed rows of one bind drawn with its row template, and its states. It reuses the design system's VirtualList (the only windowed list; one primitive, not forked) and Row, and the store
  // keeps the host's windows. While loading it draws skeleton rows of the real row height; `empty`, `error` and `refine_query` show the bind's own message (the renderer's plain default only for `error` and
  // `refine_query`, which a description may leave unsaid). The list takes the height of its container and scrolls itself, or with `inside` follows the nearest scrolling ancestor (a sidebar).
  import { VirtualList, cssPx } from "@calternal/ui";
  import type { Bind, State } from "../model";
  import type { WindowStore } from "../windows/window-store.svelte";
  import RowView from "./RowView.svelte";
  import SkeletonRow from "./SkeletonRow.svelte";
  import StateMessage from "./StateMessage.svelte";

  let { store, kindIcon, label, inside = false, selectedId, onopen, onsettle }: {
    store: WindowStore; kindIcon?: string; label: string; inside?: boolean;
    /** The id of the row that is selected (read from the list's own selection by the caller). */
    selectedId?: string | null;
    /** A row was activated (click or Return). */
    onopen?: (id: string, index: number) => void;
    /** The cursor rested on row `index` after a key burst (the settled selection, sent once). */
    onsettle?: (id: string, index: number) => void;
  } = $props();

  const bind: Bind = $derived(store.bind);
  const template = $derived(bind.row ?? { title: { field: "title" } });
  const consumer = {};
  $effect(() => { const s = store; s.ensure(consumer, 0, 0); return () => s.ensure(consumer, 0, 0); });

  const states = $derived((bind.states ?? {}) as { empty?: State; loading?: State; error?: State; refine_query?: State });
  const errorMessage: State = $derived(states.error ?? { title: "This list could not be loaded." });
  const refineMessage: State = $derived(states.refine_query ?? { title: "Refine your search." });
  const skeletons = [70, 52, 64, 44, 58];
  /** The system row height (a token), so the scroll extent is right before any row is measured. */
  const rowH = cssPx("--row-h", 44);
</script>

<div class="list" data-bind={bind.id} data-state={store.state} data-count={store.length}>
  {#if store.state === "loading"}
    <div class="sk" role="status" aria-label={states.loading?.title ?? "Loading"}>
      {#each skeletons as w, k (k)}<SkeletonRow width={w} icon={template.icon !== undefined} />{/each}
    </div>
    {#if states.loading}<StateMessage message={states.loading} />{/if}
  {:else if store.state === "empty"}
    {#if states.empty}<StateMessage message={states.empty} />{/if}
  {:else if store.state === "error"}
    <StateMessage message={errorMessage} tone="bad" live="alert" />
  {:else}
    {#if store.state === "refine_query"}<div class="refine"><StateMessage message={refineMessage} /></div>{/if}
    <div class="rows" class:inside>
      <VirtualList
        count={store.length} estimate={rowH} startAtBottom={false} {inside} {label} rowRole="listitem"
        onrange={(first, end) => store.ensure(consumer, first, end)}
        nav={{ id: (i) => store.row(i)?.id ?? "", indexOf: (id) => store.indexOf(id), selected: () => (selectedId ? store.indexOf(selectedId) : -1), open: (i) => { const r = store.row(i); if (r) onsettle?.(r.id, i); } }}
      >
        {#snippet row(i)}
          {@const r = store.row(i)}
          {#if r}<RowView {template} row={r} {kindIcon} selected={r.id === selectedId} onclick={() => onopen?.(r.id, i)} />
          {:else}<SkeletonRow icon={template.icon !== undefined} width={40 + ((i * 17) % 40)} />{/if}
        {/snippet}
      </VirtualList>
    </div>
  {/if}
</div>

<style>
  .list { position: relative; display: flex; flex-direction: column; min-height: 0; flex: 1; }
  .rows { position: relative; flex: 1; min-height: 0; }
  .rows.inside { flex: none; }
  .refine { flex: none; }
</style>

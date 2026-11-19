<script lang="ts">
  // The nested child rows of one parent row (RowTemplate.children): a bind with `source.parent`, asked for with its own window, only while the row is expanded, with the host-issued parent ref from `parent_refs`. Depth is at most 3
  // (the host checks it). Child rows draw with the child bind's own template, indented by depth; they are a short, plain list (a window), so they are not virtualised.
  import type { WindowStore } from "../windows/window-store.svelte";
  import { useApp } from "../invoke/context";
  import RowActions from "./RowActions.svelte";
  import SkeletonRow from "./SkeletonRow.svelte";
  let { store, depth, kindIcon }: { store: WindowStore; depth: number; kindIcon?: string } = $props();
  const app = useApp();
  const consumer = {};
  const template = $derived(store.bind.row ?? { title: { field: "title" } });
  $effect(() => { const s = store; s.ensure(consumer, 0, s.size); return () => s.ensure(consumer, 0, 0); });
  const n = $derived(Math.min(store.length, store.size));
</script>

<div class="kids" role="group" data-children={store.bind.id}>
  {#if store.state === "loading"}<SkeletonRow width={50} />
  {:else}{#each Array.from({ length: n }, (_, i) => i) as i (i)}
    {@const r = store.row(i)}
    {#if r}<RowActions {template} row={r} {store} {kindIcon} {depth} selected={r.id === app?.selectionFor(store.bind.id).drawn} onclick={() => { void app?.selectionFor(store.bind.id).settle(r.id); const o = template.open; if (o && "route" in o) app?.setRoute(o.route.name, r.id); }} />{/if}
  {/each}{/if}
</div>

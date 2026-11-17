<script lang="ts">
  import { href } from "../lib/route.svelte";
  // Search (placeholder): filters the fixture thread titles. The real search is an operation over the space's index.
  import { Row, Sheet } from "@calternal/ui";
  import { store } from "../apps/threads/store.svelte";
  let { open, onclose }: { open: boolean; onclose: () => void } = $props();
  let q = $state("");
  const hits = $derived(store.threads.filter((t) => t.title.toLowerCase().includes(q.trim().toLowerCase())).slice(0, 6));
</script>

<Sheet {open} label="Search" {onclose}>
  <div class="in">
    <label class="visually-hidden" for="search-q">Search threads</label>
    <input id="search-q" type="search" bind:value={q} placeholder="Search threads" autocomplete="off" />
    <ul>{#each hits as t (t.id)}<li><Row title={t.title} sub={t.workspace} icon="threads" href={href(`/threads/${t.id}`)} onclick={onclose} /></li>{/each}</ul>
    {#if !hits.length}<p>Nothing matches “{q}”.</p>{/if}
  </div>
</Sheet>

<style>
  .in { display: grid; gap: var(--s-4); padding: var(--s-5); }
  input { width: 100%; min-height: var(--pill-h); padding: 0 var(--s-6); border: 0; border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.08); font-size: var(--text-ui); color: var(--ink); }
  ul { display: grid; gap: var(--s-1); }
  p { color: var(--ink-3); padding: var(--s-4); }
</style>

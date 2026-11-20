<script lang="ts">
  // An app's sidebar: its declared collections, each shown small (a compact list, or a strip when the collection says so).
  // A section's heading opens the collection as a window; "New" appears when the app has a create operation for its kind.
  import { Icon } from "@calternal/ui";
  import { collectionId, windowOf, type Json, type Row } from "../lib/model";
  import { newAction, type Action } from "../lib/look";
  import RowLook from "./RowLook.svelte";
  import Strip from "./Strip.svelte";
  let { app, selected, onopen, oncollection, onaction }: { app: Json; selected: string | null; onopen: (r: Row) => void; oncollection: (c: Json) => void; onaction: (a: Action) => void } = $props();
</script>

<nav class="sb" aria-label={app.name}>
  {#each app.sidebar as c (c.collection)}
    {@const win = windowOf(collectionId(app, c))}
    {@const add = newAction(app, c)}
    <section class="sec">
      <header>
        <button type="button" class="h" onclick={() => oncollection(c)}>{c.collection}{#if win.count != null}<span class="n">{win.count}</span>{/if}</button>
        {#if add}<button type="button" class="ab" aria-label={add.label} onclick={() => onaction(add)}><Icon name="plus" inline /></button>{/if}
      </header>
      {#if c.view === "strip"}<Strip rows={win.rows} {selected} {onopen} label={c.collection} />
      {:else}
        {#each win.rows as row (row.item)}<RowLook {row} compact selected={selected === row.item} {onopen} />{/each}
        {#if !win.rows.length}<p class="em">Nothing here yet.</p>{/if}
      {/if}
    </section>
  {/each}
</nav>

<style>
  .sb { display: grid; gap: var(--s-2); padding: var(--s-2) var(--s-3) var(--s-6); }
  header { display: flex; align-items: center; margin-top: var(--s-5); }
  .h { flex: 1; display: flex; align-items: baseline; gap: var(--s-3); min-height: var(--row-h); padding: 0 var(--s-5); text-align: start; color: var(--ink-2); font-size: var(--text-heading); font-weight: var(--strong-weight); border-radius: var(--r-select); }
  .h:hover { background: var(--hover); } .n { color: var(--ink-3); font-size: var(--text-label); font-weight: normal; font-variant-numeric: tabular-nums; }
  .ab { display: grid; place-items: center; width: 28px; height: 28px; border-radius: var(--r-select); color: var(--ink-2); } .ab:hover { background: var(--hover); color: var(--ink); }
  .em { margin: 0; padding: var(--s-3) var(--s-5); color: var(--ink-3); font-size: var(--text-label); }
</style>

<script lang="ts">
  // The space's Trash: notes moved out of the list, newest deletion first, each with Restore (same note, same address) and Delete for good. The server keeps their history until they are deleted for good.
  import { Button, VirtualList } from "@calternal/ui";
  import { navigate } from "../../lib/route.svelte";
  import { shell } from "../../shell/shell.svelte";
  import { notes } from "./store.svelte";
  import "./column.css";
  import { shortDateTime } from "../../lib/format";
  import { IconButton } from "@calternal/ui";
  $effect(() => { void notes.space?.id; void notes.loadTrash(); });
</script>

<div class="wrap">
  <header class="nc-head">{#if shell.narrow}<IconButton icon="sidebar" label="Show notes" onclick={() => shell.toggleSidebar()} />{/if}</header>
  <article class="nc-article">
    <h1>Trash</h1>
    <p class="lede">Notes you moved here stay for as long as you keep them. Restore one to put it back where it was, or delete it for good.</p>
    {#if notes.trashed.length}
      <VirtualList inside count={notes.trashed.length} estimate={64} overscan={600} label="Trash" onrange={(_, end) => { if (end >= notes.trashed.length - 40) void notes.trashPages.next(); }}>
        {#snippet row(i)}{@const n = notes.trashed[i]}{#if n}
        <div class="r"><span class="n"><span class="t">{n.title}</span><span class="h">Deleted {shortDateTime(n.updatedAt)}</span></span>
        <span class="a"><Button text="Restore" variant="secondary" onclick={async () => { await notes.restore(n.id); }} /><Button text="Delete for good" variant="quiet" onclick={() => confirm(`Delete “${n.title}” for good? Its history goes too.`) && notes.purge(n.id)} /></span></div>
        {/if}{/snippet}
      </VirtualList>
    {:else if !notes.error}<p class="lede">The Trash is empty. A note you delete from the list waits here first.</p>{/if}
    {#if notes.trashPages.error}<p class="err" role="alert">{notes.trashPages.error} <Button text="Try again" variant="quiet" onclick={() => notes.trashPages.retry()} /></p>{/if}
    {#if notes.error}<p class="err" role="alert">{notes.error}</p>{/if}
    <p><Button text="Back to the notes" variant="quiet" onclick={() => navigate("/notes")} /></p>
  </article>
</div>

<style>
  .wrap { position: absolute; inset: 0; overflow-y: auto; }
  h1 { font-size: var(--text-title); font-weight: var(--strong-weight); }
  .lede { color: var(--ink-3); }
  .r { padding-bottom: var(--s-4); display: flex; align-items: center; justify-content: space-between; gap: var(--s-5); flex-wrap: wrap; }
  .n { display: grid; gap: var(--s-1); min-width: 0; }
  .t { font-size: var(--text-ui); font-weight: var(--strong-weight); }
  .h { font-size: var(--text-label); color: var(--ink-3); }
  .a { display: flex; gap: var(--s-2); }
  .err { color: var(--danger); font-size: var(--text-label); }
</style>

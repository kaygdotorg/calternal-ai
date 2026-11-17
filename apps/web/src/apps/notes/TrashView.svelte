<script lang="ts">
  // The space's Trash: notes moved out of the list, newest deletion first, each with Restore (same note, same address) and Delete for good. The server keeps their history until they are deleted for good.
  import { Button } from "@calternal/ui";
  import { navigate } from "../../lib/route.svelte";
  import { shell } from "../../shell/shell.svelte";
  import { notes } from "./store.svelte";
  import { IconButton } from "@calternal/ui";
  $effect(() => { void notes.space?.id; void notes.loadTrash(); });
  const when = (iso: string) => new Date(iso).toLocaleString(undefined, { month: "short", day: "numeric", hour: "numeric", minute: "2-digit" });
</script>

<div class="wrap">
  <header>{#if shell.narrow}<IconButton icon="sidebar" label="Show notes" onclick={() => shell.toggleSidebar()} />{/if}</header>
  <article>
    <h1>Trash</h1>
    <p class="lede">Notes you moved here stay for as long as you keep them. Restore one to put it back where it was, or delete it for good.</p>
    {#each notes.trashed as n (n.id)}
      <div class="r"><span class="n"><span class="t">{n.title}</span><span class="h">Deleted {when(n.updatedAt)}</span></span>
        <span class="a"><Button text="Restore" variant="secondary" onclick={async () => { await notes.restore(n.id); }} /><Button text="Delete for good" variant="quiet" onclick={() => confirm(`Delete “${n.title}” for good? Its history goes too.`) && notes.purge(n.id)} /></span></div>
    {:else}
      {#if !notes.error}<p class="lede">The Trash is empty. A note you delete from the list waits here first.</p>{/if}
    {/each}
    {#if notes.error}<p class="err" role="alert">{notes.error}</p>{/if}
    <p><Button text="Back to the notes" variant="quiet" onclick={() => navigate("/notes")} /></p>
  </article>
</div>

<style>
  .wrap { position: absolute; inset: 0; overflow-y: auto; }
  header { min-height: calc(var(--bar-h) + var(--safe-top)); padding: var(--safe-top) var(--s-5) 0 calc(var(--s-5) + var(--edge-l, 0px)); display: flex; align-items: center; }
  article { max-width: 40rem; margin: 0 auto; padding: var(--s-5) var(--pad-region) calc(var(--s-10) + var(--bottom-inset, 0px)); display: grid; gap: var(--s-5); }
  h1 { font-size: var(--text-title); font-weight: var(--strong-weight); }
  .lede { color: var(--ink-3); }
  .r { display: flex; align-items: center; justify-content: space-between; gap: var(--s-5); flex-wrap: wrap; }
  .n { display: grid; gap: var(--s-1); min-width: 0; }
  .t { font-size: var(--text-ui); font-weight: var(--strong-weight); }
  .h { font-size: var(--text-label); color: var(--ink-3); }
  .a { display: flex; gap: var(--s-2); }
  .err { color: var(--danger); font-size: var(--text-label); }
</style>

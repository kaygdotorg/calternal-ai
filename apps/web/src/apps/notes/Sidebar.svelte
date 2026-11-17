<script lang="ts">
  // Notes sidebar: the current space's notes (the fixture's, or the server's in server mode), newest first, and a way to make a new one.
  import { Row } from "@calternal/ui";
  import { href, navigate, segments } from "../../lib/route.svelte";
  import { notes } from "./store.svelte";
  const current = $derived(segments()[0] === "notes" ? segments()[1] : undefined);
  async function create() {
    const n = await notes.create("Untitled");
    if (n) navigate(`/notes/${n.id}`);
  }
</script>

<nav aria-label="Notes">
  {#if notes.space}<p class="sp">{notes.space.name}{notes.live ? " · server" : ""}</p>{/if}
  <ul>{#each notes.list as n (n.id)}<li><Row title={n.title} icon="notes" href={href(`/notes/${n.id}`)} selected={current === n.id} /></li>{:else}<li class="none">{notes.error || "No notes yet."}</li>{/each}</ul>
  <button type="button" class="new" onclick={create}>New note</button>
</nav>

<style>
  nav { display: grid; gap: var(--s-2); }
  ul { display: grid; gap: var(--s-1); }
  .sp { padding: 0 var(--s-5); font-size: var(--text-label); font-weight: var(--strong-weight); color: var(--ink-3); }
  .none { padding: var(--s-4) var(--s-5); color: var(--ink-3); font-size: var(--text-label); }
  .new { text-align: start; min-height: var(--row-h); padding: 0 var(--s-5); border-radius: var(--r-select); color: var(--ink-3); font-size: var(--text-label); font-weight: var(--strong-weight); }
</style>

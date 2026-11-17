<script lang="ts">
  // Notes sidebar: the current space's notes (the fixture's, or the server's in server mode), newest first, and a way to make a new one.
  import { Button, Row } from "@calternal/ui";
  import { href, navigate, segments } from "../../lib/route.svelte";
  import { spaces } from "../../shell/spaces.svelte";
  import { notes } from "./store.svelte";
  // The list follows the space the shell is showing.
  $effect(() => { void notes.useSpace(spaces.current.id); });
  const current = $derived(segments()[0] === "notes" ? segments()[1] : undefined);
  async function create() {
    const n = await notes.create("Untitled");
    if (n) navigate(`/notes/${n.id}`);
  }
</script>

<nav aria-label="Notes">
  {#if notes.space}<p class="sp">{notes.space.name}{notes.live ? " (server)" : ""}</p>{/if}
  <ul>{#each notes.list as n (n.id)}<li><Row title={n.title} icon="notes" href={href(`/notes/${n.id}`)} selected={current === n.id} /></li>{:else}<li class="none">{notes.error || "No notes yet."}{#if notes.error} <button type="button" class="retry" onclick={() => notes.retry()}>Try again</button>{/if}</li>{/each}</ul>
  <button type="button" class="new" onclick={create}>New note</button>
  {#if notes.waiting.length}
    <div class="wait" role="group" aria-label="Waiting to send">
      <p class="sp">Waiting to send</p>
      {#each notes.waiting as w (w.id)}
        <div class="w">
          <span class="wt">{w.operation === "notes.update" ? String(w.input.title ?? "Note edit") : w.operation}{w.state === "confirmation_required" ? " · needs you" : ""}</span>
          <Button variant="quiet" text={w.state === "confirmation_required" ? "Confirm" : "Send now"} onclick={() => notes.sendWaiting(w.id)} />
          <Button variant="quiet" text="Cancel" onclick={() => notes.cancelWaiting(w.id)} />
        </div>
      {/each}
    </div>
  {/if}
</nav>

<style>
  nav { display: grid; gap: var(--s-2); }
  ul { display: grid; gap: var(--s-1); }
  .sp { padding: 0 var(--s-5); font-size: var(--text-label); font-weight: var(--strong-weight); color: var(--ink-3); }
  .retry { text-decoration: underline; color: var(--ink-2); }
  .none { padding: var(--s-4) var(--s-5); color: var(--ink-3); font-size: var(--text-label); }
  .wait { display: grid; gap: var(--s-1); margin-top: var(--s-4); }
  .w { display: flex; flex-wrap: wrap; align-items: center; gap: var(--s-1); padding: var(--s-2) var(--s-5); font-size: var(--text-label); }
  .wt { flex: 1 1 100%; color: var(--ink-2); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .new { text-align: start; min-height: var(--row-h); padding: 0 var(--s-5); border-radius: var(--r-select); color: var(--ink-3); font-size: var(--text-label); font-weight: var(--strong-weight); }
</style>

<script lang="ts">
  // The Threads app's middle card: the open thread, or a quiet empty state when none is chosen.
  import ThreadView from "./ThreadView.svelte";
  import { Button, toast } from "@calternal/ui";
  import { navigate } from "../../lib/route.svelte";
  import { startThread, store } from "./store.svelte";
  let { id }: { id?: string } = $props();
</script>

{#if id && store.get(id)}
  {#key id}<ThreadView {id} />{/key}
{:else if id && store.ready}
  <p class="empty" role="status">That thread does not exist, or you cannot see it.</p>
{:else if !id && store.ready && store.threads.length === 0}
  <!-- A fresh account (on a phone the list is not showing, so this view says it too): what a thread is, and one action. -->
  <div class="first">
    <h2>No threads yet</h2>
    <p>A thread is a conversation with an agent that works on your files and notes, with only the access you allow. Start one and say what you want done.</p>
    <Button variant="primary" text="Start a thread" onclick={() => void startThread().then((t) => navigate(`/threads/${t.id}`)).catch((e: Error) => toast(`Could not start a thread: ${e.message}`))} />
  </div>
{:else if !id}
  <p class="empty">Choose a thread from the list.</p>
{/if}

<style>
  .first { position: absolute; inset: 0; display: grid; align-content: center; justify-items: start; gap: var(--s-4); max-width: 30rem; margin: 0 auto; padding: 0 var(--pad-region); }
  .first h2 { font-size: var(--text-heading); font-weight: var(--strong-weight); }
  .first p { color: var(--ink-2); font-size: var(--text-ui); }
  .empty { position: absolute; inset: 0; display: grid; place-items: center; color: var(--ink-3); font-size: var(--text-ui); }
</style>

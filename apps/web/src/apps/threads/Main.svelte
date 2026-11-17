<script lang="ts">
  // The Threads app's middle card: the open thread, or a quiet empty state when none is chosen.
  import ThreadView from "./ThreadView.svelte";
  import { store } from "./store.svelte";
  let { id }: { id?: string } = $props();
</script>

{#if id && store.get(id)}
  {#key id}<ThreadView {id} />{/key}
{:else if id && store.ready}
  <p class="empty" role="status">That thread does not exist, or you cannot see it.</p>
{:else if !id}
  <p class="empty">Choose a thread from the list.</p>
{/if}

<style>
  .empty { position: absolute; inset: 0; display: grid; place-items: center; color: var(--ink-3); font-size: var(--text-ui); }
</style>

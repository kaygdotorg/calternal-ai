<script lang="ts">
  import { app } from '../lib/store.svelte';
  let closeBtn: HTMLButtonElement | undefined = $state();
  $effect(() => { closeBtn?.focus(); });
</script>

<aside class="ai" aria-label="AI chat">
  <header>
    <h2>AI chat</h2>
    <button bind:this={closeBtn} class="btn quiet" onclick={() => (app.aiOpen = false)} aria-label="Close AI pane">✕</button>
  </header>
  <div class="body">
    <span class="chip warn">Setup pending</span>
    <h3>AI conversations are not connected</h3>
    <p class="muted">No agent harness is integrated in this development preview, so nothing here is simulated. Notes, plugins and the extension statistics work without it.</p>
    <p class="muted small">When harness integration lands, conversations about the open note will appear in this pane.</p>
    <label class="sr-only" for="ai-input">Message (unavailable)</label>
    <textarea id="ai-input" class="input" rows="3" placeholder="Unavailable until AI is set up" disabled></textarea>
  </div>
</aside>

<style>
  .ai { height: 100%; display: flex; flex-direction: column; padding: 12px 12px 12px 4px; min-width: 0; }
  header { display: flex; align-items: center; justify-content: space-between; height: 44px; padding-left: 8px; }
  h2 { font-size: 15px; }
  .body { display: grid; gap: 10px; align-content: start; padding: 8px; }
  p { margin: 0; }
  .small { font-size: 12px; }
  h3 { font-size: 15px; }
  textarea { resize: none; margin-top: 8px; }
  .chip { justify-self: start; }
</style>

<script lang="ts">
  // A wait that is real: nothing appears for the first 400 ms (most waits are over by then), after that the seconds counted up from when it began (infraternal's rule: never a spinner, never invented progress).
  // `progress` (done and total) replaces the seconds when the work knows how far it is.
  import { onDestroy } from "svelte";
  let { label, progress }: { label: string; progress?: { done: number; total: number } } = $props();
  const t0 = Date.now();
  let secs = $state(0), shown = $state(false);
  const show = setTimeout(() => (shown = true), 400);
  const tick = setInterval(() => (secs = Math.floor((Date.now() - t0) / 1000)), 500);
  onDestroy(() => { clearTimeout(show); clearInterval(tick); });
</script>

{#if shown}
  <p class="wait" role="status" aria-live="polite"><span>{label}</span><span class="n">{progress ? `${progress.done} of ${progress.total}` : `${secs} s`}</span></p>
{/if}

<style>
  .wait { display: flex; gap: var(--s-4); align-items: baseline; font-size: var(--text-label); color: var(--ink-3); animation: in var(--dur-base) var(--ease-out); }
  .n { font-variant-numeric: tabular-nums; color: var(--ink-2); }
  @keyframes in { from { opacity: 0; } }
  @media (prefers-reduced-motion: reduce) { .wait { animation: none; } }
</style>

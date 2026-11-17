<script lang="ts">
  // A group: a bold header, then an inset rounded card of rows (docs/ui-craft-and-settings-study.md 11.2: header 8 px above the card, 24 px between groups).
  import type { Snippet } from "svelte";
  let { title, danger = false, children }: { title?: string; danger?: boolean; children: Snippet } = $props();
</script>

<section class="g" class:danger>
  {#if title}<h2>{title}</h2>{/if}
  <div class="sg">{@render children()}</div>
</section>

<style>
  .g { display: grid; gap: var(--s-4); margin-top: var(--s-8); }
  h2 { padding: 0 var(--s-5); font-size: var(--text-ui); font-weight: var(--strong-weight); }
  .danger h2 { color: var(--ink-2); }
  .sg { border-radius: var(--r-card); background: rgb(var(--ink-rgb) / 0.05); box-shadow: inset 0 0 0 1px rgb(var(--ink-rgb) / 0.05); overflow: clip; }
  .sg { display: grid; }
</style>

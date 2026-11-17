<script lang="ts">
  // Loads a component's code when it is first drawn, then draws it with the same props. Used for the parts of an app the first paint of another app does not need (docs/ui-performance.md,
  // "first load"). Nothing is drawn until the code is in (a few milliseconds from the same origin); a failed load says so instead of leaving a blank card.
  import type { Component } from "svelte";
  let { load, ...rest }: { load: () => Promise<{ default: Component<any> }>; [k: string]: unknown } = $props();
</script>

{#await load()}
  <!-- loading -->
{:then m}
  {@const C = m.default}
  <C {...rest} />
{:catch}
  <p role="alert" class="lazy-err">This part did not load. Reload the page to try again.</p>
{/await}

<style>
  .lazy-err { padding: var(--s-6); color: var(--ink-2); }
</style>

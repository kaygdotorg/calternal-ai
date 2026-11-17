<script lang="ts">
  // Long tool output. Short logs are one <pre>; a long one is windowed: fixed line height, so the first line to draw is
  // scrollTop / lineHeight and only a screenful of lines is ever in the DOM (a 2,400-line log draws about 30 nodes).
  // The text is split once, when first opened, not per scroll.
  let { text, label }: { text: string; label: string } = $props();
  const lines = $derived(text.split("\n"));
  const LIMIT = 40;
  const view = 15; // lines visible in the window
  let top = $state(0);
  let box = $state<HTMLDivElement>();
  const first = $derived(Math.max(0, top - 6));
  const shown = $derived(lines.slice(first, Math.min(lines.length, top + view + 6)));
</script>

{#if lines.length <= LIMIT}
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <pre class="log" tabindex="0" aria-label={label}>{text}</pre>
{:else}
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div bind:this={box} class="log win" tabindex="0" role="region" aria-label="{label}, {lines.length} lines" style:--lh="1.6em" style:--view={view} onscroll={() => { top = Math.floor((box?.scrollTop ?? 0) / (box ? parseFloat(getComputedStyle(box).lineHeight) : 20)); }}>
    <div class="space" style:height="calc({lines.length} * var(--lh))">
      <pre style:transform="translateY(calc({first} * var(--lh)))">{shown.join("\n")}</pre>
    </div>
  </div>
{/if}

<style>
  .log { margin: 0; padding: var(--s-4) var(--s-5); border-radius: var(--r-control); background: rgb(var(--ink-rgb) / 0.06); font-size: var(--text-label); line-height: 1.6; overflow: auto; max-height: calc(var(--view, 15) * 1.6em); white-space: pre; color: var(--ink-2); }
  .win { padding: 0 var(--s-5); }
  .space { position: relative; }
  .win pre { position: absolute; left: 0; margin: 0; font: inherit; line-height: inherit; }
</style>

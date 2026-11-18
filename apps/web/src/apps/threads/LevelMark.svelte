<script lang="ts">
  // A model's reasoning level as a small mark: three bars of rising height, filled up to the level (the first of the model's levels fills one, the last fills all three). It is the second fact on the model chip, an icon
  // rather than a word (docs/composer.md, "Chips and adaptive rows"); the word is in the chip's tooltip and accessible name.
  let { level, levels }: { level: string; levels: string[] } = $props();
  const filled = $derived(levels.length <= 1 ? 3 : Math.max(1, Math.round(((Math.max(0, levels.indexOf(level)) + 1) / levels.length) * 3)));
</script>

<svg class="lvl" viewBox="0 0 12 12" width="12" height="12" aria-hidden="true">
  {#each [0, 1, 2] as i}<rect x={1 + i * 4} y={9 - i * 3 - 1} width="2.5" height={3 + i * 3 + 1} rx="1" class:on={i < filled} />{/each}
</svg>

<style>
  .lvl { flex: none; }
  rect { fill: currentColor; opacity: 0.28; }
  rect.on { opacity: 1; }
</style>

<script lang="ts">
  // The one seamless background behind both sidebars: the wallpaper (blurred once when chosen, appearance/wallpaper.ts) or, with none, the
  // tinted mesh. A wash of the scheme's tint lies over it; its strength is derived from the picture's brightness so ink stays legible.
  // Nothing here is a live filter.
</script>

<div class="backdrop" aria-hidden="true"><div class="wash"></div></div>

<style>
  .backdrop { position: fixed; inset: 0; z-index: 0; background: var(--wall-side, var(--backdrop)) center / cover no-repeat; }
  .wash { position: absolute; inset: 0; background: rgb(var(--tint) / var(--wash, var(--side-tint-a))); transition: background-color var(--dur-base) var(--ease-out); }
</style>

<script lang="ts">
  // A row-shaped placeholder at the real row height, so nothing shifts when the window arrives (docs/native-feel.md section 10).
  let { icon = false, width = 60 }: { icon?: boolean; width?: number } = $props();
</script>

<div class="sk" aria-hidden="true" data-skeleton>{#if icon}<i class="ic"></i>{/if}<i class="bar" style:width="{width}%"></i></div>

<style>
  .sk { display: flex; align-items: center; gap: var(--s-4); min-height: var(--row-h); padding: 0 var(--s-5); }
  i { display: block; border-radius: var(--r-select); background: color-mix(in oklab, var(--ink) 9%, transparent); animation: pulse 1.4s ease-in-out infinite; }
  .ic { width: var(--glyph); height: var(--glyph); flex: none; }
  .bar { height: 0.8em; }
  @keyframes pulse { 50% { opacity: 0.55; } }
  @media (prefers-reduced-motion: reduce) { i { animation: none; } }
</style>

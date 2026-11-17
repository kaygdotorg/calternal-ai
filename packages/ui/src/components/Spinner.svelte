<script lang="ts">
  // A small ring for work in progress. Pair it with text that says what is happening; on its own it is hidden from assistive tech.
</script>

<span class="spin" aria-hidden="true"></span>

<style>
  .spin { display: inline-block; width: var(--glyph-inline); height: var(--glyph-inline); border-radius: 50%; border: 2px solid color-mix(in oklab, currentColor 22%, transparent); border-top-color: currentColor; animation: spin 0.9s var(--ease-out) infinite; }
  @keyframes spin { to { rotate: 360deg; } }
  @media (prefers-reduced-motion: reduce) { .spin { animation: none; border-top-color: color-mix(in oklab, currentColor 60%, transparent); } }
</style>

<script lang="ts">
  // The glass layer (styles/glass.css): put it inside a surface as its first child. It has no children and no pointer events.
  // The surface itself registers with `use:adaptive` (lib/material.ts) so the layer takes the tint and blur its backdrop needs.
  let { dense = false, flat = false }: { dense?: boolean; flat?: boolean } = $props();
</script>

<div class="glass" class:dense class:flat aria-hidden="true"></div>

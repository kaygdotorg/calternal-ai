<script lang="ts">
  // The one seamless tinted background behind the whole app. Painted by CSS from the token mesh (--backdrop); fixed to the viewport so
  // surfaces on it can be adapted from the same mesh without reading pixels (packages/ui lib/material.ts).
</script>

<div class="backdrop" aria-hidden="true"></div>

<style>
  .backdrop { position: fixed; inset: 0; z-index: 0; background: var(--backdrop); }
</style>

<script lang="ts">
  // An address no installed app handles. It looks the same whether the thing is missing or you may not see it (docs/addresses.md).
  import { Button } from "@calternal/ui";
  import { navigate } from "../lib/route.svelte";
</script>

<div class="u" role="status"><h1>Nothing here</h1><p>There is nothing at this address, or you can’t see it.</p><Button text="Go to Threads" onclick={() => navigate("/threads")} /></div>

<style>
  .u { position: absolute; inset: 0; display: grid; place-content: center; justify-items: center; gap: var(--s-4); text-align: center; }
  h1 { font-size: var(--text-heading); font-weight: var(--strong-weight); } p { color: var(--ink-3); }
</style>

<script lang="ts">
  // The Notes middle card (loaded when first drawn, so it is not in the first load of another app). `/collections/<id>` is a page of this app: a hand-picked set of notes.
  import Lazy from "../Lazy.svelte";
  import { route } from "../../lib/route.svelte";
  let props = $props();
</script>

{#if route.path.startsWith("/collections/")}
  <Lazy load={() => import("./CollectionView.svelte")} {...props} />
{:else}
  <Lazy load={() => import("./Notes.svelte")} {...props} />
{/if}

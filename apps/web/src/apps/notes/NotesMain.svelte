<script lang="ts">
  // The Notes middle card (loaded when first drawn, so it is not in the first load of another app).
  import Lazy from "../Lazy.svelte";
  let props = $props();
</script>

<Lazy load={() => import("./Notes.svelte")} {...props} />

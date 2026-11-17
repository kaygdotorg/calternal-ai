<script lang="ts">
  // The Notes right-card panels (loaded when first drawn, so it is not in the first load of another app).
  import Lazy from "../Lazy.svelte";
  let props = $props();
</script>

<Lazy load={() => import("./Panels.svelte")} {...props} />

<script lang="ts">
  // Draws the Share sheet when it is open for this item. The note's app mounts one per note it shows (a window in the canvas is a Notes app too), so only the sheet whose item it is appears.
  import ShareSheet from "./ShareSheet.svelte";
  import { closeShare, shareUi } from "./share.svelte";
  import type { ItemRef } from "./sharing";
  let { item }: { item: ItemRef } = $props();
  const mine = $derived(shareUi.item?.kind === item.kind && shareUi.item.id === item.id);
</script>

{#if mine}<ShareSheet {item} title={shareUi.title} trigger={shareUi.trigger} onclose={closeShare} />{/if}

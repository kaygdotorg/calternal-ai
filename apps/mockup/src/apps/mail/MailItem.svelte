<script lang="ts">
  // What a mail item draws when it is open: its conversation, or, when it is a draft, the compose window. The same component serves the item window and a mail window's own page.
  import type { Item } from "../../data/collections";
  import Compose from "./Compose.svelte";
  import Thread from "./Thread.svelte";
  import { DRAFTS } from "./data";
  let { item, win }: { item: Item; win?: { id: string } } = $props();
</script>

{#if DRAFTS[item.id]}<Compose {item} {win} />{:else}<Thread {item} />{/if}

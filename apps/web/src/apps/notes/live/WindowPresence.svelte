<script lang="ts">
  // Who else is in a note, for the note's window in the canvas: the window's own actions pill mounts this beside its Share button (`<WindowPresence item={win.item} />` for a notes window), because on a phone the window's title pill
  // covers the note's header, where the presence sits when the note is not in a window. Draws nothing when the note is not live or nobody else is in it. Same strip, same labels button, same room as the header's.
  import Presence from "./Presence.svelte";
  import { liveOf, liveOn } from "./live.svelte";
  let { item }: { item: string } = $props();
  const room = $derived(liveOn() ? liveOf(item) : null);
</script>

{#if room?.session}<Presence here={room.here} />{/if}

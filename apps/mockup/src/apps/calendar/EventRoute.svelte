<script lang="ts">
  // What an event item draws: the form when it is one being written, else the event page.
  import type { Item } from "../../data/collections";
  import type { Win } from "../../windows.svelte";
  import EventForm from "./EventForm.svelte";
  import EventPage from "./EventPage.svelte";
  import { isNewEvent } from "./new";
  let { item, win, onback, backLabel }: { item: Item; win?: Win; onback?: () => void; backLabel?: string } = $props();
</script>

{#if isNewEvent(item.id)}<EventForm {item} {win} />{:else}<EventPage {item} {onback} {backLabel} />{/if}

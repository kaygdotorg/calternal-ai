<script lang="ts">
  // What a contact item draws: the form when it is one being written, else the contact page.
  import type { Item } from "../../data/collections";
  import type { Win } from "../../windows.svelte";
  import ContactCard from "./ContactCard.svelte";
  import ContactForm from "./ContactForm.svelte";
  import { isNewContact } from "./new";
  let { item, win, onback, onall }: { item: Item; win?: Win; onback?: () => void; onall?: (cid: string) => void } = $props();
</script>

{#if isNewContact(item.id)}<ContactForm {item} {win} />{:else}<ContactCard {item} {onback} {onall} />{/if}

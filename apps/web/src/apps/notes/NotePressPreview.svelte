<script lang="ts">
  // The card of a note row's press (ui/lib/press.ts): the note's title and its top, rendered as the note is (Md), from what was already read (`tops.ts`); a note never opened
  // says so (nothing is fetched on a press). Registered as the `note` preview.
  import Md from "../../lib/Md.svelte";
  import { notes } from "./store.svelte";
  import { topOf } from "./tops";
  let { item }: { item: string } = $props();
  const title = $derived(notes.list.find((n) => n.id === item)?.title ?? "Note");
  const top = $derived(topOf(item));
</script>

<div class="pv">
  <p class="ti">{title}</p>
  {#if top}<div class="md-top"><Md text={top} /></div>{:else}<p class="none">Not opened yet: open it to read it.</p>{/if}
</div>

<style>
  .pv { display: grid; gap: var(--s-3); padding: var(--s-5) var(--s-5) var(--s-6); font-size: var(--text-label); color: var(--ink-2); }
  .ti { color: var(--ink); font-size: var(--text-ui); font-weight: var(--strong-weight); }
  .md-top { font-size: var(--text-label); }
  .none { color: var(--ink-3); }
</style>

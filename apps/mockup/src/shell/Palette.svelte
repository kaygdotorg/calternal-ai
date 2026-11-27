<script lang="ts">
  // The search palette: a placeholder (it opens from the foot's search icon and from Mod+K) so the foot has somewhere to go; nothing is searched in the mockup.
  import { SearchField } from "@calternal/ui";
  import GlassSheet from "./GlassSheet.svelte";
  let { open, onclose }: { open: boolean; onclose: () => void } = $props();
  let q = $state("");
</script>

<GlassSheet {open} label="Search" {onclose}>
  <div class="p"><SearchField bind:value={q} label="Search everything" placeholder="Search apps, collections, chats and notes" /><p class="note">Search is a placeholder in the mockup.</p></div>
</GlassSheet>

<style>
  .p { display: grid; gap: var(--s-4); padding: var(--s-5); min-width: min(34rem, 88vw); } .note { margin: 0; color: var(--ink-3); font-size: var(--text-label); }
</style>

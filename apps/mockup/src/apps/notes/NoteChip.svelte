<script lang="ts">
  // A link to another note, drawn as a chip: the note glyph and the title on a soft pill. Choosing it opens the note (in place in a Notes window, else beside). Used in the page's text, in the backlinks, in tasks that came from a note.
  import Glyph from "../../shell/AppIcon.svelte";
  import { ITEMS } from "../../data/collections";
  import { wins } from "../../windows.svelte";
  let { id }: { id: string } = $props();
  const note = $derived(ITEMS[id]);
</script>

{#if note}<button type="button" class="nc" data-note-link={id} onclick={() => wins.openItem(note)}><Glyph name="notes" inline /><span>{note.title}</span></button>{/if}

<style>
  .nc { position: relative; min-height: 0; display: inline-flex; align-items: center; gap: var(--s-2); max-width: 100%; vertical-align: baseline; padding: 0 var(--s-4) 0 var(--s-3); border-radius: var(--r-pill); background: var(--accent-soft); color: color-mix(in oklab, var(--accent) 72%, var(--ink)); font: inherit; font-size: 0.95em; font-weight: var(--ui-weight); line-height: 1.25; padding-block: 0.05em; /* an inline link sits within the line it is in (the note's line height is 1.6): about 1.35 em tall; touch gets its 44 pt hit area from ::after, which takes no room */ transition: background-color var(--dur-fast) var(--ease-out), scale var(--dur-fast) var(--ease-out); }
  @media (hover: none) { .nc::after { content: ""; position: absolute; inset: -12px -4px; } }
  .nc span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } .nc:active { scale: var(--press-scale); }
  @media (hover: hover) and (pointer: fine) { .nc:hover { background: color-mix(in oklab, var(--accent-base) 22%, transparent); } }
</style>

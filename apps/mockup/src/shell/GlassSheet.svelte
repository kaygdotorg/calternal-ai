<script lang="ts">
  // THE glass surface for the mockup's top-of-window dialogs: the search palette and "New space" are both this (same material, radius, position and motion: a top sheet under the window's top edge, 28 px pane radius, the sheet motion). Its content is laid out by the caller.
  // MOTION lives here too: the dim and the surface come and go together (Sheet: --dur-sheet on --ease-sheet in, --dur-leave out), and buttons press to --press-scale. Legibility lives HERE, once: the sheet's glass is the product's own material, and this adds a backing tint over it (light: the warm paper at 72 %; dark: the near-black at 70 %) so lines of words hold at least 4.5:1 even on the brightest and busiest wallpaper, while the wallpaper still reads through as colour. Nothing that uses it draws its own card.
  import type { Snippet } from "svelte";
  import { Sheet } from "@calternal/ui";
  let { open, label, onclose, children }: { open: boolean; label: string; onclose: () => void; children: Snippet } = $props();
</script>

<Sheet {open} {label} side="top" {onclose}><div class="gs">{@render children()}</div></Sheet>

<style>
  .gs { border-radius: inherit; background: rgb(250 248 245 / 0.72); color: var(--ink); max-height: inherit; overflow: auto; }
  :global(html[data-scheme="dark"]) .gs { background: rgb(28 28 30 / 0.7); }
  /* press feedback for every button and row inside the surface, from the motion tokens */
  .gs :global(button) { transition: scale var(--dur-fast) var(--ease-out), background-color var(--dur-fast) var(--ease-out); } .gs :global(button:active:not(:disabled)) { scale: var(--press-scale); }
</style>

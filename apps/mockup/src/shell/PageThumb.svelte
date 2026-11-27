<script lang="ts">
  // A small page preview, as Craft draws a document: a rounded sheet with a soft shadow, an optional cover, the title in the page's own heading weight and the first lines of text. It is a picture of a page, so its text is a scaled drawing of a 600 px page (not a text size
  // of the interface) and it is hidden from screen readers; the caption beside it carries the name. Used for notes and documents in strips, galleries and cards.
  import type { Item } from "../data/collections";
  let { item, w = "100%" }: { item: Item; w?: string } = $props();
  const cover = $derived((item.id.charCodeAt(item.id.length - 1) + item.title.length) % 3 === 0);
  const body = $derived(item.body || item.sub || "");
</script>

<span class="pg" style:width={w} style:--h={item.hue ?? 205} aria-hidden="true">
  {#if cover}<span class="cv"></span>{/if}
  <span class="in"><span class="ti">{item.title}</span><span class="tx">{body}</span></span>
</span>

<style>
  .pg { display: block; aspect-ratio: 3 / 4; overflow: hidden; border-radius: var(--r-select); background: var(--paper, #fff); box-shadow: 0 1px 2px rgb(0 0 0 / 0.12), 0 4px 14px rgb(0 0 0 / 0.08), inset 0 0 0 1px rgb(0 0 0 / 0.05); container-type: inline-size; }
  :global(html[data-scheme="dark"]) .pg { --paper: #2b2b2e; box-shadow: 0 1px 2px rgb(0 0 0 / 0.4), 0 4px 14px rgb(0 0 0 / 0.3), inset 0 0 0 1px rgb(255 255 255 / 0.08); }
  .cv { display: block; height: 30%; background: linear-gradient(135deg, oklch(0.8 0.1 var(--h)), oklch(0.7 0.12 calc(var(--h) + 50))); }
  .in { display: grid; align-content: start; gap: 2cqw; padding: 7cqw; } .ti { font-size: 5.4cqw; line-height: 1.15; font-weight: 700; color: var(--ink); display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  .tx { font-size: 3.4cqw; line-height: 1.45; color: var(--ink-3); display: -webkit-box; -webkit-line-clamp: 12; -webkit-box-orient: vertical; overflow: hidden; }
</style>

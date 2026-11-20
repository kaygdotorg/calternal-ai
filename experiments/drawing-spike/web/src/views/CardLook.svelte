<script lang="ts">
  // One item drawn with its kind's card look (image, title, detail), for boards and galleries. Knows no kind.
  import { Icon } from "@calternal/ui";
  import Av from "./Av.svelte";
  import { draw } from "../lib/look";
  import { glyph } from "../lib/icons";
  import type { Row } from "../lib/model";
  let { row, onopen }: { row: Row; onopen: (r: Row) => void } = $props();
  const d = $derived(draw(row, "card"));
</script>

<button type="button" class="card" data-item={row.item} onclick={() => onopen(row)}>
  <span class="k">{#if d.avatars.length}<Av name={d.avatars[0]!.name} size={20} />{:else}<Icon name={glyph(d.icon)} inline />{/if}</span>
  <strong>{d.title}</strong>
  {#each d.detail as t, i (i)}<span class="s">{t}</span>{/each}
</button>

<style>
  .card { display: grid; gap: var(--s-2); width: 100%; padding: var(--s-4) var(--s-5); text-align: start; border-radius: var(--r-menu); background: var(--paper, rgb(255 255 255 / 0.9)); box-shadow: 0 1px 2px rgb(0 0 0 / 0.08), 0 2px 8px rgb(0 0 0 / 0.05); transition: scale var(--dur-fast) var(--ease-out); }
  .card:active { scale: var(--press-scale); }
  :global(html[data-scheme="dark"]) .card { --paper: rgb(255 255 255 / 0.08); }
  .k { color: var(--ink-3); display: flex; } strong { font-weight: var(--strong-weight); line-height: 1.3; display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  .s { color: var(--ink-3); font-size: var(--text-label); }
</style>

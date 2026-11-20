<script lang="ts">
  // A collection shown small as a strip: each item's leading (avatars, else its app icon) with its title as the label, in a
  // horizontal scroller with edge fades. The leading's unread count rides on it as a badge. Knows no kind.
  import { Icon } from "@calternal/ui";
  import Av from "./Av.svelte";
  import { draw } from "../lib/look";
  import { glyph } from "../lib/icons";
  import type { Row } from "../lib/model";
  let { rows, selected, onopen, label }: { rows: Row[]; selected: string | null; onopen: (r: Row) => void; label: string } = $props();
</script>

<div class="strip" role="list" aria-label={label}>
  {#each rows as row (row.item)}
    {@const d = draw(row, "cell")}
    {@const n = d.trailing.find((m) => "count" in m)}
    <button type="button" role="listitem" class="tile" class:sel={selected === row.item} onclick={() => onopen(row)} aria-label={d.title}>
      <span class="av">
        {#if d.avatars.length}<Av name={d.avatars[0]!.name} size={44} />{:else}<span class="sq"><Icon name={glyph(d.icon)} /></span>{/if}
        {#if n && "count" in n}<span class="badge" class:q={n.quiet}>{n.count}</span>{/if}
      </span>
      <span class="cap">{d.title}</span>
    </button>
  {/each}
</div>

<style>
  .strip { display: flex; gap: var(--s-3); overflow-x: auto; padding: var(--s-3) var(--s-3); scrollbar-width: none; mask-image: linear-gradient(to right, #000 calc(100% - 18px), transparent); }
  .tile { flex: none; width: 64px; display: grid; justify-items: center; gap: var(--s-2); padding: var(--s-2) 0; border-radius: var(--r-select); } .tile.sel { background: var(--accent-soft); }
  .av { position: relative; } .big :global(.avatar), .big :global(span) { --avatar: 44px; }
  .sq { display: grid; place-items: center; width: 44px; height: 44px; border-radius: 12px; background: rgb(var(--ink-rgb) / 0.08); color: var(--ink-2); }
  .badge { position: absolute; top: -4px; right: -6px; min-width: 18px; height: 18px; padding: 0 5px; box-sizing: border-box; border-radius: 9px; display: grid; place-items: center; background: var(--accent-base); color: #fff; font-size: 11px; font-weight: var(--strong-weight); box-shadow: 0 0 0 2px var(--side-bg, #fff); }
  .badge.q { background: #8e8e93; }
  .cap { max-width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: var(--text-label); color: var(--ink-2); }
</style>

<script lang="ts">
  // A note drawn in the shape the place needs, the same four a task has: a ONE-line `row` (page glyph, title, the day), a `strip` tile (a thumbnail and a short caption), a `card` (thumbnail beside the title, a quiet line, tags and the number of links), a `cell` (a gallery's big thumbnail and a caption).
  // A note is a page, so everywhere but a row it is drawn as its thumbnail. Selected is an outline on the thumbnail or a tint on the row; nothing moves. Fixture only.
  import Glyph from "../../shell/AppIcon.svelte";
  import NoteThumb from "./NoteThumb.svelte";
  import { docOf, linksTo } from "./data";
  import type { Item } from "../../data/collections";
  let { item, mode = "row", selected = false, onopen }: { item: Item; mode?: "row" | "strip" | "card" | "cell"; selected?: boolean; onopen?: (i: Item) => void } = $props();
  const doc = $derived(docOf(item)), links = $derived(linksTo(item.id).length);
</script>

{#if mode === "row"}
  <button type="button" class="row" class:sel={selected} aria-current={selected ? "true" : undefined} data-note={item.id} onclick={() => onopen?.(item)}><span class="g"><Glyph name="notes" /></span><span class="t">{item.title}</span>{#if item.when}<span class="w">{item.when}</span>{/if}</button>
{:else if mode === "strip"}
  <button type="button" class="tile" class:sel={selected} aria-label={item.title} data-note={item.id} onclick={() => onopen?.(item)}><span class="th"><NoteThumb {item} w="var(--tile-w, 4.5rem)" /></span><span class="cap">{item.title}</span></button>
{:else if mode === "card"}
  <button type="button" class="card" class:sel={selected} data-note={item.id} onclick={() => onopen?.(item)}>
    <span class="th"><NoteThumb {item} w="4.75rem" /></span>
    <span class="tx"><strong>{item.title}</strong><span class="q">{item.body}</span>
      <span class="ft">{#each doc.tags.slice(0, 2) as t (t)}<span class="tg">#{t}</span>{/each}<span class="sp"></span>{#if links}<span class="m"><Glyph name="link" inline />{links}</span>{/if}<span class="m">{item.when}</span></span></span>
  </button>
{:else}
  <button type="button" class="cell" class:sel={selected} data-note={item.id} onclick={() => onopen?.(item)}><span class="th"><NoteThumb {item} /></span><span class="cp"><strong>{item.title}</strong><span class="s">{doc.folder} · {item.when}</span></span></button>
{/if}

<style>
  button { text-align: start; color: var(--ink); font: inherit; }
  .row { display: flex; align-items: center; gap: var(--s-4); width: 100%; min-height: var(--row-h); padding: 0 var(--s-5); border-radius: var(--r-select); font-size: var(--text-ui); line-height: 1.25; transition: background-color var(--dur-fast) var(--ease-out); }
  @media (hover: hover) and (pointer: fine) { .row:not(.sel):hover { background: var(--hover); } } .row.sel { background: var(--accent-soft); }
  .g { flex: none; display: grid; place-items: center; width: var(--lead); color: var(--ink-2); } .row.sel .g { color: var(--accent); } .t { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: var(--ui-weight); } .w { flex: none; color: var(--ink-3); font-size: var(--text-label); font-variant-numeric: tabular-nums; }
  .tile { flex: none; display: grid; justify-items: center; align-content: start; gap: var(--s-2); width: calc(var(--tile-w, 4.5rem) + var(--s-3)); border-radius: var(--r-menu); transition: scale var(--dur-fast) var(--ease-out); } .tile:active, .cell:active, .card:active { scale: var(--press-scale); }
  .cap { max-width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: var(--text-section); line-height: 1.2; color: var(--ink-3); text-align: center; }
  .th { display: block; border-radius: var(--r-select); } .sel .th { outline: 2px solid var(--accent); outline-offset: 2px; }
  .card { display: flex; gap: var(--s-5); width: 100%; padding: var(--s-4); border-radius: var(--r-menu); background: rgb(var(--ink-rgb) / 0.06); box-shadow: 0 1px 2px rgb(var(--shadow-rgb) / 0.1); font-size: var(--text-ui); line-height: 1.3; transition: scale var(--dur-fast) var(--ease-out); } .card.sel { background: var(--accent-soft); }
  .card .th { flex: none; align-self: flex-start; } .tx { flex: 1; min-width: 0; display: grid; align-content: start; gap: var(--s-2); } .tx strong { font-weight: var(--strong-weight); display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  .q { color: var(--ink-3); font-size: var(--text-label); line-height: 1.4; display: -webkit-box; -webkit-line-clamp: 3; -webkit-box-orient: vertical; overflow: hidden; }
  .ft { display: flex; align-items: center; gap: var(--s-3); margin-top: var(--s-2); } .sp { flex: 1; } .tg { padding: 0 var(--s-4); min-height: var(--s-7); display: inline-flex; align-items: center; border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.08); color: var(--ink-2); font-size: var(--text-section); line-height: var(--s-7); } .m { display: inline-flex; align-items: center; gap: var(--s-2); color: var(--ink-3); font-size: var(--text-label); }
  .cell { display: grid; gap: var(--s-4); width: 100%; align-content: start; border-radius: var(--r-menu); transition: scale var(--dur-fast) var(--ease-out); } .cp { display: grid; gap: 1px; padding-inline: var(--s-2); } .cp strong { font-weight: var(--strong-weight); font-size: var(--text-ui); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } .s { color: var(--ink-3); font-size: var(--text-label); }
</style>

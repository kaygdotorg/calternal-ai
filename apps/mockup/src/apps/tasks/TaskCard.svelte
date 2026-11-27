<script lang="ts">
  // A task as a card (a board's, a strip's, a card in a mixed collection): the project as a small header, the checkbox with the title (two lines at most), the chips that matter, and a foot with who it is for and the progress of its subtasks.
  // The whole card opens the task; the checkbox is its own target. A card keeps its size when ticked (the title is struck, the card dims a little), so a board does not jump.
  import Check from "./Check.svelte";
  import Chips from "./Chips.svelte";
  import Glyph from "../../shell/AppIcon.svelte";
  import Av from "../../shell/Av.svelte";
  import { arr } from "../../arrange.svelte";
  import { projectOf, xOf } from "./data";
  import type { Item } from "../../data/collections";
  let { item, onopen }: { item: Item; onopen?: (i: Item) => void } = $props();
  const st = $derived(arr.statusOf(item.id) ?? item.status), done = $derived(st === "Done"), x = $derived(xOf(item.id)), pr = $derived(projectOf(x.project));
  const subs = $derived(x.subs ? `${x.subs.filter((s) => s[1]).length}/${x.subs.length}` : "");
</script>

<div class="card" class:done data-task={item.id}>
  <button type="button" class="open" aria-label={`Open: ${item.title}`} onclick={() => onopen?.(item)}></button>
  <span class="pj"><i style:--hue={pr.hue}></i>{x.project}</span>
  <span class="tk"><Check id={item.id} label={item.title} status={st} /><strong class="two" class:struck={done}>{item.title}</strong></span>
  <span class="chips"><Chips id={item.id} {done} cap={3} show={["when", "priority", "deadline", "repeat", "waiting", "tags"]} /></span>
  {#if item.owner || subs || x.note}<span class="ft">{#if item.owner}<Av name={item.owner} size="var(--s-7)" />{/if}<span class="sp"></span>{#if subs}<span class="m"><Glyph name="subtasks" inline />{subs}</span>{/if}{#if x.note}<span class="m"><Glyph name="notes" inline /></span>{/if}</span>{/if}
</div>

<style>
  .card { position: relative; display: grid; gap: var(--s-3); padding: var(--s-4) var(--s-4) var(--s-4) var(--s-5); border-radius: var(--r-menu); background: rgb(var(--ink-rgb) / 0.06); box-shadow: 0 1px 2px rgb(var(--shadow-rgb) / 0.1); font-size: var(--text-ui); line-height: 1.3; transition: opacity var(--dur-base) var(--ease-out), scale var(--dur-fast) var(--ease-out); }
  .card:has(.open:active) { scale: var(--press-scale); } .card.done { opacity: 0.62; }
  .open { position: absolute; inset: 0; border-radius: inherit; } .card > :not(.open) { position: relative; pointer-events: none; } .card :global(.ck) { pointer-events: auto; }
  .pj { display: inline-flex; align-items: center; gap: var(--s-3); color: var(--ink-2); font-size: var(--text-section); text-transform: uppercase; letter-spacing: 0.05em; } .pj i { width: var(--s-3); height: var(--s-3); border-radius: 50%; background: oklch(0.72 0.1 var(--hue)); }
  .tk { display: flex; align-items: flex-start; gap: var(--s-4); } .tk :global(.ck) { margin-block: calc((var(--row-h) - var(--glyph)) / -2 + var(--s-1)); margin-inline-end: calc(var(--s-3) * -1); align-self: flex-start; }
  strong { font-weight: var(--strong-weight); } .two { display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; } .struck { text-decoration: line-through; text-decoration-color: var(--ink-3); color: var(--ink-3); }
  .chips { display: flex; flex-wrap: wrap; gap: var(--s-3); } .ft { display: flex; align-items: center; gap: var(--s-4); } .sp { flex: 1; } .m { display: inline-flex; align-items: center; gap: var(--s-2); color: var(--ink-3); font-size: var(--text-label); }
</style>

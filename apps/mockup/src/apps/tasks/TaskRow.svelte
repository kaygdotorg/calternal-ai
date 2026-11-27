<script lang="ts">
  // A task as ONE line: the checkbox, the title, then at most three chips in a fixed order (when, priority, deadline, repeat, subtasks, waiting, tags) with the rest behind "+n", then (in a mixed list) the project. How many chips show is decided by the row's own width.
  // Ticking draws the tick, then strikes the title through (the line draws left to right, `--dur-base` on `--ease-out`, a beat after the box); the row stays where it is until the list is left, as the old app decided. Selected is a tint only: nothing moves.
  import Check from "./Check.svelte";
  import Chips from "./Chips.svelte";
  import { arr } from "../../arrange.svelte";
  import { projectOf, type X, xOf } from "./data";
  import type { Item } from "../../data/collections";
  let { item, selected = false, project = false, quiet = false, today = false, onopen }: { item: Item; selected?: boolean; /** Show the project at the end (a list that mixes projects). */ project?: boolean; /** In a note: no tint, no project, the day only. */ quiet?: boolean; today?: boolean; onopen?: (i: Item) => void } = $props();
  const st = $derived(arr.statusOf(item.id) ?? item.status), done = $derived(st === "Done");
  const x = $derived<X>(xOf(item.id)), pr = $derived(projectOf(x.project));
  // How many chips fit is decided by the row's own width (a window or a sidebar, not the screen): 3 from 38rem, then 2, 1 and none; the "+n" only from 27.5rem up; the project at the end only when there is room for it.
  let w = $state(0);
  const cap = $derived(quiet ? (w >= 260 ? 1 : 0) : w >= 608 ? 3 : w >= 440 ? 2 : w >= 240 ? 1 : 0), room = $derived(w >= 608);
</script>

<div class="tw" bind:clientWidth={w}><div class="tr" class:sel={selected} class:quiet data-task={item.id}>
  <Check id={item.id} label={item.title} status={st} />
  <button type="button" class="main" aria-current={selected ? "true" : undefined} onclick={() => onopen?.(item)}>
    <span class="ti"><span class="tx" class:done>{item.title}</span></span>
    <span class="chips"><Chips id={item.id} {done} {today} {cap} more={w >= 440} show={quiet ? ["when"] : undefined} /></span>
    {#if project && !quiet && room}<span class="pj"><i style:--hue={pr.hue}></i>{x.project}</span>{/if}
  </button>
</div></div>

<style>
  .tw { container-type: inline-size; }
  .tr { display: flex; align-items: center; gap: var(--s-4); min-height: var(--row-h); padding: 0 var(--s-5); border-radius: var(--r-select); font-size: var(--text-ui); line-height: 1.3; transition: background-color var(--dur-fast) var(--ease-out); }
  @media (hover: hover) and (pointer: fine) { .tr:not(.sel):hover { background: var(--hover); } }
  .tr.sel { background: var(--accent-soft); } .tr.quiet { padding-inline: 0; } .tr.quiet :global(.ck) { margin-inline-start: calc((var(--row-h) - var(--glyph)) / -2); }
  .main { flex: 1; min-width: 0; align-self: stretch; display: flex; align-items: center; gap: var(--s-4); padding: 0; color: var(--ink); text-align: start; font: inherit; }
  .ti { flex: 1 1 0; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: var(--ui-weight); }
  /* the strike is a line drawn under the text's middle; it grows from the left once the tick has landed */
  .tx { color: var(--ink); background: linear-gradient(currentColor, currentColor) no-repeat 0 58% / 0 1px; transition: background-size var(--dur-base) var(--ease-out) var(--dur-fast), color var(--dur-base) var(--ease-out) var(--dur-fast); }
  .tx.done { color: var(--ink-3); background-size: 100% 1px; }
  .chips { flex: none; display: inline-flex; align-items: center; gap: var(--s-3); }
  .pj { flex: none; display: inline-flex; align-items: center; gap: var(--s-3); color: var(--ink-2); font-size: var(--text-label); } .pj i { width: var(--s-4); height: var(--s-4); border-radius: 50%; background: oklch(0.72 0.1 var(--hue)); }
</style>

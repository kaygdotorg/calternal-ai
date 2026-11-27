<script lang="ts">
  // An event as the shell's boards and galleries draw it (they pass an item and a shape). CARD: the calendar's tint with its stripe, the day and time, the title, where. CELL: a tile whose face is the day of the month over the weekday in the calendar's tint, with the title and the time under it.
  // The shapes inside Calendar's own pages are in EventLook.svelte; this one is for places where the event sits among other kinds. Fixture only.
  import type { Item } from "../../data/collections";
  import { calOf, EV, when } from "./data";
  import { dayNum, longDate, weekdayShort } from "./dates";
  let { item, mode, onopen }: { item: Item; mode: "row" | "strip" | "card" | "cell"; onopen?: (i: Item) => void } = $props();
  const ev = $derived(EV[item.id]), first = $derived(ev?.days[0] ?? item.date ?? "2026-11-20"), hue = $derived(ev ? calOf(ev.cal).hue : (item.hue ?? 205));
</script>

{#if mode === "cell" || mode === "strip"}
  <button type="button" class="cell" data-event-cell={item.id} data-item={item.id} style:--hue={hue} onclick={() => onopen?.(item)}>
    <span class="face"><b>{dayNum(first)}</b><span>{weekdayShort(first)}</span></span><span class="cp"><strong>{item.title}</strong><span class="q">{ev ? when(ev) : item.sub}</span></span>
  </button>
{:else}
  <button type="button" class="card" data-event-card={item.id} data-item={item.id} style:--hue={hue} onclick={() => onopen?.(item)}>
    <span class="d">{longDate(first)}{ev?.start ? `, ${ev.start}` : ""}</span><strong class="two">{item.title}</strong>{#if ev?.where}<span class="q">{ev.where}</span>{/if}
  </button>
{/if}

<style>
  button { color: var(--ink); text-align: start; }
  .card { position: relative; display: grid; gap: var(--s-2); width: 100%; box-sizing: border-box; padding: var(--s-4) var(--s-4) var(--s-4) var(--s-6); border-radius: var(--r-menu); background: oklch(0.78 0.09 var(--hue) / 0.35); font-size: var(--text-ui); line-height: 1.3; transition: scale var(--dur-fast) var(--ease-out); } .card:active { scale: var(--press-scale); }
  .card::before { content: ""; position: absolute; inset: var(--s-3) auto var(--s-3) var(--s-3); width: 3px; border-radius: var(--r-pill); background: oklch(0.68 0.13 var(--hue)); } .d, .q { color: var(--ink-2); font-size: var(--text-label); font-variant-numeric: tabular-nums; } .card strong { font-weight: var(--strong-weight); } .two { display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  .cell { display: grid; gap: var(--s-3); width: 100%; align-content: start; } .face { display: grid; place-content: center; justify-items: center; aspect-ratio: 4 / 3; border-radius: var(--r-menu); background: oklch(0.78 0.09 var(--hue) / 0.4); } .face b { font-size: var(--text-title); line-height: 1; font-weight: var(--strong-weight); font-variant-numeric: tabular-nums; } .face span { margin-top: var(--s-2); color: var(--ink-2); font-size: var(--text-section); text-transform: uppercase; letter-spacing: 0.05em; }
  .cp { display: grid; min-width: 0; } .cp strong { font-weight: var(--strong-weight); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } .cp .q { color: var(--ink-3); font-size: var(--text-label); }
</style>

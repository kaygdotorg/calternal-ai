<script lang="ts">
  // An event's three looks. CELL: one line in a month's day (a dot, the start time, the title; an all-day event is a filled bar with the title). CARD: a block in a week or day column, its calendar's tint with a stripe on the start edge, the title, the time and where.
  // ROW: one line in a list (a stripe, the time range, the title, where, quietly). The colour is the calendar's hue; a past event today is dimmed. Fixture only.
  import Glyph from "../../shell/AppIcon.svelte";
  import { calOf, when, type Ev } from "./data";
  let { ev, mode, day, past = false, selected = false, short = false, onopen }: { ev: Ev; mode: "row" | "card" | "cell"; day?: string; past?: boolean; selected?: boolean; /** A card too low for three lines: the title and the time on one line. */ short?: boolean; onopen?: () => void } = $props();
  const hue = $derived(calOf(ev.cal).hue), allDay = $derived(!ev.start);
  const span = $derived(ev.days.length > 1 && allDay ? (day === ev.days[0] ? "start" : day === ev.days.at(-1) ? "end" : "mid") : null);
</script>

{#if mode === "cell"}
  <button type="button" class="cell" class:all={allDay} class:past data-event={ev.id} data-span={span} style:--hue={hue} title={`${ev.title}, ${when(ev)}`} onclick={(e) => { e.stopPropagation(); onopen?.(); }}>
    {#if !allDay}<i class="dot"></i><span class="t">{ev.start}</span>{/if}{#if ev.cal === "birthdays"}<span class="gi"><Glyph name="gift" inline /></span>{/if}<span class="n">{span && span !== "start" ? "" : ev.title}</span>
  </button>
{:else if mode === "card"}
  <button type="button" class="card" class:short class:past data-event={ev.id} style:--hue={hue} onclick={onopen}>
    <span class="n">{ev.title}</span><span class="t">{when(ev)}</span>{#if ev.where}<span class="w">{ev.where}</span>{/if}
  </button>
{:else}
  <button type="button" class="row" class:past class:sel={selected} aria-current={selected ? "true" : undefined} data-event={ev.id} style:--hue={hue} onclick={onopen}>
    <i class="bar"></i><span class="t">{allDay ? "All day" : `${ev.start}–${ev.end}`}</span><span class="n">{ev.title}</span>{#if ev.where}<span class="w">{ev.where}</span>{/if}
  </button>
{/if}

<style>
  button { color: var(--ink); text-align: start; } .past { opacity: 0.55; }
  .cell { display: flex; align-items: center; gap: var(--s-2); width: 100%; min-width: 0; height: 20px; padding: 0 var(--s-2); border-radius: var(--r-select); font-size: var(--text-section); line-height: 1; transition: background-color var(--dur-fast) var(--ease-out); }
  @media (hover: hover) and (pointer: fine) { .cell:hover { background: var(--hover); } }
  .dot { flex: none; width: 6px; height: 6px; border-radius: 50%; background: oklch(0.7 0.12 var(--hue)); } .cell .t { flex: none; color: var(--ink-3); font-variant-numeric: tabular-nums; } .cell .n { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: var(--ui-weight); }
  .gi { flex: none; display: grid; color: var(--ink-2); }
  .cell.all { background: oklch(0.78 0.09 var(--hue) / 0.5); padding: 0 var(--s-3); } .cell.all[data-span="start"] { border-start-end-radius: 0; border-end-end-radius: 0; } .cell.all[data-span="mid"] { border-radius: 0; } .cell.all[data-span="end"] { border-start-start-radius: 0; border-end-start-radius: 0; }
  .card { position: relative; display: grid; align-content: start; gap: var(--s-1); width: 100%; height: 100%; box-sizing: border-box; overflow: hidden; padding: var(--s-2) var(--s-3) var(--s-2) var(--s-4); border-radius: var(--r-select); background: oklch(0.78 0.09 var(--hue) / 0.4); font-size: var(--text-label); line-height: 1.25; transition: scale var(--dur-fast) var(--ease-out); }
  .card::before { content: ""; position: absolute; inset: var(--s-1) auto var(--s-1) var(--s-1); width: 3px; border-radius: var(--r-pill); background: oklch(0.68 0.13 var(--hue)); } .card:active { scale: var(--press-scale); }
  .card.short { display: flex; align-items: center; gap: var(--s-3); padding-block: 0; } .card.short .n { display: block; flex: 0 1 auto; min-width: 0; white-space: nowrap; text-overflow: ellipsis; } .card.short .t, .card.short .w { display: none; }
  @container (max-width: 56rem) { .cell:not(.all) .t { display: none; } }
  .card .n { overflow: hidden; font-weight: var(--strong-weight); display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; } .card .t, .card .w { color: var(--ink-2); font-size: var(--text-section); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-variant-numeric: tabular-nums; }
  .row { display: flex; align-items: center; gap: var(--s-4); width: 100%; min-height: var(--row-h); padding: 0 var(--s-5); border-radius: var(--r-select); font-size: var(--text-ui); transition: background-color var(--dur-fast) var(--ease-out); } .row.sel { background: var(--accent-soft); }
  @media (hover: hover) and (pointer: fine) { .row:not(.sel):hover { background: var(--hover); } }
  .bar { flex: none; width: 3px; height: 18px; border-radius: var(--r-pill); background: oklch(0.68 0.13 var(--hue)); } .row .t { flex: none; width: 6.5rem; color: var(--ink-3); font-size: var(--text-label); font-variant-numeric: tabular-nums; } .row .n { flex: 0 1 auto; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .row .w { flex: 1 1 0; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--ink-3); font-size: var(--text-label); }
</style>

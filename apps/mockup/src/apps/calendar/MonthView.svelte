<script lang="ts">
  // A month as a grid of days, Monday first. Each day lists its events as one-line cells (three, then "+N more"); the day of the month is a number, today's is the accent's disc, days of the neighbouring months are quiet. In a narrow window the cells show only dots
  // and the selected day's events are listed under the grid (the phone's pattern). A click selects a day; Enter, a double click or "+N more" opens it as a day.
  import { calOf, onDay } from "./data";
  import { dayNum, longDate, monthGrid, TODAY, weekdayShort, add } from "./dates";
  import EventLook from "./EventLook.svelte";
  let { anchor, sel, hidden, onsel, onopen, ongo }: { anchor: string; sel: string; hidden: string[]; onsel: (d: string) => void; onopen: (id: string) => void; ongo: (d: string, from?: HTMLElement) => void } = $props();
  const weeks = $derived(monthGrid(anchor)), off = $derived(new Set(hidden)), MAX = 3;
  const heads = $derived(Array.from({ length: 7 }, (_, i) => weekdayShort(add("2026-11-16", i))));
</script>

<div class="mv" style:--rows={weeks.length}>
  <div class="hd">{#each heads as h (h)}<span>{h}</span>{/each}</div>
  <div class="gr">
    {#each weeks as w (w[0])}
      {#each w as d (d)}
        {@const evs = onDay(d, off)}
        <div class="day" class:out={d.slice(0, 7) !== anchor.slice(0, 7)} class:sel={d === sel} class:today={d === TODAY} data-day={d} role="presentation" onclick={() => onsel(d)} ondblclick={(e) => ongo(d, e.currentTarget)}>
          <button type="button" class="dn" aria-label={`${longDate(d)}${evs.length ? `, ${evs.length} events` : ""}`} aria-current={d === TODAY ? "date" : undefined} onclick={(e) => { e.stopPropagation(); onsel(d); }} ondblclick={(e) => { e.stopPropagation(); ongo(d, e.currentTarget.parentElement as HTMLElement); }} onkeydown={(e) => { if (e.key === "Enter") { e.preventDefault(); ongo(d, e.currentTarget.parentElement as HTMLElement); } }}>{dayNum(d)}</button>
          <div class="chips">{#each evs.slice(0, evs.length > MAX ? MAX - 1 : MAX) as e (e.id)}<EventLook ev={e} mode="cell" day={d} onopen={() => onopen(e.id)} />{/each}{#if evs.length > MAX}<button type="button" class="more" onclick={(e) => { e.stopPropagation(); ongo(d, e.currentTarget.closest(".day") as HTMLElement); }}>+{evs.length - (MAX - 1)} more</button>{/if}</div>
          <div class="dots" aria-hidden="true">{#each evs.slice(0, 4) as e (e.id)}<i style:--hue={calOf(e.cal).hue}></i>{/each}</div>
        </div>
      {/each}
    {/each}
  </div>
</div>

<style>
  .mv { flex: 1; min-height: 0; display: flex; flex-direction: column; } .hd { flex: none; display: grid; grid-template-columns: repeat(7, minmax(0, 1fr)); padding-bottom: var(--s-3); color: var(--ink-3); font-size: var(--text-section); text-transform: uppercase; letter-spacing: 0.05em; } .hd span { padding-inline: var(--s-3); }
  .gr { flex: 1; min-height: 0; display: grid; grid-template-columns: repeat(7, minmax(0, 1fr)); grid-template-rows: repeat(var(--rows), minmax(0, 1fr)); border-top: 1px solid rgb(var(--ink-rgb) / 0.09); }
  .day { display: flex; flex-direction: column; gap: var(--s-1); min-width: 0; min-height: 0; overflow: hidden; padding: var(--s-2) var(--s-1) var(--s-1); border-inline-start: 1px solid rgb(var(--ink-rgb) / 0.07); border-bottom: 1px solid rgb(var(--ink-rgb) / 0.07); cursor: default; transition: background-color var(--dur-fast) var(--ease-out); } .day:nth-child(7n + 1) { border-inline-start: 0; }
  @media (hover: hover) and (pointer: fine) { .day:hover { background: rgb(var(--ink-rgb) / 0.03); } } .day.sel { background: var(--accent-soft); } .day.out { color: var(--ink-3); } .day.out .chips { opacity: 0.6; }
  .dn { color: inherit; align-self: flex-start; display: grid; place-items: center; min-width: 22px; height: 22px; margin-inline-start: var(--s-2); border-radius: var(--r-menu); font-size: var(--text-label); font-variant-numeric: tabular-nums; } .today .dn { background: var(--badge-bg); color: #fff; font-weight: var(--strong-weight); }
  .chips { display: grid; gap: var(--s-1); min-height: 0; } .more { padding: 0 var(--s-2); height: 20px; text-align: start; color: var(--ink-3); font-size: var(--text-section); border-radius: var(--r-select); } @media (hover: hover) and (pointer: fine) { .more:hover { color: var(--ink); background: var(--hover); } }
  .dots { display: none; justify-content: center; gap: 3px; } .dots i { width: 6px; height: 6px; border-radius: 50%; background: oklch(0.7 0.12 var(--hue)); }
  @container (max-width: 34rem) { .chips { display: none; } .dots { display: flex; } .day { align-items: center; padding-top: var(--s-3); } .dn { margin: 0; } .hd { text-align: center; } .hd span { padding: 0; } }
</style>

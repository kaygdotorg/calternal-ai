<script lang="ts">
  // The Calendar window: a title for the span shown (a month, a week, a day), previous, Today and next, and the view (Month, Week, Day, Agenda). An open event replaces the page, with a way back that keeps the view and the day it was on. The calendars that are
  // switched off are in the right panel and apply to every view. In a narrow window Week shows three days and Month shows dots with the selected day listed under it. MOTION: changing the view or the span fades the new page in and up over the base duration on the
  // out curve (the old page leaves at once); reduced motion is instant.
  import { Button, IconButton, Selector } from "@calternal/ui";
  import AppIcon from "../../shell/AppIcon.svelte";
  import { ITEMS } from "../../data/collections";
  import type { PageProps } from "../registry";
  import { onDay } from "./data";
  import { add, addMonths, dayMonth, longDate, monthYear, relative, shortMonth, TODAY, dayNum, weekStart, weekdayShort, weekday } from "./dates";
  import EventLook from "./EventLook.svelte";
  import EventRoute from "./EventRoute.svelte";
  import MonthView from "./MonthView.svelte";
  import { calState, type Mode } from "./state.svelte";
  import TimeGrid from "./TimeGrid.svelte";
  let { win, onopen }: PageProps = $props();
  const s = calState(win.id);
  const item = $derived(win.sel ? ITEMS[win.sel] : undefined);
  let width = $state(900);
  const narrow = $derived(width < 560);
  const days = $derived(s.mode === "day" ? [s.anchor] : s.mode === "week" ? (narrow ? [0, 1, 2].map((i) => add(s.anchor, i)) : Array.from({ length: 7 }, (_, i) => add(weekStart(s.anchor), i))) : []);
  const title = $derived(s.mode === "month" ? monthYear(s.anchor) : s.mode === "day" ? (narrow ? `${weekdayShort(s.anchor)} ${dayNum(s.anchor)} ${shortMonth(s.anchor)}` : longDate(s.anchor)) : s.mode === "week" ? (narrow ? `${dayNum(days[0]!)}–${dayNum(days.at(-1)!)} ${shortMonth(days.at(-1)!)}` : `${dayNum(days[0]!)}${days[0]!.slice(0, 7) !== days.at(-1)!.slice(0, 7) ? ` ${shortMonth(days[0]!)}` : ""} – ${dayNum(days.at(-1)!)} ${shortMonth(days.at(-1)!)} ${days.at(-1)!.slice(0, 4)}`) : "Coming up");
  const step = (n: -1 | 1) => { zoom = null; s.anchor = s.mode === "month" ? addMonths(s.anchor, n) : s.mode === "week" ? add(s.anchor, n * (narrow ? 3 : 7)) : s.mode === "day" ? add(s.anchor, n) : add(s.anchor, n * 14); if (s.mode === "month" || s.mode === "day") s.sel = s.anchor; };
  const today = () => { s.anchor = TODAY; s.sel = TODAY; };
  const MODES = [{ value: "month", label: "Month" }, { value: "week", label: "Week" }, { value: "day", label: "Day" }, { value: "agenda", label: "Agenda" }];
  /** "Today", "Tomorrow", else the weekday (the date follows in quiet text). */
  const head = (d: string) => { const r = relative(d); return r === "Today" || r === "Tomorrow" || r === "Yesterday" ? r : weekday(d); };
  const open = (id: string) => onopen(ITEMS[id]!);
  /** Month to Day: the day page grows out of the cell that was chosen. */
  let page = $state<HTMLElement>(), zoom = $state<string | null>(null);
  const go = (d: string, from?: HTMLElement) => { const a = page?.getBoundingClientRect(), b = from?.getBoundingClientRect(); zoom = a && b ? `${Math.round(b.left + b.width / 2 - a.left)}px ${Math.round(b.top + b.height / 2 - a.top)}px` : null; s.anchor = d; s.sel = d; s.mode = "day"; };
  const strip = $derived(Array.from({ length: 7 }, (_, i) => add(weekStart(s.anchor), i)));
  const off = $derived(new Set(s.hidden));
  const agenda = $derived(Array.from({ length: 21 }, (_, i) => add(s.anchor, i)).map((d) => ({ d, evs: onDay(d, off) })).filter((g) => g.evs.length));
  const selDay = $derived(onDay(s.sel, off));
</script>

{#if item && item.kind === "event"}
  <EventRoute {item} {win} onback={() => (win.sel = null)} backLabel={s.mode === "month" ? monthYear(s.anchor) : "Calendar"} />
{:else}
  <div class="cp" bind:clientWidth={width} data-calendar data-mode={s.mode}>
    <header class="top">
      <h2>{title}</h2>
      <div class="nav"><IconButton icon="back" label={s.mode === "month" ? "Previous month" : s.mode === "week" ? "Previous week" : s.mode === "day" ? "Previous day" : "Earlier"} onclick={() => step(-1)} /><Button text="Today" variant="quiet" onclick={today} /><IconButton icon="chevron" label={s.mode === "month" ? "Next month" : s.mode === "week" ? "Next week" : s.mode === "day" ? "Next day" : "Later"} onclick={() => step(1)} /></div>
      <div class="sel"><Selector options={MODES} value={s.mode} label="View" size="sm" fill={false} onchange={(v: string) => { zoom = null; s.mode = v as Mode; }} /></div>
    </header>
    {#key `${s.mode}:${s.anchor.slice(0, s.mode === "month" ? 7 : 10)}`}
      <div class="pg" class:zoom={!!zoom} style:--org={zoom ?? undefined} bind:this={page}>
        {#if narrow && s.mode === "day"}<div class="wk" role="group" aria-label="Days of the week">{#each strip as d (d)}<button type="button" class="dd" class:today={d === TODAY} class:sel={d === s.anchor} aria-label={longDate(d)} aria-pressed={d === s.anchor} onclick={() => { zoom = null; s.anchor = d; s.sel = d; }}><span class="w">{weekdayShort(d)[0]}</span><span class="n">{dayNum(d)}</span><i class:on={onDay(d, off).length > 0}></i></button>{/each}</div>{/if}
        {#if s.mode === "month"}
          <MonthView anchor={s.anchor} sel={s.sel} hidden={s.hidden} onsel={(d) => (s.sel = d)} onopen={open} ongo={go} />
          {#if narrow}
            <section class="ag" aria-label={`Events on ${longDate(s.sel)}`}><h3>{head(s.sel)}<span>{dayMonth(s.sel)}</span></h3>
              {#each selDay as e (e.id)}<EventLook ev={e} mode="row" onopen={() => open(e.id)} />{:else}<p class="q">Nothing planned.</p>{/each}</section>
          {/if}
        {:else if s.mode === "agenda"}
          <div class="al">{#each agenda as g (g.d)}<section><h3>{head(g.d)}<span>{dayMonth(g.d)}</span></h3>{#each g.evs as e (e.id)}<EventLook ev={e} mode="row" onopen={() => open(e.id)} />{/each}</section>{:else}<p class="q big">Nothing planned for the next three weeks.</p>{/each}</div>
        {:else}
          <TimeGrid {days} hour={narrow ? 36 : 48} head={!(narrow && s.mode === "day")} hidden={s.hidden} onopen={open} ongo={go} empty={s.mode === "day" ? "Nothing planned. Enjoy the quiet." : "Nothing planned this week."} />
        {/if}
      </div>
    {/key}
  </div>
{/if}

<style>
  .cp { flex: 1; min-height: 0; display: flex; flex-direction: column; container-type: inline-size; } .top { flex: none; display: flex; align-items: center; flex-wrap: wrap; gap: var(--s-3) var(--s-5); padding: var(--s-2) 0 var(--s-5); }
  h2 { flex: 1 1 auto; min-width: 0; margin: 0; font-size: var(--text-title); font-weight: var(--strong-weight); line-height: 1.2; font-variant-numeric: tabular-nums; } .nav { display: flex; align-items: center; gap: var(--s-1); } .sel { flex: none; }
  .pg { flex: 1; min-height: 0; display: flex; flex-direction: column; animation: pin var(--dur-base) var(--ease-out); } @keyframes pin { from { opacity: 0; translate: 0 6px; } } .pg.zoom { transform-origin: var(--org, 50% 30%); animation-name: zin; } @keyframes zin { from { opacity: 0; scale: 0.94; } }
  .wk { flex: none; display: grid; grid-template-columns: repeat(7, 1fr); padding-bottom: var(--s-3); } .dd { display: grid; justify-items: center; align-content: start; width: 100%; min-height: 66px; gap: var(--s-1); padding: var(--s-2) 0 var(--s-3); border-radius: var(--r-menu); color: var(--ink-3); transition: background-color var(--dur-fast) var(--ease-out); } .dd .w { font-size: var(--text-section); text-transform: uppercase; } .dd .n { display: grid; place-items: center; width: 32px; height: 32px; border-radius: 50%; color: var(--ink); font-size: var(--text-heading); font-variant-numeric: tabular-nums; transition: background-color var(--dur-fast) var(--ease-out); } .dd i { width: 4px; height: 4px; border-radius: 50%; background: transparent; } .dd i.on { background: var(--ink-3); } .dd.today .w { color: var(--accent); } .dd.sel .n { background: var(--accent-soft); color: var(--accent); } .dd.today.sel .n, .dd.today .n { font-weight: var(--strong-weight); } .dd.today.sel .n { background: var(--badge-bg); color: #fff; }
  .al { flex: 1; overflow: auto; scrollbar-width: thin; display: grid; align-content: start; gap: var(--gap-before-head); max-width: 44rem; padding-bottom: var(--s-8); } h3 { display: flex; align-items: baseline; gap: var(--s-4); margin: 0 0 var(--gap-head-items); padding-inline: var(--s-5); font-size: var(--text-heading); font-weight: var(--strong-weight); line-height: 1.2; } h3 span { color: var(--ink-3); font-size: var(--text-label); font-weight: var(--ui-weight); }
  .ag { flex: none; padding-top: var(--s-5); max-height: 40%; overflow: auto; } .q { margin: 0; padding: var(--s-4) var(--s-5); color: var(--ink-3); font-size: var(--text-ui); } .q.big { padding-top: var(--s-7); }
  @container (max-width: 36rem) { h2 { font-size: var(--text-heading); } .nav { margin-inline-start: auto; } .top { gap: var(--s-3); } .sel { order: 3; width: 100%; } .sel :global(*) { max-width: 100%; } }
</style>

<script lang="ts">
  import PanelFrame from "../../shell/PanelFrame.svelte";
  // Calendar's right panel. With an event open: the calendar it is on, how its guests answered, and that day (the other events, this one marked). With no event: a month to jump by (it moves the window's view) and the calendars, each with a switch that hides or shows
  // its events in every view. The Assistant tab is the shared one with calendar prompts. Fixture only.
  import { IconButton } from "@calternal/ui";
  import AppIcon from "../../shell/AppIcon.svelte";
  import ItemRow from "../../shell/Item.svelte";
  import { ITEMS, type Item } from "../../data/collections";
  import { wins, type Win } from "../../windows.svelte";
  import { CALS, calOf, EV, onDay, rsvp } from "./data";
  import { dayNum, dayMonth, monthGrid, monthYear, addMonths, TODAY, weekdayShort, add, relative } from "./dates";
  import EventLook from "./EventLook.svelte";
  import { calState } from "./state.svelte";
  let { win, item }: { win: Win | undefined; item?: Item } = $props();
  const s = calState(win?.id ?? "none");
  const ev = $derived(item ? EV[item.id] : undefined), day = $derived(ev?.days[0] ?? TODAY);
  const answers = $derived(ev ? ev.guests.map((_, i) => rsvp(ev, i)) : []);
  const set = (id: string, on: boolean) => { s.hidden = on ? s.hidden.filter((h) => h !== id) : [...s.hidden, id]; };
</script>

<PanelFrame title={ev ? ev.title : "Calendar"}>
      {#if ev}
        <section><h3>Calendar</h3><ul>{#each CALS as c (c.id)}<li class:on={c.id === ev.cal}><i style:--hue={c.hue}></i><span>{c.name}</span>{#if c.id === ev.cal}<b>On this one</b>{/if}</li>{/each}</ul></section>
        {#if ev.guests.length && ev.cal !== "birthdays"}<section><h3>Answers</h3><dl><dt>Going</dt><dd>{answers.filter((a) => a === "going").length}</dd><dt>Maybe</dt><dd>{answers.filter((a) => a === "maybe").length}</dd><dt>Can't go</dt><dd>{answers.filter((a) => a === "declined").length}</dd><dt>No answer</dt><dd>{answers.filter((a) => a === "waiting").length}</dd></dl></section>{/if}
        <section><h3>{relative(day)}</h3><div class="day">{#each onDay(day) as e (e.id)}<EventLook ev={e} mode="row" selected={e.id === ev.id} onopen={() => wins.openItem(ITEMS[e.id]!)} />{/each}</div></section>
        <section><h3>Time</h3><dl><dt>Shows as</dt><dd>{ev.busy === false ? "Free" : "Busy"}</dd><dt>Travel</dt><dd>None</dd></dl></section>
      {:else}
        <section><h3 class="mh"><span>{monthYear(s.anchor)}</span><span class="mb"><IconButton small icon="back" label="Previous month" onclick={() => (s.anchor = addMonths(s.anchor, -1))} /><IconButton small icon="chevron" label="Next month" onclick={() => (s.anchor = addMonths(s.anchor, 1))} /></span></h3>
          <div class="mm" role="grid" aria-label="Month">{#each Array.from({ length: 7 }, (_, i) => weekdayShort(add("2026-11-16", i))[0]) as h, i (i)}<span class="h">{h}</span>{/each}
            {#each monthGrid(s.anchor).flat() as d (d)}<button type="button" class="d" class:out={d.slice(0, 7) !== s.anchor.slice(0, 7)} class:today={d === TODAY} class:sel={d === s.sel} aria-label={dayMonth(d)} onclick={() => { s.anchor = d; s.sel = d; if (s.mode === "agenda") s.mode = "day"; }}>{dayNum(d)}{#if onDay(d).length}<i></i>{/if}</button>{/each}</div>
          
        </section>
        <section><h3>Calendars</h3><ul class="cs">{#each CALS as c (c.id)}{@const on = !s.hidden.includes(c.id)}<li><button type="button" aria-pressed={on} onclick={() => set(c.id, !on)}><i style:--hue={c.hue} class:off={!on}></i><span class:off={!on}>{c.name}</span><span class="ck">{#if on}<AppIcon name="check" inline />{/if}</span></button></li>{/each}</ul></section>
      {/if}
</PanelFrame>

<style>
  .rp { display: flex; flex-direction: column; height: 100%; min-height: 0; padding: 0 var(--s-3); box-sizing: border-box; gap: var(--s-3); } header { flex: none; padding: var(--s-5) var(--s-3) 0; } h2 { margin: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: var(--text-heading); line-height: 1.2; font-weight: var(--strong-weight); }
  .bd { flex: 1; min-height: 0; overflow-y: auto; scrollbar-width: none; display: grid; align-content: start; gap: var(--gap-before-head); padding: var(--s-4) var(--s-3); } h3 { margin: 0 0 var(--gap-head-items); color: var(--ink-2); font-size: var(--text-heading); line-height: 1.2; font-weight: var(--strong-weight); }
  dl { display: grid; grid-template-columns: max-content minmax(0, 1fr); gap: var(--s-3) var(--s-6); margin: 0; font-size: var(--text-ui); line-height: 1.3; } dt { color: var(--ink-3); } dd { margin: 0; font-variant-numeric: tabular-nums; }
  ul { list-style: none; margin: 0; padding: 0; display: grid; gap: var(--s-2); font-size: var(--text-ui); } li { display: flex; align-items: center; gap: var(--s-4); min-height: 32px; } li i { flex: none; width: 10px; height: 10px; border-radius: 50%; background: oklch(0.7 0.12 var(--hue)); } li b { margin-inline-start: auto; color: var(--ink-3); font-size: var(--text-label); font-weight: var(--ui-weight); } .cs li { min-height: 0; } .cs button { display: flex; align-items: center; gap: var(--s-4); width: 100%; min-height: 32px; padding: 0 var(--s-3); margin-inline: calc(var(--s-3) * -1); box-sizing: content-box; border-radius: var(--r-select); color: var(--ink); text-align: start; font-size: var(--text-ui); } .cs i { flex: none; width: 10px; height: 10px; border-radius: 50%; background: oklch(0.7 0.12 var(--hue)); transition: background-color var(--dur-fast) var(--ease-out); } .cs i.off { background: transparent; box-shadow: inset 0 0 0 1.5px oklch(0.7 0.12 var(--hue)); } .cs .off { color: var(--ink-3); } .ck { margin-inline-start: auto; display: grid; color: var(--accent); } @media (hover: hover) and (pointer: fine) { .cs button:hover { background: var(--hover); } }
  .day { margin-inline-start: calc(var(--s-5) * -1 + var(--s-3)); display: grid; } .day :global(.t) { width: auto !important; }
  .mm { display: grid; grid-template-columns: repeat(7, 1fr); gap: 2px; text-align: center; font-size: var(--text-label); } .h { color: var(--ink-3); font-size: var(--text-section); padding-bottom: var(--s-2); } .d { position: relative; display: grid; place-items: center; height: 30px; border-radius: 50%; color: var(--ink); font-variant-numeric: tabular-nums; } .d.out { color: var(--ink-3); } .d i { position: absolute; bottom: 3px; width: 4px; height: 4px; border-radius: 50%; background: var(--ink-3); }
  .d.today { background: var(--badge-bg); color: #fff; } .d.today i { background: #fff; } .d.sel:not(.today) { background: var(--accent-soft); color: var(--accent); } @media (hover: hover) and (pointer: fine) { .d:not(.today):not(.sel):hover { background: var(--hover); } }
  .mh { display: flex; align-items: center; justify-content: space-between; } .mb { display: flex; gap: var(--s-1); }
  .ai { display: grid; gap: var(--s-4); } .b { margin: 0; padding: var(--s-4) var(--s-5); border-radius: var(--r-card); background: rgb(var(--ink-rgb) / 0.07); font-size: var(--text-ui); line-height: 1.45; } .sg { display: flex; flex-wrap: wrap; gap: var(--s-3); } .sg button { min-height: 30px; padding: 0 var(--s-5); border-radius: var(--r-pill); background: var(--accent-soft); color: var(--accent); font-size: var(--text-label); }
  .in input { width: 100%; box-sizing: border-box; min-height: 34px; padding: 0 var(--s-5); border-radius: var(--r-pill); border: 0; outline: 0; background: rgb(var(--ink-rgb) / 0.08); color: var(--ink); font: inherit; font-size: var(--text-ui); } footer { flex: none; padding-bottom: var(--s-3); }
</style>

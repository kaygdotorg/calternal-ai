<script lang="ts">
  // The hours of one to seven days: a header with each day's name and number (today in the accent), an all-day row, then a scrolling column of hours with the day's events as cards. Events that overlap share the width. Today carries a line at the mockup's
  // clock (11:20). It opens scrolled to 07:00. An empty day (or days) says so in words over the grid.
    import { calOf, onDay, type Ev } from "./data";
  import { dayNum, mins, NOW, TODAY, weekdayShort } from "./dates";
  import EventLook from "./EventLook.svelte";
  let { days, hidden, onopen, ongo, empty = "Nothing planned.", hour = 48, head = true }: { days: string[]; hidden: string[]; onopen: (id: string) => void; ongo?: (d: string) => void; empty?: string; /** Pixels per hour: 48 on a desktop, denser on a phone. */ hour?: number; /** Draw the day names and numbers (a phone's day view has its own week strip instead). */ head?: boolean } = $props();
  const H = $derived(hour), hidden_ = $derived(new Set(hidden));
  const per = $derived(days.map((d) => onDay(d, hidden_)));
  const none = $derived(per.every((l) => l.length === 0));
  /** Greedy columns: an event goes in the first column free at its start; a cluster of overlapping events shares one width. */
  function lay(evs: Ev[]): { ev: Ev; col: number; n: number }[] {
    const timed = evs.filter((e) => e.start), out: { ev: Ev; col: number; n: number }[] = []; let cluster: typeof out = [], ends: number[] = [], clusterEnd = 0;
    const close = () => { for (const c of cluster) c.n = ends.length; out.push(...cluster); cluster = []; ends = []; };
    for (const ev of timed) {
      const s = mins(ev.start!), e = mins(ev.end!); if (cluster.length && s >= clusterEnd) close();
      let col = ends.findIndex((x) => x <= s); if (col < 0) { col = ends.length; ends.push(e); } else ends[col] = e;
      cluster.push({ ev, col, n: 1 }); clusterEnd = Math.max(clusterEnd, e);
    }
    close(); return out;
  }
  let sc = $state<HTMLElement>();
  /** Opens at 07:00 (a little above, so its label shows) and again if the hour height changes (a window becoming narrow). */
  $effect(() => { if (sc) sc.scrollTop = 7 * H - 10; });
  const nowTop = $derived(mins(NOW) / 60 * H);
</script>

<div class="tg" style:--cols={days.length} style:--h="{H}px">
  {#if head}<div class="head"><span class="gut"></span>
    {#each days as d (d)}<button type="button" class="dh" class:today={d === TODAY} aria-label={`${weekdayShort(d)} ${dayNum(d)}`} onclick={() => ongo?.(d)}><span class="wd">{weekdayShort(d)}</span><span class="dn">{dayNum(d)}</span></button>{/each}
  </div>{/if}
  {#if per.some((l) => l.some((e) => !e.start))}
    <div class="all"><span class="gut"><span class="gl">all-day</span></span>{#each days as d, i (d)}<div class="ac">{#each per[i]!.filter((e) => !e.start) as e (e.id)}<EventLook ev={e} mode="cell" day={d} onopen={() => onopen(e.id)} />{/each}</div>{/each}</div>
  {/if}
  <div class="sc" bind:this={sc}>
    <div class="bd" style:height="{24 * H}px">
      <div class="hrs">{#each Array.from({ length: 24 }, (_, i) => i) as h (h)}<div class="hr" style:top="{h * H}px"><span class="gut"><span class="hl">{h === 0 ? "" : `${String(h).padStart(2, "0")}:00`}</span></span><i></i></div>{/each}</div>
      <div class="cols">
        <span class="gut"></span>
        {#each days as d, i (d)}
          <div class="col" class:today={d === TODAY}>
            {#each lay(per[i]!) as { ev, col, n } (ev.id)}
              <div class="ev" style:top="{mins(ev.start!) / 60 * H + 1}px" style:height="{Math.max(20, (mins(ev.end!) - mins(ev.start!)) / 60 * H - 2)}px" style:left="calc(100% / {n} * {col})" style:width="calc(100% / {n} - 2px)"><EventLook {ev} mode="card" short={(mins(ev.end!) - mins(ev.start!)) / 60 * H < 44} past={d === TODAY && mins(ev.end!) < mins(NOW)} onopen={() => onopen(ev.id)} /></div>
            {/each}
            {#if d === TODAY}<span class="now" style:top="{nowTop}px" aria-label={`Now, ${NOW}`}></span>{/if}
          </div>
        {/each}
      </div>
      {#if none}<p class="none">{empty}</p>{/if}
    </div>
  </div>
</div>

<style>
  .tg { flex: 1; min-height: 0; display: flex; flex-direction: column; --gw: 3.25rem; } .gut { flex: none; width: var(--gw); }
  .head, .all { flex: none; display: grid; grid-template-columns: var(--gw) repeat(var(--cols), minmax(0, 1fr)); } .head { padding-bottom: var(--s-3); }
  .dh { display: grid; justify-items: center; gap: var(--s-1); padding: var(--s-2) 0; border-radius: var(--r-select); color: var(--ink-3); } .wd { font-size: var(--text-section); text-transform: uppercase; letter-spacing: 0.05em; } .dn { display: grid; place-items: center; min-width: 28px; height: 28px; border-radius: 14px; color: var(--ink); font-size: var(--text-heading); font-variant-numeric: tabular-nums; }
  .dh.today .wd { color: var(--accent); } .dh.today .dn { background: var(--badge-bg); color: #fff; font-weight: var(--strong-weight); } @media (hover: hover) and (pointer: fine) { .dh:hover .dn:not(:where(.today *)) { background: var(--hover); } }
  .all { padding-bottom: var(--s-3); border-bottom: 1px solid rgb(var(--ink-rgb) / 0.09); } .gl, .hl { display: block; padding-inline-end: var(--s-4); text-align: end; color: var(--ink-3); font-size: var(--text-section); font-variant-numeric: tabular-nums; } .gl { padding-top: 3px; } .ac { display: grid; gap: var(--s-1); align-content: start; padding-inline: 1px; }
  .sc { flex: 1; min-height: 0; overflow-y: auto; scrollbar-width: thin; } .bd { position: relative; }
  .hrs { position: absolute; inset: 0; pointer-events: none; } .hr { position: absolute; left: 0; right: 0; display: flex; align-items: center; height: 0; } .hr .gut { display: block; } .hr i { flex: 1; height: 1px; background: rgb(var(--ink-rgb) / 0.08); }
  .cols { position: absolute; inset: 0; display: grid; grid-template-columns: var(--gw) repeat(var(--cols), minmax(0, 1fr)); } .col { position: relative; border-inline-start: 1px solid rgb(var(--ink-rgb) / 0.08); } .col.today { background: rgb(var(--ink-rgb) / 0.03); }
  .ev { position: absolute; } .now { position: absolute; left: -1px; right: 0; height: 2px; margin-top: -1px; background: var(--accent-base); pointer-events: none; } .now::before { content: ""; position: absolute; left: -4px; top: -3px; width: 8px; height: 8px; border-radius: 50%; background: var(--accent-base); }
  .none { position: absolute; left: var(--gw); right: 0; top: calc(7 * var(--h) + 4rem); margin: 0; text-align: center; color: var(--ink-3); font-size: var(--text-ui); pointer-events: none; }
</style>

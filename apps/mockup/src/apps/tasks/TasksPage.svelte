<script lang="ts">
  // The Tasks window: a list page. A list switcher (Today, Upcoming, Anytime, Completed), a project filter, and the list in a view (List with sections and inline checkboxes, Board by status, Table with the task's cells). The sidebar's lists are the same lists, opened here.
  // MOTION. Switching list or view: the old content goes at once and the new rises 6 px while fading in (`--dur-base`, `--ease-out`), the way every view change in the mockup does; ticking is the check's and the row's (they do not move). Reduced motion: instant (the tokens are 1 ms).
  // A ticked task stays where it is until the list is left (the old app's rule): `snap` remembers which tasks were done when the list was entered.
  import { Selector } from "@calternal/ui";
  import Empty from "./Empty.svelte";
  import Glyph from "../../shell/AppIcon.svelte";
  import TaskRow from "./TaskRow.svelte";
  import TaskCard from "./TaskCard.svelte";
  import TaskCell from "./TaskCell.svelte";
  import { dayLong, dayName, diff, lists, PROJECTS, tasks, xOf, TODAY } from "./data";
  import { arr } from "../../arrange.svelte";
  import { wins } from "../../windows.svelte";
  import type { PageProps } from "../registry";
  import type { Item } from "../../data/collections";
  let { onopen }: PageProps = $props();
  type L = "today" | "upcoming" | "anytime" | "done";
  let list = $state<L>("today"), view = $state<"list" | "board" | "table">("list"), proj = $state<string | null>(null), snap = $state(new Set<string>()), draft = $state("");
  const enter = () => (snap = new Set(tasks.filter((t) => (arr.statusOf(t.id) ?? t.status) === "Done").map((t) => t.id)));
  enter();
  const LISTS = [{ value: "today", label: "Today" }, { value: "upcoming", label: "Upcoming" }, { value: "anytime", label: "Anytime" }, { value: "done", label: "Completed" }];
  const VIEWS = [{ value: "list", label: "List", iconOnly: true }, { value: "board", label: "Board", iconOnly: true }, { value: "table", label: "Table", iconOnly: true }];
  const pick = <T,>(set: (v: T) => void) => (v: T) => { set(v); enter(); };
  const base = (id: L) => id === "done" ? tasks.filter((t) => snap.has(t.id)) : lists[id]().filter((t) => !snap.has(t.id));
  const here = $derived(base(list).filter((t) => !proj || t.sub === proj).sort((a, b) => (xOf(a.id).when ?? "9").localeCompare(xOf(b.id).when ?? "9") || (xOf(b.id).pri ?? 0) - (xOf(a.id).pri ?? 0)));
  const open = $derived(here.filter((t) => (arr.statusOf(t.id) ?? t.status) !== "Done").length);
  interface G { key: string; title: string; hint?: string; late?: boolean; rows: Item[] }
  const groups = $derived.by((): G[] => {
    const by = (f: (t: Item) => string, name: (k: string) => Omit<G, "key" | "rows">): G[] => { const m = new Map<string, Item[]>(); for (const t of here) { const k = f(t); m.set(k, [...(m.get(k) ?? []), t]); } return [...m].map(([key, rows]) => ({ key, rows, ...name(key) })); };
    if (list === "today") return by((t) => (diff(xOf(t.id).when!) < 0 ? "late" : "today"), (k) => (k === "late" ? { title: "Overdue", late: true } : { title: "Today", hint: dayLong(TODAY) }));
    if (list === "upcoming") return by((t) => xOf(t.id).when!, (k) => ({ title: dayName(k) === k ? k : dayLong(k), hint: dayName(k) === "Tomorrow" ? "Tomorrow" : undefined }));
    if (list === "anytime") return by((t) => t.sub ?? "", (k) => ({ title: k }));
    return [{ key: "done", title: "Completed", rows: here }];
  });
  const COLS = ["To do", "Doing", "Done"] as const;
  const status = (t: Item) => arr.statusOf(t.id) ?? t.status ?? "To do";
  const heading = $derived({ today: "Today", upcoming: "Upcoming", anytime: "Anytime", done: "Completed" }[list]);
  const sel = $derived(wins.selected);
  const EMPTY = $derived(proj ? { icon: "check", title: `Nothing in ${proj}`, text: "No open tasks here. Add one, or choose another project." } : list === "done" ? { icon: "check", title: "Nothing completed yet", text: "Tasks you tick off are kept here, struck through, so you can see what got done." } : { icon: "sun", title: "All clear", text: "Nothing is planned. Add a task, or enjoy it." });
  const empty = $derived(here.length === 0);
</script>

<div class="tkp" data-tasks-page>
  <header class="hd">
    <div class="hl"><h2>{heading}</h2><span class="n" aria-label={`${open} open`}>{open}</span></div>
    <div class="ctl">
      <Selector options={LISTS} value={list} label="List" size="sm" fill={false} onchange={pick((v: L) => (list = v))} />
      <Selector options={VIEWS} value={view} label="View" size="sm" fill={false} onchange={(v: typeof view) => (view = v)}>{#snippet optionIcon(o)}<Glyph name={String(o.value)} inline />{/snippet}</Selector>
    </div>
  </header>
  <div class="pf" role="group" aria-label="Project">
    <button type="button" class="pc" class:on={!proj} aria-pressed={!proj} onclick={() => { proj = null; enter(); }}>All</button>
    {#each PROJECTS as p (p.id)}<button type="button" class="pc" class:on={proj === p.name} aria-pressed={proj === p.name} onclick={() => { proj = proj === p.name ? null : p.name; enter(); }}><i style:--hue={p.hue}></i>{p.name}</button>{/each}
  </div>
  {#key `${list}${view}${proj}`}
    <div class="bd" data-view={view}>
      {#if view === "list"}
        <form class="add" onsubmit={(e) => { e.preventDefault(); draft = ""; }}><span class="pl"><Glyph name="plus" inline /></span><input bind:value={draft} placeholder="Add a task" aria-label="Add a task" /></form>
      {/if}
      {#if empty}<Empty {...EMPTY} action="New task" />
      {:else if view === "list"}
        {#each groups as g (g.key)}
          <section class="gp"><h3 class:late={g.late}>{g.title}<span class="gc">{g.rows.length}</span>{#if g.hint}<span class="gh">{g.hint}</span>{/if}</h3>
            {#each g.rows as t (t.id)}<TaskRow item={t} selected={sel === t.id} project={list !== "anytime" && !proj} today={(list === "today" || list === "upcoming") && !g.late} {onopen} />{/each}</section>
        {/each}
      {:else if view === "board"}
        <div class="board">{#each COLS as c (c)}{@const col = here.filter((t) => status(t) === c)}
          <section class="col"><h3>{c}<span class="gc">{col.length}</span></h3><div class="cards">{#each col as t (t.id)}<TaskCard item={t} {onopen} />{:else}<p class="none">Nothing here</p>{/each}</div></section>{/each}</div>
      {:else}
        <div class="tbl" role="table" aria-label="Tasks">
          <div class="th" role="row">{#each ["Task", "Status", "When", "Project", "Priority"] as h (h)}<span role="columnheader" class="h-{h.toLowerCase()}">{h}</span>{/each}</div>
          {#each here as t (t.id)}<div class="tb" role="row" class:sel={sel === t.id}>
            <span role="cell" class="c-title"><TaskCell item={t} col="title" /></span><span role="cell" class="c-status"><TaskCell item={t} col="status" /></span><span role="cell" class="c-when"><TaskCell item={t} col="when" /></span>
            <span role="cell" class="c-project"><TaskCell item={t} col="project" /></span><span role="cell" class="c-priority"><TaskCell item={t} col="priority" /></span>
            <button type="button" class="ov" aria-label={`Open: ${t.title}`} onclick={() => onopen(t)}></button></div>{/each}
        </div>
      {/if}
    </div>
  {/key}
</div>

<style>
  .tkp { flex: none; width: 100%; display: flex; flex-direction: column; container-type: inline-size; }
  .hd { flex: none; display: flex; align-items: center; justify-content: space-between; gap: var(--s-4) var(--s-6); flex-wrap: wrap; padding: var(--s-2) 0 var(--s-4); }
  .hl { display: flex; align-items: baseline; gap: var(--s-4); } h2 { margin: 0; font-size: var(--text-title); font-weight: var(--strong-weight); line-height: 1.2; } .n { color: var(--ink-3); font-size: var(--text-heading); font-variant-numeric: tabular-nums; }
  .ctl { display: flex; align-items: center; gap: var(--s-4); min-width: 0; max-width: 100%; overflow-x: auto; scrollbar-width: none; }
  .pf { flex: none; display: flex; gap: var(--s-3); overflow-x: auto; scrollbar-width: none; padding: 0 0 var(--s-4); }
  .pc { flex: none; display: inline-flex; align-items: center; gap: var(--s-3); height: var(--s-8); padding: 0 var(--s-5); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.06); color: var(--ink-2); font-size: var(--text-label); transition: background-color var(--dur-fast) var(--ease-out), color var(--dur-fast) var(--ease-out); }
  @media (hover: hover) and (pointer: fine) { .pc:not(.on):hover { background: rgb(var(--ink-rgb) / 0.1); } }
  .pc.on { background: var(--accent-soft); color: var(--accent); font-weight: var(--strong-weight); } .pc i { width: var(--s-4); height: var(--s-4); border-radius: 50%; background: oklch(0.72 0.1 var(--hue)); }
  .bd { flex: none; /* the window scrolls the whole page */ padding-bottom: var(--s-8); animation: vin var(--dur-base) var(--ease-out); } @keyframes vin { from { opacity: 0; translate: 0 6px; } }
  .add { display: flex; align-items: center; gap: var(--s-4); min-height: var(--row-h); padding: 0 var(--s-5); margin-bottom: var(--s-2); color: var(--ink-3); } .pl { display: grid; width: var(--glyph); place-items: center; }
  .add input { flex: 1; min-width: 0; border: 0; outline: 0; background: transparent; color: var(--ink); font: inherit; font-size: var(--text-ui); } .add input::placeholder { color: var(--ink-3); }
  .gp { margin-top: var(--gap-before-head); } .gp:first-of-type { margin-top: var(--s-4); }
  h3 { display: flex; align-items: baseline; gap: var(--s-3); margin: 0; padding: 0 var(--s-5) var(--gap-head-items); color: var(--ink-2); font-size: var(--text-label); font-weight: var(--strong-weight); line-height: 1.3; } h3.late { color: color-mix(in oklab, var(--danger) 75%, var(--ink)); } :global(html[data-scheme="dark"]) h3.late { color: color-mix(in oklab, var(--danger) 60%, white); }
  .gc { color: var(--ink-3); font-weight: var(--ui-weight); font-variant-numeric: tabular-nums; } .gh { margin-inline-start: auto; color: var(--ink-3); font-weight: var(--ui-weight); }
  .board { display: grid; grid-template-columns: repeat(3, minmax(13rem, 1fr)); gap: var(--s-5); align-items: start; overflow-x: auto; padding-bottom: var(--s-4); }
  .col { display: grid; gap: var(--s-3); align-content: start; min-width: 0; } .col h3 { padding-inline: var(--s-2); } .cards { display: grid; gap: var(--s-4); } .none { margin: 0; padding: var(--s-5); color: var(--ink-3); font-size: var(--text-label); text-align: center; border-radius: var(--r-menu); background: rgb(var(--ink-rgb) / 0.03); }
  @container (max-width: 40rem) { .board { grid-template-columns: repeat(3, 15rem); } }
  .tbl { display: grid; font-size: var(--text-ui); } .th, .tb { position: relative; display: grid; grid-template-columns: minmax(12rem, 3fr) 7.5rem 7rem 9.5rem 6.5rem; align-items: center; gap: var(--s-5); padding: 0 var(--s-5); min-height: var(--row-h); border-radius: var(--r-select); }
  .th { color: var(--ink-3); font-size: var(--text-section); font-weight: var(--strong-weight); text-transform: uppercase; letter-spacing: 0.05em; min-height: var(--s-9); border-bottom: 1px solid var(--line); border-radius: 0; margin-bottom: var(--s-2); }
  .tb { transition: background-color var(--dur-fast) var(--ease-out); } .tb.sel { background: var(--accent-soft); } @media (hover: hover) and (pointer: fine) { .tb:not(.sel):hover { background: var(--hover); } }
  .tb [role="cell"] { min-width: 0; position: relative; z-index: 1; pointer-events: none; } .tb :global(.ck) { pointer-events: auto; } .ov { position: absolute; inset: 0; border-radius: inherit; }
  @container (max-width: 46rem) { .th, .tb { grid-template-columns: minmax(10rem, 1fr) 7rem 6.5rem; } .h-project, .h-priority, .c-project, .c-priority { display: none; } }
  @container (max-width: 28rem) { .th, .tb { grid-template-columns: minmax(0, 1fr) 6.5rem; } .h-status, .c-status { display: none; } }
</style>

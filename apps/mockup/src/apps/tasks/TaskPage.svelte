<script lang="ts">
  // One task as a page: the checkbox and the title (large), the properties as a row of pills (an unset one shows only its glyph, a set one its value, as the old app did), the description (the task's note, so it is a page like any other), the subtasks as a checklist, and the log.
  // It is the same page whether it opens beside a list or fills a phone. Ticking a subtask or the task is the checkbox's own motion. Properties are shown, not edited: the mockup has no pickers (the old app's are the reference).
  import Check from "./Check.svelte";
  import Glyph from "../../shell/AppIcon.svelte";
  import NoteChip from "../notes/NoteChip.svelte";
  import { arr } from "../../arrange.svelte";
  import { PRIORITY, dayName, diff, projectOf, xOf } from "./data";
  import type { Item } from "../../data/collections";
  let { item }: { item: Item } = $props();
  const x = $derived(xOf(item.id)), st = $derived(arr.statusOf(item.id) ?? item.status), done = $derived(st === "Done"), pr = $derived(projectOf(x.project));
  let ticked = $state<Record<string, boolean>>({});
  const subDone = (id: string, i: number, v: boolean) => ticked[`${id}${i}`] ?? v;
  const late = $derived(!done && !!x.when && diff(x.when) < 0);
  const PILLS = $derived([
    { id: "when", label: "When", icon: "calendar", v: x.when ? `${dayName(x.when)}${x.time ? ` ${x.time}` : ""}` : "", late },
    { id: "deadline", label: "Deadline", icon: "clock", v: x.deadline ? dayName(x.deadline) : "" }, { id: "repeat", label: "Repeat", icon: "repeat", v: x.repeat ?? "" },
    { id: "priority", label: "Priority", icon: "flag", v: x.pri ? PRIORITY[x.pri] : "" }, { id: "tags", label: "Tags", icon: "tag", v: x.tags.length ? x.tags.map((t) => `#${t}`).join(" ") : "" },
  ]);
</script>

<article class="tk" data-task-page={item.id}>
  <header class="hd"><Check id={item.id} label={item.title} status={st} /><h2 class:done>{item.title}</h2></header>
  <p class="pj"><i style:--hue={pr.hue}></i>{x.project}{#if x.waiting}<span class="w"><Glyph name="waiting" inline />Waiting on {x.waiting}</span>{/if}</p>
  <div class="props" role="group" aria-label="Properties">
    {#each PILLS as p (p.id)}<button type="button" class="pill" class:set={!!p.v} class:late={p.late} aria-label={p.v ? `${p.label}: ${p.v}` : p.label}><Glyph name={p.icon} inline />{#if p.v}<span>{p.v}</span>{/if}</button>{/each}
  </div>
  {#if x.desc}<section class="desc"><p>{x.desc}</p></section>{:else}<p class="ph">Add a description…</p>{/if}
  {#if x.subs}
    <section class="subs"><h3>Subtasks <span>{x.subs.filter((s, i) => subDone(item.id, i, s[1])).length}/{x.subs.length}</span></h3>
      {#each x.subs as s, i (s[0])}{@const on = subDone(item.id, i, s[1])}
        <div class="sr"><button type="button" class="ck" role="checkbox" aria-checked={on} aria-label={`Done: ${s[0]}`} onclick={() => (ticked[`${item.id}${i}`] = !on)}><span class="bx" class:on><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M6 12.5l4 4 8-9" pathLength="1" /></svg></span></button><span class="st" class:on>{s[0]}</span></div>{/each}
    </section>
  {/if}
  {#if x.note}<section class="from"><h3>From</h3><NoteChip id={x.note} /></section>{/if}
  {#if x.log}<section class="log"><h3>History</h3><ul>{#each x.log as l (l)}<li>{l}</li>{/each}</ul></section>{/if}
</article>

<style>
  .tk { flex: 1; min-height: 0; overflow: auto; display: grid; align-content: start; gap: var(--s-5); max-width: 40rem; width: 100%; margin-inline: auto; padding: var(--s-2) 0 var(--s-9); box-sizing: border-box; container-type: inline-size; }
  .hd { display: flex; align-items: flex-start; gap: var(--s-4); } .hd :global(.ck) { margin-block: calc((var(--row-h) - var(--text-title) * 1.25) / 2 * -1 + var(--s-1)); } .hd :global(.box) { width: calc(var(--glyph) * 1.2); height: calc(var(--glyph) * 1.2); }
  h2 { margin: 0; font-size: var(--text-title); font-weight: var(--strong-weight); line-height: 1.25; text-wrap: balance; } h2.done { color: var(--ink-3); text-decoration: line-through; text-decoration-color: var(--ink-4); }
  .pj { margin: calc(var(--s-3) * -1) 0 0; display: flex; align-items: center; gap: var(--s-3); color: var(--ink-2); font-size: var(--text-label); } .pj i { width: var(--s-4); height: var(--s-4); border-radius: 50%; background: oklch(0.72 0.1 var(--hue)); } .w { display: inline-flex; align-items: center; gap: var(--s-2); margin-inline-start: var(--s-4); }
  .props { display: flex; flex-wrap: wrap; gap: var(--s-3); }
  .pill { display: inline-flex; align-items: center; gap: var(--s-3); min-height: var(--s-9); padding: 0 var(--s-5); box-sizing: border-box; border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.06); color: var(--ink-3); font-size: var(--text-ui); transition: background-color var(--dur-fast) var(--ease-out), scale var(--dur-fast) var(--ease-out); } .pill:active { scale: var(--press-scale); }
  .pill.set { color: var(--ink); background: rgb(var(--ink-rgb) / 0.08); } .pill.late { background: color-mix(in oklab, var(--danger) 13%, transparent); color: var(--danger); } :global(html[data-scheme="dark"]) .pill.late { color: color-mix(in oklab, var(--danger) 60%, white); }
  @media (hover: hover) and (pointer: fine) { .pill:hover { background: rgb(var(--ink-rgb) / 0.11); } }
  .desc p, .ph { margin: 0; font-size: var(--text-ui); line-height: 1.6; font-weight: var(--body-weight); } .ph { color: var(--ink-3); }
  h3 { margin: 0 0 var(--gap-head-items); color: var(--ink-2); font-size: var(--text-label); font-weight: var(--strong-weight); } h3 span { color: var(--ink-3); font-weight: var(--ui-weight); font-variant-numeric: tabular-nums; margin-inline-start: var(--s-2); }
  .sr { display: flex; align-items: center; gap: var(--s-4); min-height: var(--row-h); border-bottom: 1px solid var(--line); font-size: var(--text-ui); } .sr:last-child { border-bottom: 0; }
  .ck { flex: none; display: grid; place-items: center; width: var(--row-h); min-height: var(--row-h); margin-inline: calc(var(--s-3) * -1) calc(var(--s-4) * -1); color: #fff; }
  .bx { display: grid; place-items: center; width: var(--glyph); height: var(--glyph); box-sizing: border-box; border-radius: 50%; border: 2px solid var(--ink-4); transition: background-color var(--dur-fast) var(--ease-out), border-color var(--dur-fast) var(--ease-out); } .bx.on { background: var(--badge-bg); border-color: var(--badge-bg); }
  .bx svg { width: 72%; height: 72%; stroke-dasharray: 1; stroke-dashoffset: 1; transition: stroke-dashoffset var(--dur-base) var(--ease-out); } .bx.on svg { stroke-dashoffset: 0; }
  .st { background: linear-gradient(currentColor, currentColor) no-repeat 0 58% / 0 1px; transition: background-size var(--dur-base) var(--ease-out) var(--dur-fast), color var(--dur-base) var(--ease-out) var(--dur-fast); } .st.on { color: var(--ink-3); background-size: 100% 1px; }
  .log ul { margin: 0; padding: 0; list-style: none; display: grid; gap: var(--s-2); color: var(--ink-3); font-size: var(--text-label); }
</style>

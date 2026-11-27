<script lang="ts">
  import PanelFrame from "../../shell/PanelFrame.svelte";
  // The Tasks right panel, for the open window. With a task: its properties as rows, where it came from, and its log. With the Tasks window itself: how the lists stand and each project's progress. Flat on the background like the shell's panel (it is the panel's own content, drawn with its tokens).
  import Glyph from "../../shell/AppIcon.svelte";
  import NoteChip from "../notes/NoteChip.svelte";
  import Av from "../../shell/Av.svelte";
  import { PRIORITY, PROJECTS, dayName, lists, tasks, xOf } from "./data";
  import { arr } from "../../arrange.svelte";
  import type { Win } from "../../windows.svelte";
  import type { Item } from "../../data/collections";
  let { item }: { win: Win | undefined; item?: Item } = $props();
  const x = $derived(item ? xOf(item.id) : undefined);
  const rows = $derived.by((): [string, string][] => item && x ? [["Status", arr.statusOf(item.id) ?? item.status ?? "To do"], ["When", x.when ? `${dayName(x.when)}${x.time ? ` ${x.time}` : ""}` : "Anytime"], ["Deadline", x.deadline ? dayName(x.deadline) : "None"], ["Repeat", x.repeat ?? "Never"], ["Priority", x.pri ? PRIORITY[x.pri] : "None"], ["Project", x.project], ["Tags", x.tags.length ? x.tags.map((t) => `#${t}`).join(", ") : "None"]] : []);
  const stat = $derived([["Today", lists.today().length], ["Upcoming", lists.upcoming().length], ["Anytime", lists.anytime().length], ["Completed", lists.done().length]] as const);
</script>

<PanelFrame title={item ? item.title : "Tasks"} suggestions={["What is overdue?", "Plan my week", "Split this task"]}>
  {#if item && x}
      <section><h3>Properties</h3><dl>{#each rows as [k, v] (k)}<dt>{k}</dt><dd title={v} class:none={v === "None" || v === "Never" || v === "Anytime"}>{v}</dd>{/each}</dl></section>
      {#if item.owner}<section><h3>For</h3><ul><li><Av name={item.owner} size="var(--s-8)" /><span class="pn">{item.owner}</span><span class="rl">Assigned</span></li></ul></section>{/if}
      {#if x.note}<section><h3>From the note</h3><NoteChip id={x.note} /></section>{/if}
      {#if x.waiting}<section><h3>Waiting</h3><p class="w"><Glyph name="waiting" inline />Hotel Alfama has not answered since Wednesday.</p></section>{/if}
      <section><h3>Log</h3><ul class="act">{#each x.log ?? ["You created this · Mon"] as l (l)}<li>{l}</li>{/each}</ul></section>
    {:else}
      <section><h3>Lists</h3><dl class="big">{#each stat as [k, n] (k)}<dt>{k}</dt><dd>{n}</dd>{/each}</dl></section>
      <section><h3>Projects</h3><ul class="pr">{#each PROJECTS as p (p.id)}{@const all = tasks.filter((t) => t.sub === p.name)}{@const dn = all.filter((t) => (arr.statusOf(t.id) ?? t.status) === "Done").length}
        <li><i style:--hue={p.hue}></i><span class="pn">{p.name}</span><span class="rl">{all.length ? `${dn}/${all.length}` : "Empty"}</span><span class="bar" aria-hidden="true"><b style:width={all.length ? `${(dn / all.length) * 100}%` : "0"}></b></span></li>{/each}</ul></section>
  {/if}
</PanelFrame>

<style>
  .rp { display: flex; flex-direction: column; height: 100%; min-height: 0; padding: 0 var(--s-3); box-sizing: border-box; gap: var(--s-3); } header { flex: none; padding: var(--s-5) var(--s-3) 0; } h2 { margin: 0; font-size: var(--text-title); line-height: 1.2; font-weight: var(--strong-weight); }
  .bd { flex: 1; min-height: 0; overflow-y: auto; scrollbar-width: none; display: grid; align-content: start; gap: var(--gap-before-head); padding: var(--s-4) var(--s-3); } h3 { margin: 0 0 var(--gap-head-items); color: var(--ink-2); font-size: var(--text-heading); line-height: 1.2; font-weight: var(--strong-weight); }
  dl { display: grid; grid-template-columns: max-content minmax(0, 1fr); gap: var(--s-3) var(--s-6); margin: 0; font-size: var(--text-ui); line-height: 1.3; } dt { color: var(--ink-3); } dd { margin: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } dd.none { color: var(--ink-3); } .big dd { font-variant-numeric: tabular-nums; text-align: end; }
  ul { list-style: none; margin: 0; padding: 0; display: grid; gap: var(--s-3); font-size: var(--text-ui); } li { display: flex; align-items: center; gap: var(--s-4); min-height: var(--s-9); } .pn { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } .rl { color: var(--ink-3); font-size: var(--text-label); font-variant-numeric: tabular-nums; } .act li { min-height: 0; color: var(--ink-3); font-size: var(--text-label); }
  .pr li { position: relative; flex-wrap: wrap; row-gap: var(--s-2); } .pr i { width: var(--s-4); height: var(--s-4); border-radius: 50%; background: oklch(0.72 0.1 var(--hue)); } .bar { flex: 0 0 100%; height: var(--s-2); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.08); overflow: hidden; margin: calc(var(--s-3) * -1) 0 var(--s-2) calc(var(--s-4) + var(--s-4)); width: calc(100% - var(--s-4) * 2); } .bar b { display: block; height: 100%; border-radius: inherit; background: var(--accent-base); transition: width var(--dur-slow) var(--ease-out); }
  .w { margin: 0; display: flex; gap: var(--s-3); color: var(--ink-2); font-size: var(--text-ui); line-height: 1.4; }
  .ai { display: grid; gap: var(--s-3); } .b { margin: 0; padding: var(--s-4) var(--s-5); border-radius: var(--r-card); background: rgb(var(--ink-rgb) / 0.07); font-size: var(--text-ui); line-height: 1.45; } .b.me { justify-self: end; background: var(--accent-soft); }
  .in input { width: 100%; box-sizing: border-box; min-height: var(--s-10); padding: 0 var(--s-5); border-radius: var(--r-pill); border: 0; outline: 0; background: rgb(var(--ink-rgb) / 0.08); color: var(--ink); font: inherit; font-size: var(--text-ui); } footer { flex: none; padding: 0 var(--s-10) var(--s-3) 0; }
</style>

<script lang="ts">
  // A task as the cells of a table row, one component per column: the title (checkbox and text, one line), status (a badge: words, never colour alone), day, project, priority, tags, owner. A table of mixed kinds draws a task's cells through here.
  import { Badge } from "@calternal/ui";
  import Check from "./Check.svelte";
  import Chips from "./Chips.svelte";
  import Glyph from "../../shell/AppIcon.svelte";
  import Av from "../../shell/Av.svelte";
  import { arr } from "../../arrange.svelte";
  import { PRIORITY, projectOf, xOf } from "./data";
  import type { Item } from "../../data/collections";
  let { item, col }: { item: Item; col: "title" | "status" | "when" | "project" | "priority" | "tags" | "owner" } = $props();
  const st = $derived(arr.statusOf(item.id) ?? item.status), done = $derived(st === "Done"), x = $derived(xOf(item.id)), pr = $derived(projectOf(x.project));
</script>

{#if col === "title"}<span class="c title"><Check id={item.id} label={item.title} status={st} /><span class="t" class:done>{item.title}</span></span>
{:else if col === "status"}<span class="c"><Badge text={st ?? "To do"} tone={done ? "good" : st === "Doing" ? "accent" : "neutral"} dot /></span>
{:else if col === "when"}<span class="c">{#if x.when}<Chips id={item.id} {done} show={["when"]} />{:else}<span class="none">Anytime</span>{/if}</span>
{:else if col === "project"}<span class="c pj"><i style:--hue={pr.hue}></i>{x.project}</span>
{:else if col === "priority"}<span class="c">{#if x.pri}<span class="pri p{x.pri}"><Glyph name="flag" inline />{PRIORITY[x.pri]}</span>{:else}<span class="none">None</span>{/if}</span>
{:else if col === "tags"}<span class="c"><Chips id={item.id} show={["tags"]} /></span>
{:else}<span class="c">{#if item.owner}<Av name={item.owner} size="var(--s-8)" /><span class="ow">{item.owner}</span>{/if}</span>{/if}

<style>
  .c { display: inline-flex; align-items: center; gap: var(--s-3); min-width: 0; font-size: var(--text-ui); } .title { gap: var(--s-4); } .t { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } .t.done { color: var(--ink-3); text-decoration: line-through; }
  .none, .ow { color: var(--ink-3); font-size: var(--text-label); } .pj i { width: var(--s-4); height: var(--s-4); border-radius: 50%; background: oklch(0.72 0.1 var(--hue)); flex: none; } .ow { color: var(--ink-2); }
  .pri { display: inline-flex; align-items: center; gap: var(--s-2); font-size: var(--text-label); color: var(--ink-2); } .p4 { color: var(--danger); } .p3 { color: var(--warn); }
</style>

<script lang="ts">
  // The chips of a task: only what is set, our own glyphs, never emoji. Each chip says its value in words as well as colour (overdue says "Yesterday", a priority says "High").
  // STRICT ORDER, always: when, priority, deadline, repeat, subtasks, waiting, tags. `cap` is how many show (a row 3, a narrower window fewer; no cap shows all); the rest are one "+n" chip whose name lists them. `show` picks which kinds are drawn at all (a table's cell shows one kind).
  // Under a day's own heading (`today`) the day is no news: only a time shows.
  import Glyph from "../../shell/AppIcon.svelte";
  import { PRIORITY, dayName, diff, xOf } from "./data";
  const ALL = ["when", "priority", "deadline", "repeat", "subs", "waiting", "tags"];
  let { id, show = ALL, done = false, today = false, cap = Infinity, more = true }: { id: string; show?: string[]; done?: boolean; today?: boolean; cap?: number; /** Draw the "+n" for what is hidden (off where it would say nothing, as in a narrow sidebar row). */ more?: boolean } = $props();
  const x = $derived(xOf(id));
  interface C { key: string; icon: string; text: string; cls?: string; late?: boolean }
  const chips = $derived.by((): C[] => {
    const late = !done && !!x.when && diff(x.when) < 0, out: C[] = [];
    const on = (k: string) => show.includes(k);
    if (x.when && on("when") && !(today && !late && !x.time)) out.push({ key: "when", icon: "calendar", late, text: today && !late ? x.time! : `${dayName(x.when)}${x.time ? ` ${x.time}` : ""}` });
    if (x.pri && x.pri >= 3 && on("priority")) out.push({ key: "priority", icon: "flag", cls: `p${x.pri}`, text: PRIORITY[x.pri] });
    if (x.deadline && x.deadline !== x.when && on("deadline")) out.push({ key: "deadline", icon: "flag", late: !done && diff(x.deadline) < 0, text: dayName(x.deadline) });
    if (x.repeat && on("repeat")) out.push({ key: "repeat", icon: "repeat", text: x.repeat });
    if (x.subs && on("subs")) out.push({ key: "subs", icon: "subtasks", text: `${x.subs.filter((s) => s[1]).length}/${x.subs.length}` });
    if (x.waiting && on("waiting")) out.push({ key: "waiting", icon: "waiting", text: x.waiting });
    if (on("tags")) for (const t of x.tags) out.push({ key: "tag", icon: "tag", text: t });
    return out;
  });
  const shown = $derived(chips.slice(0, cap)), rest = $derived(chips.slice(cap));
</script>

{#each shown as c, i (`${c.key}${i}`)}<span class="chip {c.cls ?? ''}" class:late={c.late} data-chip={c.key}><Glyph name={c.icon} inline />{c.text}</span>{/each}
{#if rest.length && more}<span class="chip more" data-chip="more" role="img" aria-label={`${rest.length} more: ${rest.map((r) => r.text).join(", ")}`} title={rest.map((r) => r.text).join(", ")}>+{rest.length}</span>{/if}

<style>
  .chip { flex: none; display: inline-flex; align-items: center; gap: var(--s-2); height: var(--s-7); padding: 0 var(--s-4) 0 var(--s-3); box-sizing: border-box; border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.08); color: var(--ink-2); font-size: var(--text-label); font-weight: var(--ui-weight); line-height: 1; white-space: nowrap; font-variant-numeric: tabular-nums; }
  .more { padding-inline: var(--s-3); color: var(--ink-3); }
  .late, .p4 { background: color-mix(in oklab, var(--danger) 13%, transparent); color: var(--ink); } .late :global(svg), .p4 :global(svg) { color: var(--danger); } /* the words stay in the ink (readable on the tint); the glyph carries the colour */
  .p3 { background: color-mix(in oklab, var(--warn) 14%, transparent); color: var(--ink); } .p3 :global(svg) { color: var(--warn); }
  
  
</style>

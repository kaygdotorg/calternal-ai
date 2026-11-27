<script lang="ts">
  // The calendar: a month grid (Monday first) with the collection's dated items on their days, each a chip with its kind's glyph; a day with more than three shows "+n". Previous and next move a month. Today is outlined. Items without a date are not shown here.
  import AppIcon from "../../shell/AppIcon.svelte";
  import type { Item as ItemT } from "../../data/collections";
  let { items, onopen }: { items: ItemT[]; onopen: (i: ItemT) => void } = $props();
  const GLYPH: Record<string, string> = { note: "notes", mail: "mail", chat: "chat", task: "check", file: "file", photo: "image", event: "calendar", contact: "user", transaction: "coin", workout: "pulse", label: "folder", setting: "settings" };
  let month = $state(new Date(2026, 10, 1));
  const TODAY = "2026-11-20";
  const cells = $derived.by(() => {
    const y = month.getFullYear(), m = month.getMonth(), first = new Date(y, m, 1), lead = (first.getDay() + 6) % 7, days = new Date(y, m + 1, 0).getDate();
    return Array.from({ length: 42 }, (_, i) => { const d = i - lead + 1, inMonth = d >= 1 && d <= days, dt = new Date(y, m, d), iso = `${dt.getFullYear()}-${String(dt.getMonth() + 1).padStart(2, "0")}-${String(dt.getDate()).padStart(2, "0")}`; return { d: dt.getDate(), inMonth, iso, items: items.filter((x) => x.date === iso) }; });
  });
  const step = (n: number) => (month = new Date(month.getFullYear(), month.getMonth() + n, 1));
  const title = $derived(month.toLocaleDateString("en", { month: "long", year: "numeric" }));
  const dated = $derived(items.filter((x) => x.date).length);
</script>

<div class="cal" data-calendar>
  <header><strong>{title}</strong><span class="q">{dated} dated of {items.length}</span><span class="sp"></span><button type="button" aria-label="Previous month" onclick={() => step(-1)}>‹</button><button type="button" aria-label="Next month" onclick={() => step(1)}>›</button></header>
  <div class="wk">{#each ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"] as w (w)}<span>{w}</span>{/each}</div>
  <div class="grid">{#each cells as c (c.iso)}
    <div class="day" class:out={!c.inMonth} class:today={c.iso === TODAY}><span class="dn">{c.d}</span>
      {#each c.items.slice(0, 3) as it (it.id)}<button type="button" class="ev" data-kind={it.kind} onclick={() => onopen(it)} title={it.title}><AppIcon name={it.icon ?? GLYPH[it.kind]!} inline /><span>{it.title}</span></button>{/each}
      {#if c.items.length > 3}<span class="more">+{c.items.length - 3}</span>{/if}
    </div>{/each}</div>
</div>

<style>
  .cal { display: flex; flex-direction: column; height: 100%; min-height: 0; } header { flex: none; display: flex; align-items: center; gap: var(--s-4); padding-bottom: var(--s-4); font-size: var(--text-heading); } header strong { font-weight: var(--strong-weight); } .q { color: var(--ink-3); font-size: var(--text-label); } .sp { flex: 1; }
  header button { width: 28px; height: 28px; border-radius: var(--r-select); font-size: var(--text-heading); color: var(--ink-2); } header button:hover { background: var(--hover); }
  .wk, .grid { display: grid; grid-template-columns: repeat(7, minmax(0, 1fr)); } .wk span { padding: var(--s-2) var(--s-3); color: var(--ink-3); font-size: var(--text-label); font-weight: var(--strong-weight); }
  .grid { flex: 1; min-height: 0; grid-template-rows: repeat(6, minmax(5.5rem, 1fr)); overflow-y: auto; border-top: 1px solid rgb(var(--ink-rgb) / 0.1); }
  .day { display: flex; flex-direction: column; gap: 2px; min-width: 0; padding: var(--s-2); border-inline-end: 1px solid rgb(var(--ink-rgb) / 0.07); border-bottom: 1px solid rgb(var(--ink-rgb) / 0.07); } .day.out { opacity: 0.4; }
  .dn { align-self: flex-start; min-width: 22px; height: 22px; display: grid; place-items: center; border-radius: var(--r-menu); font-size: var(--text-label); font-variant-numeric: tabular-nums; color: var(--ink-2); } .today .dn { background: var(--badge-bg); color: #fff; }
  .ev { display: flex; align-items: center; gap: var(--s-2); min-width: 0; min-height: 20px; padding: 0 var(--s-3); border-radius: var(--r-select); background: var(--accent-soft); color: var(--ink); font-size: var(--text-label); text-align: start; } .ev span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } .ev[data-kind="task"] { background: oklch(0.85 0.08 28 / 0.4); } .ev[data-kind="mail"] { background: oklch(0.85 0.06 205 / 0.4); } .ev[data-kind="note"], .ev[data-kind="file"] { background: oklch(0.88 0.08 90 / 0.45); } .ev[data-kind="transaction"] { background: oklch(0.85 0.08 150 / 0.4); }
  .more { color: var(--ink-3); font-size: var(--text-label); padding-inline: var(--s-3); }
</style>

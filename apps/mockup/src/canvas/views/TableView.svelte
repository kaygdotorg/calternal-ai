<script lang="ts">
  // The table: delegates to `@calternal/canvas-shell`'s `TableShell` with mockup fixture columns and cell renderers.
  import { TableShell, type TableColumn } from "@calternal/canvas-shell";
  import AppIcon from "../../shell/AppIcon.svelte";
  import Av from "../../shell/Av.svelte";
  import Mark from "../../shell/Mark.svelte";
  import { arr } from "../../arrange.svelte";
  import type { Item as ItemT } from "../../data/collections";
  import { itemWhen } from "../../lib/when";

  let { items, onopen }: { items: ItemT[]; onopen: (i: ItemT) => void } = $props();
  const GLYPH: Record<string, string> = { note: "notes", mail: "mail", chat: "chat", task: "check", file: "file", photo: "image", event: "calendar", contact: "user", transaction: "coin", workout: "pulse", label: "folder", setting: "settings" };
  type Key = "title" | "kind" | "status" | "date" | "amount" | "owner";
  let key = $state<Key>("title"), dir = $state<1 | -1>(1);
  const num = (a?: string) => (a ? parseFloat(a.replace(/[^\d.+-]/g, "")) || 0 : 0);
  const val = (i: ItemT, k: Key) => (k === "amount" ? num(i.amount) : k === "status" ? (arr.statusOf(i.id) ?? "") : k === "date" ? (i.date ?? "") : (i[k] ?? ""));
  const rows = $derived([...items].sort((a, b) => { const x = val(a, key), y = val(b, key); return (typeof x === "number" && typeof y === "number" ? x - y : String(x).localeCompare(String(y))) * dir; }));
  const sort = (k: string) => { if (key === k) dir = (dir * -1) as 1 | -1; else { key = k as Key; dir = 1; } };
  const detail = (i: ItemT) => (i.kind === "task" ? i.due : i.sub ?? i.body ?? "");

  const columns: TableColumn[] = [
    { key: "title", label: "Name" },
    { key: "kind", label: "Kind" },
    { key: "status", label: "Status" },
    { key: "detail", label: "Detail", sortable: false },
    { key: "date", label: "Date" },
    { key: "amount", label: "Amount", align: "end" },
    { key: "owner", label: "Owner" },
  ];
</script>

<TableShell {columns} {rows} sortKey={key} sortDir={dir} onsort={sort} {onopen}>
  {#snippet renderCell(it, col)}
    {#if col.key === "title"}
      <span class="nm">{#if it.provider}<Mark provider={it.provider} size="18px" />{:else}<span class="ic"><AppIcon name={it.icon ?? GLYPH[it.kind]!} inline /></span>{/if}<span class="tx" title={it.title}>{it.title}</span></span>
    {:else if col.key === "kind"}
      <span class="q">{it.kind}</span>
    {:else if col.key === "status"}
      {#if arr.statusOf(it.id) ?? it.status}<span class="pill" data-s={arr.statusOf(it.id) ?? it.status}>{arr.statusOf(it.id) ?? it.status}</span>{/if}
    {:else if col.key === "detail"}
      <span class="dt">{#if it.kind === "task" && it.due}<span class="chip">{it.due}</span>{:else}<span class="tx" title={detail(it)}>{detail(it)}</span>{/if}</span>
    {:else if col.key === "date"}
      <span class="q num">{itemWhen(it) ?? ""}</span>
    {:else if col.key === "amount"}
      <span class="amt" class:pos={it.amount?.startsWith("+")}>{it.amount ?? ""}</span>
    {:else if col.key === "owner"}
      {#if it.owner}<span class="nm"><Av name={it.owner} size="18px" /><span class="tx">{it.owner}</span></span>{/if}
    {/if}
  {/snippet}
</TableShell>

<style>
  .nm { display: flex; align-items: center; gap: var(--s-3); min-width: 0; }
  .tx { overflow: hidden; text-overflow: ellipsis; }
  .ic { display: grid; color: var(--ink-2); }
  .q { color: var(--ink-3); font-size: var(--text-label); }
  .num { font-variant-numeric: tabular-nums; }
  .dt { color: var(--ink-2); }
  .amt { font-variant-numeric: tabular-nums; }
  .amt.pos { color: var(--good, oklch(0.62 0.14 150)); }
  .chip { white-space: nowrap; padding: 0 var(--s-4); min-height: var(--s-7); display: inline-flex; align-items: center; border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.08); color: var(--ink-2); font-size: var(--text-label); line-height: 1.25; }
  .pill { padding: 1px var(--s-4); border-radius: var(--r-pill); font-size: var(--text-label); background: rgb(var(--ink-rgb) / 0.08); }
  .pill[data-s="Doing"] { background: var(--accent-soft); color: var(--accent); }
  .pill[data-s="Done"] { background: oklch(0.85 0.08 150 / 0.4); }
</style>

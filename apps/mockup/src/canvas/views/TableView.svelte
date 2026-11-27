<script lang="ts">
  // The table: aligned columns (Name, Kind, Status, Detail, Date, Owner), a sticky header that sorts when pressed (the arrow shows which way), one line per row, and a cell per kind in Detail (a price right-aligned and tabular, a due date as a chip, a
  // size, a distance, the subject of a mail). Rows highlight on hover and open the item. Numbers use tabular figures so columns line up; long text truncates with the full text in its tooltip.
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
  const sort = (k: Key) => { if (key === k) dir = (dir * -1) as 1 | -1; else { key = k; dir = 1; } };
  const detail = (i: ItemT) => i.kind === "task" ? i.due : i.sub ?? i.body ?? "";
  const nice = (d?: string) => (d ? new Date(d + "T12:00").toLocaleDateString("en", { day: "numeric", month: "short" }) : "");
  /** The columns, each with one alignment for every kind: text columns start-aligned, Amount (money only) end-aligned with tabular figures. */
  const COLS: [string, string, string][] = [["title", "Name", ""], ["kind", "Kind", ""], ["status", "Status", ""], ["detail", "Detail", ""], ["date", "Date", ""], ["amount", "Amount", "end"], ["owner", "Owner", ""]];
</script>

<div class="tw" data-table-wrap><table data-table>
  <thead><tr>
    {#each COLS as [k, l, cls] (k)}<th class={cls} aria-sort={key === k ? (dir === 1 ? "ascending" : "descending") : "none"}>{#if k === "detail"}<span class="plain">{l}</span>{:else}<button type="button" class={cls} onclick={() => sort(k as Key)}>{l}{#if key === k}<span class="ar">{dir === 1 ? "↑" : "↓"}</span>{/if}</button>{/if}</th>{/each}
  </tr></thead>
  <tbody>
    {#each rows as it (it.id)}
      <tr onclick={() => onopen(it)} tabindex="0" onkeydown={(e) => e.key === "Enter" && onopen(it)}>
        <td><span class="nm">{#if it.provider}<Mark provider={it.provider} size="18px" />{:else}<span class="ic"><AppIcon name={it.icon ?? GLYPH[it.kind]!} inline /></span>{/if}<span class="tx" title={it.title}>{it.title}</span></span></td>
        <td class="q">{it.kind}</td>
        <td>{#if arr.statusOf(it.id) ?? it.status}<span class="pill" data-s={arr.statusOf(it.id) ?? it.status}>{arr.statusOf(it.id) ?? it.status}</span>{/if}</td>
        <td class="dt">{#if it.kind === "task" && it.due}<span class="chip">{it.due}</span>{:else}<span class="tx" title={detail(it)}>{detail(it)}</span>{/if}</td>
        <td class="q num">{itemWhen(it) ?? ""}</td>
        <td class="amt" class:pos={it.amount?.startsWith("+")}>{it.amount ?? ""}</td>
        <td>{#if it.owner}<span class="nm"><Av name={it.owner} size="18px" /><span class="tx">{it.owner}</span></span>{/if}</td>
      </tr>
    {/each}
  </tbody>
</table></div>

<style>
  .tw { overflow-x: auto; overflow-y: hidden; --sticky-top: 0px; } table { width: 100%; min-width: 32rem; table-layout: auto; border-collapse: separate; border-spacing: 0; font-size: var(--text-ui); line-height: 1.25; }
  thead th { position: sticky; top: var(--sticky-top, 0px); z-index: 1; text-align: start; padding: 0; background: rgb(var(--tint) / 0.5); backdrop-filter: blur(12px); border-bottom: 1px solid rgb(var(--ink-rgb) / 0.12); } 
  th button { display: flex; align-items: center; gap: var(--s-2); width: 100%; min-height: 34px; padding: 0 var(--s-4); color: var(--ink-3); font-size: var(--text-label); font-weight: var(--strong-weight); text-align: start; } th button:hover { color: var(--ink); } .ar { color: var(--accent); }
  td:first-child, th:first-child { width: 100%; min-width: 9rem; max-width: 0; } th { white-space: nowrap; } td { height: 40px; vertical-align: middle; line-height: 1.25; padding: 0 var(--s-4); border-bottom: 1px solid rgb(var(--ink-rgb) / 0.07); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; } tbody tr { cursor: pointer; } tbody tr:hover td { background: var(--hover); } tbody tr:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  .nm { display: flex; align-items: center; gap: var(--s-3); min-width: 0; } .tx { overflow: hidden; text-overflow: ellipsis; } .ic { display: grid; color: var(--ink-2); } .q { color: var(--ink-3); font-size: var(--text-label); } .num { font-variant-numeric: tabular-nums; }
  .dt { color: var(--ink-2); } .amt { font-variant-numeric: tabular-nums; text-align: end; } th.end, th button.end { justify-content: flex-end; text-align: end; } .plain { display: flex; align-items: center; min-height: 34px; padding: 0 var(--s-4); color: var(--ink-3); font-size: var(--text-label); font-weight: var(--strong-weight); } .amt.pos { color: var(--good, oklch(0.62 0.14 150)); }
  .chip { white-space: nowrap; padding: 0 var(--s-4); min-height: var(--s-7); display: inline-flex; align-items: center; border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.08); color: var(--ink-2); font-size: var(--text-label); line-height: 1.25; }
  .pill { padding: 1px var(--s-4); border-radius: var(--r-pill); font-size: var(--text-label); background: rgb(var(--ink-rgb) / 0.08); } .pill[data-s="Doing"] { background: var(--accent-soft); color: var(--accent); } .pill[data-s="Done"] { background: oklch(0.85 0.08 150 / 0.4); }
</style>

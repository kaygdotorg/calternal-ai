<script lang="ts">
  // A collection opened as a window: its title and count, "New" from the app's create operation, and the big view the person
  // picked (list or board; interface state, not declaration). The list is grouped and the board's columns are made from the
  // collection's `group` field, with the column names from that field's declared `values`. Knows no kind.
  import { Button, Selector } from "@calternal/ui";
  import { collectionId, fieldOf, windowOf, type Json, type Row } from "../lib/model";
  import { newAction, type Action } from "../lib/look";
  import { call } from "../lib/calls.svelte";
  import RowLook from "./RowLook.svelte";
  import CardLook from "./CardLook.svelte";
  let { app, c, view = $bindable("list"), selected, onopen, onaction }: { app: Json; c: Json; view?: "list" | "board"; selected: string | null; onopen: (r: Row) => void; onaction: (a: Action) => void } = $props();
  const win = $derived(windowOf(collectionId(app, c)));
  const add = $derived(newAction(app, c));
  const field = $derived(c.group && c.kind ? fieldOf(`${app.id}/${c.kind}`, c.group) : null);
  const groups = $derived.by(() => {
    if (!field) return [{ key: "", name: "", rows: win.rows }];
    const names: Record<string, string> = field.values ?? {};
    const keys = Object.keys(names).length ? Object.keys(names) : [...new Set(win.rows.map((r) => String(r.values[c.group])))];
    return keys.map((k) => ({ key: k, name: names[k] ?? k, rows: win.rows.filter((r) => String(r.values[c.group] ?? field.default ?? "") === k) }));
  });
  const move = (row: Row, key: string) => call("host.files.set", { item: row.item, fields: { [c.group]: field?.type === "bool" ? key === "true" : key } });
</script>

<section class="cw" aria-label={c.collection}>
  <header class="hd">
    <h1>{c.collection}</h1>{#if win.count != null}<span class="n">{win.count}</span>{/if}
    <span class="sp"></span>
    {#if c.group}<Selector label="View" size="sm" fill={false} bind:value={view} options={[{ value: "list", label: "List" }, { value: "board", label: "Board" }]} />{/if}
    {#if add}<Button text={add.label} icon="plus" variant="secondary" onclick={() => onaction(add)} />{/if}
  </header>
  {#if view === "board" && field}
    <div class="board">
      {#each groups as g (g.key)}
        <section class="col" aria-label={`${g.name}, ${g.rows.length}`}>
          <header><strong>{g.name}</strong><span class="n">{g.rows.length}</span></header>
          <div class="cards">{#each g.rows as row (row.item)}<div oncontextmenu={(e) => { e.preventDefault(); const other = groups.find((x) => x.key !== g.key); if (other) move(row, other.key); }}><CardLook {row} {onopen} /></div>{/each}</div>
        </section>
      {/each}
    </div>
  {:else}
    <div class="list">
      {#each groups as g (g.key)}
        {#if g.name}<h2 class="gh">{g.name}</h2>{/if}
        {#each g.rows as row (row.item)}<RowLook {row} selected={selected === row.item} {onopen} />{/each}
      {/each}
    </div>
  {/if}
</section>

<style>
  .cw { display: flex; flex-direction: column; min-height: 0; height: 100%; }
  .hd { flex: none; display: flex; align-items: center; gap: var(--s-4); padding: var(--s-5) var(--s-6) var(--s-4); flex-wrap: wrap; }
  h1 { margin: 0; font-size: var(--text-title, 1.25rem); font-weight: var(--strong-weight); } .n { color: var(--ink-3); font-variant-numeric: tabular-nums; } .sp { flex: 1; }
  .list { flex: 1; overflow-y: auto; padding: 0 var(--s-4) var(--s-6); }
  .gh { margin: var(--s-5) 0 var(--s-2); padding: 0 var(--s-5); font-size: var(--text-label); font-weight: var(--strong-weight); color: var(--ink-3); }
  .board { flex: 1; min-height: 0; display: grid; grid-auto-flow: column; grid-auto-columns: minmax(14rem, 1fr); gap: var(--s-5); padding: 0 var(--s-6) var(--s-6); overflow-x: auto; scroll-snap-type: x mandatory; }
  .col { display: flex; flex-direction: column; min-height: 0; border-radius: var(--r-card); background: rgb(var(--ink-rgb) / 0.05); scroll-snap-align: start; }
  .col header { display: flex; align-items: center; gap: var(--s-4); padding: var(--s-5) var(--s-5) var(--s-3); }
  .cards { flex: 1; overflow-y: auto; display: grid; align-content: start; gap: var(--s-3); padding: var(--s-2) var(--s-3) var(--s-4); }
  @media (max-width: 719px) { .board { grid-auto-columns: 82%; } .hd { padding-inline: var(--s-5); } }
</style>

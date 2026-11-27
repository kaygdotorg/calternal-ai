<script lang="ts">
  /**
   * Generic declaration-driven collection view rendered inside `Canvas.svelte`.
   * Supports `list` / `compact`, `board` (via `@calternal/canvas-shell`'s `BoardShell`),
   * and `table` (via `TableShell`) with a view switcher when multiple views are declared,
   * group headers and totals from `QueryWindowResult`, inline primary actions from
   * `row.when` + `placement`, and live search bound to `kind.search`.
   * Zero hardcoded app, kind, or field names.
   */
  import { Icon, Menu, type MenuItem } from "@calternal/ui";
  import { Av, BoardShell, TableShell, type BoardColumn, type TableColumn } from "@calternal/canvas-shell";
  import {
    boardLanes,
    formatCollectionTotals,
    formatFieldValue,
    formatRelativeTime,
    humanize,
    itemMenuActions,
    itemPrimaryActions,
    kindHasRecords,
    readRowValue,
    rowBadge,
    rowTitle,
    type DerivedAction,
  } from "./format";
  import type { CollectionDecl, KindDecl, QueryWindowResult, RowData } from "./types";

  let {
    collection,
    kind,
    result,
    selectedId,
    searchQuery = "",
    onsearch,
    onopen,
    onadd,
    onaction,
    onmoveField,
  }: {
    collection: CollectionDecl;
    kind: KindDecl | null;
    result: QueryWindowResult;
    selectedId: string | null;
    searchQuery?: string;
    onsearch?: (q: string) => void;
    onopen: (row: RowData) => void;
    onadd: (kind: KindDecl) => void;
    onaction: (row: RowData, action: DerivedAction) => void;
    onmoveField?: (row: RowData, field: string, value: string | boolean) => void;
  } = $props();

  let viewIndex = $state(0);
  $effect(() => {
    void collection.qualified;
    viewIndex = 0;
  });

  const views = $derived(collection.views.length ? collection.views : [{ type: "list" as const, columns: kind?.row ?? ["title"] }]);
  const activeView = $derived(views[Math.min(viewIndex, views.length - 1)]!);
  const rows = $derived(result.rows);
  const totals = $derived(formatCollectionTotals(collection, result));
  const canSearch = $derived(Boolean(kind?.search?.length && onsearch));
  const isRecordsKind = $derived(Boolean(kind && kindHasRecords(kind)));

  // Columns to show in list/table (first column is title; remaining are secondary metadata columns).
  const displayCols = $derived(activeView.columns.length ? activeView.columns : (kind?.row ?? ["title"]));
  const titleCol = $derived(displayCols[0] ?? "title");
  const secondaryCols = $derived(displayCols.slice(1).filter((c) => !(kind?.fields[c]?.badge)));

  // Grouped list sections derived strictly from `result.groups` + `row.groups`.
  const groupedSections = $derived.by(() => {
    if (!collection.grouped || !result.groups.length) return null;
    return result.groups.map((g) => {
      const keyStr = g.key === null || g.key === undefined ? "" : String(g.key);
      const groupRows = rows.filter((r) => {
        const rk = r.groups?.[0];
        return (rk === null || rk === undefined ? "" : String(rk)) === keyStr;
      });
      return {
        key: keyStr,
        label: keyStr ? humanize(keyStr) : "Other",
        matched: g.matched,
        rows: groupRows,
      };
    });
  });

  // Board columns derived strictly from the declared board field's schema (`yes/no` or `choice`).
  const boardField = $derived(activeView.type === "board" ? activeView.columns[0] ?? "" : "");
  const lanes = $derived(kind && boardField ? boardLanes(kind, boardField) : []);
  const boardColumns = $derived<BoardColumn<RowData>[]>(
    lanes.map((lane) => ({
      id: lane.id,
      label: lane.label,
      items: rows.filter((r) => {
        const v = readRowValue(r, boardField);
        if (typeof lane.value === "boolean") return Boolean(v) === lane.value;
        return String(v ?? "") === lane.id;
      }),
    })),
  );

  // Table columns for `table` view.
  const tableColumns = $derived<TableColumn[]>(
    displayCols.map((c) => ({
      key: c,
      label: humanize(c),
      align: kind?.fields[c]?.type === "number" || kind?.fields[c]?.type?.startsWith("money") ? "end" : "start",
      sortable: false,
    })),
  );

  let menuRow = $state<RowData | null>(null);
  let menuEl = $state<HTMLElement | null>(null);
  let actions = $state<DerivedAction[]>([]);
  const menuItems = $derived<MenuItem[]>(
    actions.map((a) => ({ id: a.id, label: a.label, danger: a.danger })),
  );

  function openMenu(row: RowData, el: HTMLElement) {
    if (!kind) return;
    actions = itemMenuActions(row, kind);
    if (!actions.length) return;
    menuRow = row;
    menuEl = el;
  }

  function handleMenuPick(id: string) {
    const r = menuRow;
    const act = actions.find((a) => a.id === id);
    menuRow = null;
    if (r && act) onaction(r, act);
  }

  function handleBoardMove(row: RowData, toColId: string) {
    if (!onmoveField || !boardField) return;
    const lane = lanes.find((l) => l.id === toColId);
    if (!lane) return;
    onmoveField(row, boardField, lane.value);
  }

  function secondaryText(row: RowData): string {
    const parts: string[] = [];
    for (const col of secondaryCols) {
      const decl = kind?.fields[col];
      const v = formatFieldValue(readRowValue(row, col), decl);
      if (v) parts.push(v);
    }
    if (!parts.length && isRecordsKind && row.edited) {
      const rel = formatRelativeTime(row.edited);
      if (rel) parts.push(rel);
    }
    return parts.join(" · ");
  }
</script>

{#snippet rowCard(row: RowData)}
  {@const title = rowTitle(row, kind ?? undefined)}
  {@const badge = rowBadge(row, kind ?? undefined)}
  {@const sub = secondaryText(row)}
  {@const primaries = kind ? itemPrimaryActions(row, kind) : []}
  {@const sel = selectedId === row.id}
  <div
    class="row-wrap"
    data-row={row.id}
    oncontextmenu={(e) => {
      e.preventDefault();
      openMenu(row, e.currentTarget as HTMLElement);
    }}
  >
    <button
      type="button"
      class="irow"
      class:sel
      aria-current={sel ? "true" : undefined}
      data-item-id={row.id}
      onclick={() => onopen(row)}
    >
      {#if isRecordsKind}
        <span class="lead" aria-hidden="true"><Av name={title} size="28px" /></span>
      {/if}
      <span class="mid">
        <span class="row-title">{title}</span>
        {#if sub}<span class="row-sub">{sub}</span>{/if}
      </span>
      {#if badge}
        <span class="badge" aria-label={`${badge.value} ${badge.name}`}>{badge.value}</span>
      {/if}
    </button>
    {#if primaries.length}
      <span class="primaries">
        {#each primaries as act (act.id)}
          <button
            type="button"
            class="prim-btn"
            data-primary-op={act.operation.name}
            onclick={() => onaction(row, act)}
          >
            {act.label}
          </button>
        {/each}
      </span>
    {/if}
    {#if kind}
      <button
        type="button"
        class="more"
        aria-label={`Actions for ${title}`}
        onclick={(e) => {
          e.stopPropagation();
          openMenu(row, e.currentTarget as HTMLElement);
        }}
      >
        <Icon name="more" inline />
      </button>
    {/if}
  </div>
{/snippet}

<section class="col-view" aria-label={collection.title} data-collection-view={activeView.type}>
  <div class="bar">
    <div class="bar-left">
      {#if views.length > 1}
        <div class="vswitch" role="tablist" aria-label="View">
          {#each views as v, i (v.type + i)}
            <button
              type="button"
              role="tab"
              class="vtab"
              class:on={viewIndex === i}
              aria-selected={viewIndex === i}
              data-view-tab={v.type}
              onclick={() => (viewIndex = i)}
            >
              {humanize(v.type)}
            </button>
          {/each}
        </div>
      {/if}
      {#if totals.length}
        <div class="totals" data-collection-totals>
          {#each totals as t (t.label)}
            <span class="tot"><em>{t.label}</em> <strong>{t.value}</strong></span>
          {/each}
        </div>
      {/if}
    </div>
    <div class="bar-right">
      {#if canSearch}
        <label class="search-box">
          <Icon name="search" inline />
          <input
            type="search"
            value={searchQuery}
            placeholder="Search"
            aria-label={`Search ${collection.title}`}
            oninput={(e) => onsearch?.((e.currentTarget as HTMLInputElement).value)}
          />
        </label>
      {/if}
      {#if kind?.operations.add}
        <button type="button" class="add-btn" onclick={() => onadd(kind)}>
          <Icon name="plus" inline />
          <span>New {kind.name}</span>
        </button>
      {/if}
    </div>
  </div>

  {#if activeView.type === "board" && boardColumns.length}
    <div class="board-wrap">
      <BoardShell
        columns={boardColumns}
        emptyText="None"
        canDrag={Boolean(onmoveField)}
        onmove={handleBoardMove}
      >
        {#snippet renderCard(row, quiet)}
          {@const title = rowTitle(row, kind ?? undefined)}
          {@const sub = secondaryText(row)}
          {@const primaries = kind ? itemPrimaryActions(row, kind) : []}
          {@const firstPrimary = primaries[0] ?? null}
          {@const restPrimaries = primaries.slice(1)}
          <div class="bcard" data-row={row.id}>
            <div class="bcard-head">
              {#if firstPrimary}
                <button
                  type="button"
                  class="chk-btn"
                  aria-label={firstPrimary.label}
                  title={firstPrimary.label}
                  data-primary-op={firstPrimary.operation.name}
                  onclick={(e) => { e.stopPropagation(); onaction(row, firstPrimary); }}
                >
                  <span class="chk-glyph" aria-hidden="true"><Icon name="check" inline /></span>
                </button>
              {/if}
              <button
                type="button"
                class="bcard-main"
                onclick={() => { if (!quiet) onopen(row); }}
              >
                <strong class="btitle">{title}</strong>
                {#if sub}<span class="bsub">{sub}</span>{/if}
              </button>
            </div>
            {#if restPrimaries.length}
              <div class="bacts">
                {#each restPrimaries as act (act.id)}
                  <button
                    type="button"
                    class="prim-btn"
                    data-primary-op={act.operation.name}
                    onclick={(e) => { e.stopPropagation(); onaction(row, act); }}
                  >
                    {act.label}
                  </button>
                {/each}
              </div>
            {/if}
          </div>
        {/snippet}
      </BoardShell>
    </div>
  {:else if activeView.type === "table"}
    <div class="table-wrap">
      <TableShell columns={tableColumns} {rows} {onopen}>
        {#snippet renderCell(row, col)}
          {@const val = col.key === titleCol ? rowTitle(row, kind ?? undefined) : formatFieldValue(readRowValue(row, col.key), kind?.fields[col.key])}
          <span class={col.key === titleCol ? "row-title" : "tval"}>{val}</span>
        {/snippet}
      </TableShell>
    </div>
  {:else}
    <div class="list" role="list">
      {#if !rows.length}
        <p class="empty">Nothing here yet.</p>
      {:else if groupedSections}
        {#each groupedSections as sec (sec.key)}
          <div class="grp" data-group={sec.key}>
            <header class="gh">
              <strong>{sec.label}</strong>
              <span class="gn">{sec.matched}</span>
            </header>
            {#each sec.rows as row (row.id)}
              {@render rowCard(row)}
            {/each}
          </div>
        {/each}
      {:else}
        {#each rows as row (row.id)}
          {@render rowCard(row)}
        {/each}
      {/if}
    </div>
  {/if}
</section>

<Menu
  open={menuRow !== null}
  items={menuItems}
  label="Item actions"
  onpick={handleMenuPick}
  onclose={() => (menuRow = null)}
  trigger={menuEl}
/>

<style>
  .col-view {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    padding: var(--s-2) var(--s-5) var(--s-5);
    gap: var(--s-3);
    box-sizing: border-box;
  }
  .bar {
    flex: none;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--s-3);
    flex-wrap: wrap;
    padding: 0 var(--s-2);
  }
  .bar-left, .bar-right {
    display: inline-flex;
    align-items: center;
    gap: var(--s-3);
    flex-wrap: wrap;
  }
  .vswitch {
    display: inline-flex;
    padding: 2px;
    border-radius: var(--r-pill);
    background: rgb(var(--ink-rgb) / 0.06);
  }
  .vtab {
    min-height: 28px;
    padding: 0 var(--s-4);
    border: 0;
    border-radius: var(--r-pill);
    background: transparent;
    color: var(--ink-2);
    font: inherit;
    font-size: var(--text-label);
    font-weight: var(--strong-weight);
    cursor: pointer;
    transition: background-color var(--dur-fast) var(--ease-out), color var(--dur-fast) var(--ease-out);
  }
  .vtab.on { background: var(--paper, #fff); color: var(--ink); box-shadow: 0 1px 2px rgb(0 0 0 / 0.1); }
  :global(html[data-scheme="dark"]) .vtab.on { background: rgb(var(--ink-rgb) / 0.16); }
  .totals { display: inline-flex; align-items: center; gap: var(--s-3); font-size: var(--text-label); color: var(--ink-2); font-variant-numeric: tabular-nums; }
  .tot em { font-style: normal; color: var(--ink-3); }
  .tot strong { color: var(--ink); font-weight: var(--strong-weight); }
  .search-box {
    display: inline-flex;
    align-items: center;
    gap: var(--s-2);
    min-height: 32px;
    padding: 0 var(--s-3);
    border-radius: var(--r-pill);
    background: rgb(var(--ink-rgb) / 0.06);
    color: var(--ink-2);
  }
  .search-box input {
    width: 10rem;
    border: 0;
    outline: 0;
    background: none;
    color: var(--ink);
    font: inherit;
    font-size: var(--text-label);
  }
  .add-btn {
    display: inline-flex;
    align-items: center;
    gap: var(--s-2);
    min-height: 32px;
    padding: 0 var(--s-4);
    border: 0;
    border-radius: var(--r-pill);
    background: var(--accent-soft);
    color: var(--accent);
    font: inherit;
    font-size: var(--text-label);
    font-weight: var(--strong-weight);
    cursor: pointer;
    transition: background-color var(--dur-fast) var(--ease-out), scale var(--dur-fast) var(--ease-out);
  }
  .add-btn:hover { background: color-mix(in oklab, var(--accent) 18%, transparent); }
  .add-btn:active { scale: var(--press-scale); }
  .add-btn:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }

  .board-wrap { flex: 1; min-height: 0; }
  .table-wrap { flex: 1; min-height: 0; overflow: auto; }
  .list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .grp { display: flex; flex-direction: column; gap: 2px; margin-bottom: var(--s-3); }
  .gh {
    display: flex;
    align-items: center;
    gap: var(--s-3);
    padding: var(--s-2) var(--s-4);
    font-size: var(--text-label);
    color: var(--ink-2);
  }
  .gh strong { font-weight: var(--strong-weight); }
  .gn {
    min-width: 18px;
    padding: 0 var(--s-2);
    border-radius: var(--r-pill);
    background: rgb(var(--ink-rgb) / 0.08);
    color: var(--ink-2);
    font-size: var(--text-label);
    font-variant-numeric: tabular-nums;
    text-align: center;
  }
  .empty {
    margin: auto;
    color: var(--ink-2);
    font-size: var(--text-ui);
    text-align: center;
  }
  .row-wrap {
    position: relative;
    display: flex;
    align-items: center;
    gap: var(--s-2);
    border-radius: var(--r-select);
  }
  .row-wrap:hover { background: var(--hover); }
  .irow {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: var(--s-4);
    min-height: 48px;
    padding: var(--s-2) var(--s-4);
    border: 0;
    border-radius: var(--r-select);
    background: transparent;
    color: var(--ink);
    font: inherit;
    text-align: start;
    cursor: pointer;
    transition: background-color var(--dur-fast) var(--ease-out), scale var(--dur-fast) var(--ease-out);
  }
  .irow:active { scale: var(--press-scale); }
  .irow.sel { background: var(--accent-soft); }
  .irow:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  .lead { flex: none; display: grid; place-items: center; width: 28px; }
  .mid {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
  }
  .row-title {
    width: 100%;
    font-size: var(--text-ui);
    font-weight: var(--strong-weight);
    color: var(--ink);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .row-sub {
    width: 100%;
    font-size: var(--text-label);
    color: var(--ink-2);
    font-variant-numeric: tabular-nums;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .badge {
    flex: none;
    min-width: 20px;
    height: 20px;
    padding: 0 6px;
    border-radius: var(--r-pill);
    display: grid;
    place-items: center;
    background: var(--badge-bg, var(--accent-base));
    color: #fff;
    font-size: var(--text-label);
    font-weight: var(--strong-weight);
    font-variant-numeric: tabular-nums;
  }
  .primaries { flex: none; display: inline-flex; align-items: center; gap: var(--s-2); }
  .prim-btn {
    min-height: 28px;
    padding: 0 var(--s-3);
    border: 0;
    border-radius: var(--r-pill);
    background: var(--accent-soft);
    color: var(--accent);
    font: inherit;
    font-size: var(--text-label);
    font-weight: var(--strong-weight);
    cursor: pointer;
    transition: background-color var(--dur-fast) var(--ease-out), scale var(--dur-fast) var(--ease-out);
  }
  .prim-btn:hover { background: color-mix(in oklab, var(--accent) 18%, transparent); }
  .prim-btn:active { scale: var(--press-scale); }
  .more {
    flex: none;
    width: 30px;
    height: 30px;
    margin-inline-end: var(--s-2);
    border: 0;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: transparent;
    color: var(--ink-2);
    cursor: pointer;
    opacity: 0;
    transition: opacity var(--dur-fast) var(--ease-out), background-color var(--dur-fast) var(--ease-out);
  }
  .row-wrap:hover .more, .more:focus-visible { opacity: 1; }
  .more:hover { background: rgb(var(--ink-rgb) / 0.08); color: var(--ink); }
  .more:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  @media (hover: none) { .more { opacity: 1; } }

  .bcard {
    display: grid;
    gap: var(--s-2);
    padding: var(--s-4);
    border-radius: var(--r-menu);
    background: var(--paper, #fff);
    box-shadow: 0 1px 2px rgb(0 0 0 / 0.08);
  }
  :global(html[data-scheme="dark"]) .bcard { background: rgb(var(--tint) / 0.9); }
  .bcard-head {
    display: flex;
    align-items: flex-start;
    gap: var(--s-3);
  }
  .chk-btn {
    flex: none;
    width: 18px;
    height: 18px;
    margin-top: 2px;
    padding: 0;
    box-sizing: border-box;
    border-radius: 50%;
    border: 2px solid var(--ink-3);
    background: transparent;
    color: transparent;
    display: grid;
    place-items: center;
    cursor: pointer;
    transition: border-color var(--dur-fast) var(--ease-out), background-color var(--dur-fast) var(--ease-out), color var(--dur-fast) var(--ease-out), scale var(--dur-fast) var(--ease-out);
  }
  .chk-btn .chk-glyph { display: grid; place-items: center; transform: scale(0.72); }
  .chk-btn:hover, .chk-btn:focus-visible { border-color: var(--accent); color: var(--accent); background: var(--accent-soft); }
  .chk-btn:active { scale: var(--press-scale); }
  .chk-btn:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .bcard-main {
    flex: 1;
    min-width: 0;
    display: grid;
    gap: var(--s-1);
    text-align: start;
    color: var(--ink);
    cursor: pointer;
  }
  .btitle { font-size: var(--text-ui); font-weight: var(--strong-weight); overflow-wrap: anywhere; }
  .bsub { font-size: var(--text-label); color: var(--ink-2); }
  .bacts { display: flex; flex-wrap: wrap; gap: var(--s-2); }
  .tval { color: var(--ink-2); font-size: var(--text-label); }
</style>

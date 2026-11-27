<script lang="ts" module>
  export interface TableColumn {
    key: string;
    label: string;
    align?: "start" | "end";
    sortable?: boolean;
  }
</script>

<script lang="ts" generics="T extends { id: string }">
  /**
   * Shared table layout from `apps/mockup/src/canvas/views/TableView.svelte`.
   * Sticky frosted header with sort arrows, aligned columns, tabular figures,
   * and keyboard-accessible row selection. Used by both `apps/mockup` and `apps/web`.
   */
  import type { Snippet } from "svelte";

  let {
    columns,
    rows,
    sortKey = "",
    sortDir = 1,
    onsort,
    onopen,
    renderCell,
  }: {
    columns: TableColumn[];
    rows: T[];
    sortKey?: string;
    sortDir?: 1 | -1;
    onsort?: (key: string) => void;
    onopen?: (row: T) => void;
    renderCell: Snippet<[T, TableColumn]>;
  } = $props();
</script>

<div class="tw" data-table-wrap>
  <table data-table>
    <thead>
      <tr>
        {#each columns as col (col.key)}
          <th
            class:end={col.align === "end"}
            aria-sort={sortKey === col.key ? (sortDir === 1 ? "ascending" : "descending") : "none"}
          >
            {#if col.sortable === false || !onsort}
              <span class="plain" class:end={col.align === "end"}>{col.label}</span>
            {:else}
              <button type="button" class:end={col.align === "end"} onclick={() => onsort(col.key)}>
                {col.label}{#if sortKey === col.key}<span class="ar">{sortDir === 1 ? "↑" : "↓"}</span>{/if}
              </button>
            {/if}
          </th>
        {/each}
      </tr>
    </thead>
    <tbody>
      {#each rows as row (row.id)}
        <tr
          onclick={() => onopen?.(row)}
          tabindex={onopen ? 0 : undefined}
          onkeydown={(e) => e.key === "Enter" && onopen?.(row)}
        >
          {#each columns as col (col.key)}
            <td class:end={col.align === "end"}>{@render renderCell(row, col)}</td>
          {/each}
        </tr>
      {/each}
    </tbody>
  </table>
</div>

<style>
  .tw { overflow-x: auto; overflow-y: hidden; --sticky-top: 0px; }
  table { width: 100%; min-width: 32rem; table-layout: auto; border-collapse: separate; border-spacing: 0; font-size: var(--text-ui); line-height: 1.25; }
  thead th { position: sticky; top: var(--sticky-top, 0px); z-index: 1; text-align: start; padding: 0; background: rgb(var(--tint) / 0.5); backdrop-filter: blur(12px); border-bottom: 1px solid rgb(var(--ink-rgb) / 0.12); white-space: nowrap; }
  th button, .plain { display: flex; align-items: center; gap: var(--s-2); width: 100%; min-height: 34px; padding: 0 var(--s-4); color: var(--ink-3); font-size: var(--text-label); font-weight: var(--strong-weight); text-align: start; }
  th button:hover { color: var(--ink); }
  .ar { color: var(--accent); }
  td:first-child, th:first-child { width: 100%; min-width: 9rem; max-width: 0; }
  td { height: 40px; vertical-align: middle; line-height: 1.25; padding: 0 var(--s-4); border-bottom: 1px solid rgb(var(--ink-rgb) / 0.07); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  td.end, th.end, th button.end, .plain.end { justify-content: flex-end; text-align: end; font-variant-numeric: tabular-nums; }
  tbody tr { cursor: pointer; }
  tbody tr:hover td { background: var(--hover); }
  tbody tr:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
</style>

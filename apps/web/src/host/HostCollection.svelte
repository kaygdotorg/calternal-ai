<script lang="ts">
  /**
   * Generic collection list view rendered inside `Canvas.svelte`.
   * Uses server-evaluated `row.when` for menus and `FieldDecl.computed` for badges.
   * Zero hardcoded plugin vocabulary or guessed field labels.
   */
  import { Icon, Menu, type MenuItem } from "@calternal/ui";
  import { Av } from "@calternal/canvas-shell";
  import {
    formatClock,
    itemMenuActions,
    rowBadge,
    rowTitle,
    type DerivedAction,
  } from "./format";
  import type { CollectionDecl, KindDecl, RowData } from "./types";

  let {
    collection,
    kind,
    rows,
    selectedId,
    onopen,
    onadd,
    onaction,
  }: {
    collection: CollectionDecl;
    kind: KindDecl | null;
    rows: RowData[];
    selectedId: string | null;
    onopen: (row: RowData) => void;
    onadd: (kind: KindDecl) => void;
    onaction: (row: RowData, action: DerivedAction) => void;
  } = $props();

  const canAdd = $derived(!!kind?.operations.add);
  let menu = $state<{ row: RowData; el: HTMLElement; actions: DerivedAction[] } | null>(null);
  const menuItems = $derived<MenuItem[]>(
    menu
      ? menu.actions.map((a) => ({
          id: a.id,
          label: a.label,
          danger: a.danger,
        }))
      : [],
  );

  function openMenu(row: RowData, el: HTMLElement) {
    if (!kind) return;
    const actions = itemMenuActions(row, kind);
    if (!actions.length) return;
    menu = { row, el, actions };
  }

  function pickMenu(id: string) {
    const cur = menu;
    menu = null;
    if (!cur) return;
    const action = cur.actions.find((a) => a.id === id);
    if (action) onaction(cur.row, action);
  }
</script>

<section class="cw" aria-label={collection.title}>
  <div class="list" role="list">
    {#if canAdd && kind}
      <div class="toolbar-row">
        <button type="button" class="add-pill" onclick={() => onadd(kind)}>
          <Icon name="plus" inline />
          <span>New {kind.name}</span>
        </button>
      </div>
    {/if}
    {#if !rows.length}
      <div class="empty">
        <p>Nothing in {collection.title} yet.</p>
      </div>
    {:else}
      {#each rows as row (row.id)}
        {@const title = rowTitle(row, kind ?? undefined)}
        {@const badge = rowBadge(row, kind ?? undefined)}
        {@const sel = selectedId === row.id}
        {@const edited = row.edited ? formatClock(row.edited) : ""}
        <div
          class="item-wrap"
          role="listitem"
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
            <span class="lead" aria-hidden="true"><Av name={title} size={28} /></span>
            <span class="mid">
              <span class="row-title">{title}</span>
              {#if edited}<span class="row-sub">{edited}</span>{/if}
            </span>
            {#if badge}
              <span class="badge" aria-label={`${badge.value} ${badge.name}`}>{badge.value}</span>
            {/if}
          </button>
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
      {/each}
    {/if}
  </div>

  <Menu
    open={menu !== null}
    items={menuItems}
    label="Item actions"
    onpick={pickMenu}
    onclose={() => (menu = null)}
    trigger={menu?.el ?? null}
  />
</section>

<style>
  .cw {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .list {
    padding: var(--s-2) var(--s-5) var(--s-6);
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .toolbar-row {
    display: flex;
    justify-content: flex-end;
    padding: 0 var(--s-2) var(--s-3);
  }
  .add-pill {
    display: inline-flex;
    align-items: center;
    gap: var(--s-2);
    min-height: 32px;
    padding: 0 var(--s-4);
    border-radius: var(--r-pill);
    background: var(--accent-soft);
    color: var(--accent);
    font-size: var(--text-label);
    font-weight: var(--strong-weight);
    transition: scale var(--dur-fast) var(--ease-out);
  }
  .add-pill:active { scale: var(--press-scale); }
  .empty {
    margin: auto;
    padding: var(--s-8);
    color: var(--ink-3);
    text-align: center;
    font-size: var(--text-ui);
  }
  .empty p { margin: 0; }
  .item-wrap {
    position: relative;
    display: flex;
    align-items: center;
    animation: rise var(--dur-base) var(--ease-out);
  }
  @keyframes rise {
    from { opacity: 0; translate: 0 6px; }
  }
  @media (prefers-reduced-motion: reduce) {
    .item-wrap { animation: none; }
  }
  .irow {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: var(--s-4);
    min-height: 56px;
    padding: var(--s-3) var(--s-8) var(--s-3) var(--s-5);
    border-radius: var(--r-select);
    color: var(--ink);
    font: inherit;
    text-align: start;
    transition: background-color var(--dur-fast) var(--ease-out), scale var(--dur-fast) var(--ease-out);
  }
  .irow:hover { background: var(--hover); }
  .irow:active { scale: var(--press-scale); }
  .irow:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  .irow.sel { background: var(--accent-soft); }
  .lead { flex: none; display: grid; place-items: center; width: var(--lead, 28px); }
  .mid {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 2px;
    text-align: start;
  }
  .row-title,
  .row-sub {
    display: block;
    margin: 0;
    padding: 0;
    text-align: start;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .row-title { font-size: var(--text-ui); font-weight: var(--strong-weight); color: var(--ink); }
  .row-sub { font-size: var(--text-label); color: var(--ink-2); font-variant-numeric: tabular-nums; }
  .badge {
    flex: none;
    min-width: 20px;
    height: 20px;
    padding: 0 6px;
    border-radius: var(--r-pill);
    display: grid;
    place-items: center;
    background: var(--accent-base);
    color: #fff;
    font-size: 11px;
    font-weight: var(--strong-weight);
    font-variant-numeric: tabular-nums;
  }
  .more {
    position: absolute;
    right: var(--s-3);
    width: 30px;
    height: 30px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    color: var(--ink-2);
    opacity: 0;
    transition: opacity var(--dur-fast) var(--ease-out), background-color var(--dur-fast) var(--ease-out);
  }
  .item-wrap:hover .more, .item-wrap:focus-within .more, .more:focus-visible { opacity: 1; }
  .more:hover { background: var(--hover); color: var(--ink); }
  .more:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  @media (hover: none) { .more { opacity: 1; width: 44px; height: 44px; } }
</style>

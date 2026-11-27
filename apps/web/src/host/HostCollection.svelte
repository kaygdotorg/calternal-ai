<script lang="ts">
  /**
   * Generic collection list window drawn from `CollectionDecl` and `KindDecl.row`.
   * Row actions are generated from `kind.operations` filtered by `when`.
   * Sizes follow `docs/native-ui-study.md` §2.3 & §7.3 (56px list rows, 16px inset, 8px selection radius).
   */
  import { Icon, Menu, type MenuItem } from "@calternal/ui";
  import {
    initials,
    itemMenuActions,
    rowCount,
    rowPeople,
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
    {#if !rows.length}
      <div class="empty">
        <p>Nothing in {collection.title} yet.</p>
        {#if canAdd && kind}
          <button type="button" class="empty-add" onclick={() => onadd(kind)}>
            <Icon name="plus" inline />
            <span>New {kind.name}</span>
          </button>
        {/if}
      </div>
    {:else}
      {#each rows as row (row.id)}
        {@const title = rowTitle(row, kind ?? undefined)}
        {@const people = rowPeople(row, kind ?? undefined)}
        {@const count = rowCount(row, kind ?? undefined)}
        {@const sel = selectedId === row.id}
        <div
          class="item-wrap"
          role="listitem"
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
            <span class="lead" aria-hidden="true">
              <span class="av">{initials(people[0] ?? title)}</span>
            </span>
            <span class="mid">
              <span class="title">{title}</span>
              {#if people.length}
                <span class="sub">{people.join(", ")}</span>
              {/if}
            </span>
            {#if count > 0}
              <span class="badge" aria-label={`${count} unread`}>{count}</span>
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
    overflow: hidden;
  }
  .list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
    padding: calc(var(--bar-h, 52px) + var(--s-2)) var(--s-4) var(--s-6);
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .empty {
    margin: auto;
    display: grid;
    justify-items: center;
    gap: var(--s-4);
    padding: var(--s-8);
    color: var(--ink-3);
    text-align: center;
  }
  .empty p { margin: 0; font-size: var(--text-ui); }
  .empty-add {
    display: inline-flex;
    align-items: center;
    gap: var(--s-3);
    min-height: 36px;
    padding: 0 var(--s-5);
    border: 0;
    border-radius: var(--r-pill);
    background: var(--accent-soft);
    color: var(--accent);
    font: inherit;
    font-size: var(--text-ui);
    font-weight: var(--strong-weight);
    cursor: pointer;
    transition: scale var(--dur-fast) var(--ease-out), background-color var(--dur-fast) var(--ease-out);
  }
  .empty-add:active { scale: var(--press-scale); }
  .empty-add:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }

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
    padding: var(--s-3) var(--s-6) var(--s-3) var(--s-5);
    border: 0;
    border-radius: var(--r-select);
    background: transparent;
    color: var(--ink);
    font: inherit;
    text-align: start;
    cursor: pointer;
    transition: background-color var(--dur-fast) var(--ease-out), scale var(--dur-fast) var(--ease-out);
  }
  .irow:hover { background: var(--hover); }
  .irow:active { scale: var(--press-scale); }
  .irow:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  .irow.sel { background: var(--accent-soft); }

  .lead {
    width: 28px;
    height: 28px;
    flex: none;
    display: grid;
    place-items: center;
  }
  .av {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: color-mix(in oklab, var(--accent-base) 20%, transparent);
    color: var(--accent);
    font-size: 11px;
    font-weight: var(--strong-weight);
  }
  .mid {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
  }
  .title {
    width: 100%;
    font-size: var(--text-ui);
    font-weight: var(--strong-weight);
    color: var(--ink);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sub {
    width: 100%;
    font-size: var(--text-label);
    color: var(--ink-3);
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
    background: var(--accent-base);
    color: #fff;
    font-size: 11px;
    font-weight: var(--strong-weight);
    font-variant-numeric: tabular-nums;
  }
  .irow.sel .badge {
    background: rgb(var(--ink-rgb) / 0.2);
    color: var(--ink);
  }
  .more {
    position: absolute;
    right: var(--s-3);
    width: 30px;
    height: 30px;
    display: grid;
    place-items: center;
    border: 0;
    border-radius: 50%;
    background: transparent;
    color: var(--ink-3);
    opacity: 0;
    cursor: pointer;
    transition: opacity var(--dur-fast) var(--ease-out), background-color var(--dur-fast) var(--ease-out);
  }
  .item-wrap:hover .more,
  .item-wrap:focus-within .more,
  .more:focus-visible {
    opacity: 1;
  }
  .more:hover { background: var(--hover); color: var(--ink); }
  .more:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  @media (hover: none) {
    .more { opacity: 1; width: 44px; height: 44px; }
  }
</style>

<script lang="ts">
  /**
   * Declaration-driven sidebar body matching `apps/mockup/src/sidebars/SidebarBody.svelte`
   * and `Section.svelte`: each entry in `sidebars/*.md` is a collapsible section showing
   * its live rows directly on the wallpaper. Zero guessed icons or hardcoded field labels.
   */
  import { Icon } from "@calternal/ui";
  import { Av } from "@calternal/canvas-shell";
  import { formatClock, rowBadge, rowTitle } from "./format";
  import type { CollectionDecl, KindDecl, RowData, SidebarDecl } from "./types";

  let {
    sidebar,
    collections,
    kinds,
    rowsByCollection,
    selectedItemId,
    narrow = false,
    onopenItem,
    onopenCollection,
    onadd,
  }: {
    sidebar: SidebarDecl;
    collections: Record<string, CollectionDecl>;
    kinds: Record<string, KindDecl>;
    rowsByCollection: Record<string, RowData[]>;
    selectedItemId: string | null;
    narrow?: boolean;
    onopenItem: (row: RowData) => void;
    onopenCollection: (collectionId: string) => void;
    onadd: (kind: KindDecl) => void;
  } = $props();

  let collapsed = $state<Record<string, boolean>>({});
  const LIMIT = 6;

  function kindFor(colId: string): KindDecl | null {
    const cid = colId.includes(".") ? colId.split(".")[1]! : colId;
    const col = collections[cid];
    if (!col) return null;
    const kid = col.from.includes(".") ? col.from.split(".")[1]! : col.from;
    return kinds[kid] ?? null;
  }

  function toggleSection(key: string) {
    collapsed = { ...collapsed, [key]: !collapsed[key] };
  }
</script>

<div class="body" class:narrow data-sidebar-body>
  {#each sidebar.entries as entry (entry.collection ?? entry.title)}
    {#if entry.type === "section"}
      <h2 class="sec-div">{entry.title}</h2>
    {:else if entry.collection}
      {@const cid = entry.collection}
      {@const allRows = rowsByCollection[cid] ?? []}
      {@const shown = allRows.slice(0, LIMIT)}
      {@const kind = kindFor(cid)}
      {@const shut = Boolean(collapsed[cid])}
      <section class="sec" data-sec data-cid={cid}>
        <header class="sh">
          <button
            type="button"
            class="tog"
            onclick={() => onopenCollection(cid)}
          >
            <span class="nm" title={entry.title}>{entry.title}</span>
          </button>
          <button
            type="button"
            class="chev-b"
            aria-expanded={!shut}
            aria-label={shut ? `Unfold ${entry.title}` : `Fold ${entry.title}`}
            onclick={() => toggleSection(cid)}
          >
            <span class="chev" class:shut><Icon name="chevron-down" inline /></span>
          </button>
          {#if kind?.operations.add}
            <span class="act">
              <button
                type="button"
                class="ab"
                aria-label={`New ${kind.name}`}
                onclick={() => onadd(kind)}
              >
                <Icon name="plus" inline />
              </button>
            </span>
          {/if}
        </header>

        {#if !shut}
          <div class="rows">
            {#each shown as row (row.id)}
              {@const title = rowTitle(row, kind ?? undefined)}
              {@const badge = rowBadge(row, kind ?? undefined)}
              {@const edited = row.edited ? formatClock(row.edited) : ""}
              <div class="rw" data-row={row.id}>
                <button
                  type="button"
                  class="irow"
                  class:sel={selectedItemId === row.id}
                  aria-current={selectedItemId === row.id ? "true" : undefined}
                  onclick={() => onopenItem(row)}
                >
                  <span class="lead" aria-hidden="true"><Av name={title} size={28} /></span>
                  <span class="rt">{title}</span>
                  {#if edited}<span class="tm">{edited}</span>{/if}
                  {#if badge}<span class="cn" aria-label={`${badge.value} ${badge.name}`}>{badge.value}</span>{/if}
                </button>
              </div>
            {/each}
            {#if !allRows.length}
              <p class="em">Nothing here yet.</p>
            {/if}
            {#if allRows.length > LIMIT}
              <button type="button" class="all" onclick={() => onopenCollection(cid)}>
                Show all {allRows.length}
              </button>
            {/if}
          </div>
        {/if}
      </section>
    {/if}
  {/each}
</div>

<style>
  .body {
    container-type: inline-size;
    position: relative;
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overscroll-behavior: contain;
    scrollbar-width: none;
    padding: var(--s-2) 0 var(--s-10);
    -webkit-mask-image: linear-gradient(to bottom, transparent, #000 var(--s-4), #000 calc(100% - var(--row-h)), transparent);
    mask-image: linear-gradient(to bottom, transparent, #000 var(--s-4), #000 calc(100% - var(--row-h)), transparent);
  }
  .body.narrow {
    --bar: calc(var(--pill-h) + var(--s-9) + var(--tb-h) + var(--s-5) + env(safe-area-inset-bottom, 0px));
    padding-bottom: var(--bar);
  }
  @container (max-width: 230px) {
    .tm, .cn { display: none; }
  }
  .sec-div {
    margin: var(--sb-gap-before) 0 var(--sb-gap-head);
    padding: 0 var(--s-5);
    font-size: var(--text-section);
    font-weight: var(--strong-weight);
    color: var(--ink-3);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .sec { position: relative; border-radius: var(--r-select); }
  .sh {
    position: relative;
    display: flex;
    align-items: center;
    gap: var(--s-1);
    margin-top: var(--sb-gap-before);
    margin-bottom: var(--sb-gap-head);
  }
  .tog {
    flex: 0 1 auto;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: var(--s-3);
    min-height: var(--sb-head-h);
    padding: 0 var(--s-5);
    text-align: start;
    color: var(--ink-2);
    font-size: var(--text-heading);
    line-height: 1.2;
    font-weight: var(--strong-weight);
    border-radius: var(--r-select);
    transition: background-color var(--dur-fast) var(--ease-out), scale var(--dur-fast) var(--ease-out);
  }
  .nm { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .chev-b {
    display: grid;
    place-items: center;
    flex: none;
    width: 24px;
    height: var(--sb-head-h);
    border-radius: var(--r-select);
    color: var(--ink-3);
    transition: background-color var(--dur-fast) var(--ease-out), scale var(--dur-fast) var(--ease-out);
  }
  .chev { display: inline-grid; transition: rotate var(--dur-base) var(--ease-out); }
  .chev.shut { rotate: -90deg; }
  .act {
    position: absolute;
    inset-inline-end: var(--s-2);
    top: 50%;
    translate: 0 -50%;
    display: inline-flex;
    align-items: center;
    border-radius: var(--r-pill);
    background: rgb(var(--tint) / 0.72);
    backdrop-filter: blur(8px);
  }
  .ab {
    flex: none;
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    color: var(--ink-2);
    border-radius: var(--r-select);
    transition: background-color var(--dur-fast) var(--ease-out), scale var(--dur-fast) var(--ease-out);
  }
  @media (hover: hover) and (pointer: fine) {
    .tog:hover, .chev-b:hover, .ab:hover { background: var(--hover); color: var(--ink); }
    .act { opacity: 0; transition: opacity var(--dur-fast) var(--ease-out); }
    .sec:hover .act, .sec:focus-within .act { opacity: 1; }
  }
  .tog:active, .chev-b:active, .ab:active { scale: var(--press-scale); }
  .tog:focus-visible, .chev-b:focus-visible, .ab:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }

  .rows { display: flex; flex-direction: column; gap: 2px; }
  .irow {
    width: 100%;
    display: flex;
    align-items: center;
    gap: var(--lead-gap, var(--s-4));
    min-height: var(--row-h);
    padding: 0 var(--s-5);
    border-radius: var(--r-select);
    color: var(--ink);
    font-size: var(--text-ui);
    text-align: start;
    transition: background-color var(--dur-fast) var(--ease-out), scale var(--dur-fast) var(--ease-out);
  }
  .irow:hover { background: var(--hover); }
  .irow:active { scale: var(--press-scale); }
  .irow.sel { background: var(--accent-soft); font-weight: var(--strong-weight); }
  .irow:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  .lead { flex: none; display: grid; place-items: center; width: var(--lead, 28px); }
  .rt { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tm { flex: none; font-size: var(--text-label); color: var(--ink-2); font-variant-numeric: tabular-nums; }
  .cn {
    flex: none;
    min-width: 18px;
    height: 18px;
    padding: 0 6px;
    border-radius: var(--r-pill);
    display: grid;
    place-items: center;
    background: var(--accent-base);
    color: #fff;
    font-size: var(--text-label);
    font-variant-numeric: tabular-nums;
  }
  .em { margin: 0; padding: var(--s-2) var(--s-5); font-size: var(--text-label); color: var(--ink-3); }
  .all {
    margin: var(--s-2) var(--s-5);
    padding: var(--s-2) var(--s-4);
    border-radius: var(--r-select);
    color: var(--ink-2);
    font-size: var(--text-label);
    text-align: start;
  }
</style>

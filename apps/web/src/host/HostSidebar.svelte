<script lang="ts">
  /**
   * Floating glass sidebar drawn strictly from a plugin's `sidebars/*.md` declaration.
   * Sizes follow `docs/native-ui-study.md` §2.2 & §7.3 (inset 8px, radius 20px, 32px desktop / 48px touch rows).
   */
  import { Icon } from "@calternal/ui";
  import type { KindDecl, SidebarDecl, SidebarEntryDecl } from "./types";

  let {
    sidebar,
    activeCollection,
    counts,
    addKind,
    onselect,
    onadd,
    onsignout,
    accountName,
  }: {
    sidebar: SidebarDecl;
    activeCollection: string | null;
    counts: Record<string, number>;
    addKind: KindDecl | null;
    onselect: (collectionId: string) => void;
    onadd: (kind: KindDecl) => void;
    onsignout: () => void;
    accountName: string;
  } = $props();

  function entryIcon(entry: SidebarEntryDecl): "spark" | "list" | "folder" | "archive" {
    const id = entry.collection?.split(".")[1] ?? "";
    if (id.includes("pin")) return "spark";
    if (id.includes("arch")) return "archive";
    return "list";
  }
</script>

<nav class="side-panel" aria-label={sidebar.title}>
  <header class="side-head">
    <h1 class="side-title">{sidebar.title}</h1>
    {#if addKind}
      <button
        type="button"
        class="add-btn"
        aria-label={`New ${addKind.name}`}
        title={`New ${addKind.name}`}
        onclick={() => onadd(addKind)}
      >
        <Icon name="plus" inline />
      </button>
    {/if}
  </header>

  <ul class="entries" role="list">
    {#each sidebar.entries as entry (entry.collection ?? entry.title)}
      {#if entry.type === "section"}
        <li class="sec-hd">{entry.title}</li>
      {:else if entry.collection}
        {@const selected = activeCollection === entry.collection}
        {@const count = counts[entry.collection]}
        <li>
          <button
            type="button"
            class="row"
            class:sel={selected}
            aria-current={selected ? "page" : undefined}
            onclick={() => onselect(entry.collection!)}
          >
            <span class="ic" aria-hidden="true"><Icon name={entryIcon(entry)} inline /></span>
            <span class="lbl">{entry.title}</span>
            {#if count != null && count > 0}
              <span class="cnt">{count}</span>
            {/if}
          </button>
        </li>
      {/if}
    {/each}
  </ul>

  <footer class="side-foot">
    <span class="who" title={accountName}>{accountName}</span>
    <button type="button" class="out" onclick={onsignout} aria-label="Sign out">
      Sign out
    </button>
  </footer>
</nav>

<style>
  .side-panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    box-sizing: border-box;
    padding: var(--s-5) var(--s-4);
    border-radius: var(--r-card);
    background: rgb(var(--tint) / 0.72);
    backdrop-filter: blur(var(--glass-blur, 16px)) saturate(var(--glass-sat, 180%));
    -webkit-backdrop-filter: blur(var(--glass-blur, 16px)) saturate(var(--glass-sat, 180%));
    box-shadow: var(--glass-edge), var(--shadow-float);
    color: var(--ink);
    overflow: hidden;
  }
  .side-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--s-3);
    padding: var(--s-2) var(--s-3) var(--s-4);
  }
  .side-title {
    margin: 0;
    font-size: var(--text-heading);
    font-weight: var(--strong-weight);
    letter-spacing: -0.01em;
    color: var(--ink);
  }
  .add-btn {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border: 0;
    border-radius: var(--r-pill);
    background: rgb(var(--ink-rgb) / 0.08);
    color: var(--ink);
    cursor: pointer;
    transition: background-color var(--dur-fast) var(--ease-out), scale var(--dur-fast) var(--ease-out);
  }
  .add-btn:hover { background: rgb(var(--ink-rgb) / 0.14); }
  .add-btn:active { scale: var(--press-scale); }
  .add-btn:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }

  .entries {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .sec-hd {
    margin-top: var(--section-above, 16px);
    padding: 0 var(--s-4) var(--section-below, 6px);
    font-size: 11px;
    font-weight: var(--strong-weight);
    color: var(--ink-3);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: var(--s-4);
    min-height: 32px;
    padding: 0 var(--s-4);
    border: 0;
    border-radius: var(--r-select);
    background: transparent;
    color: var(--ink);
    font: inherit;
    font-size: var(--text-ui);
    text-align: start;
    cursor: pointer;
    transition: background-color var(--dur-fast) var(--ease-out), scale var(--dur-fast) var(--ease-out);
  }
  .row:hover { background: var(--hover); }
  .row:active { scale: var(--press-scale); }
  .row:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  .row.sel {
    background: var(--accent-soft);
    color: var(--ink);
    font-weight: var(--strong-weight);
  }
  .ic {
    width: 20px;
    display: grid;
    place-items: center;
    color: var(--ink-3);
    flex: none;
  }
  .row.sel .ic { color: var(--accent); }
  .lbl { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .cnt {
    font-size: var(--text-label);
    color: var(--ink-3);
    font-variant-numeric: tabular-nums;
  }
  .side-foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--s-3);
    padding: var(--s-4) var(--s-3) 0;
    border-top: 1px solid var(--line);
    font-size: var(--text-label);
  }
  .who { color: var(--ink-2); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .out {
    border: 0;
    background: transparent;
    color: var(--ink-3);
    font: inherit;
    padding: var(--s-2) var(--s-3);
    border-radius: var(--r-select);
    cursor: pointer;
    transition: background-color var(--dur-fast) var(--ease-out), color var(--dur-fast) var(--ease-out);
  }
  .out:hover { background: var(--hover); color: var(--ink); }
  .out:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }

  @media (pointer: coarse), (max-width: 759px) {
    .row { min-height: 44px; }
    .add-btn { width: 44px; height: 44px; }
  }
</style>

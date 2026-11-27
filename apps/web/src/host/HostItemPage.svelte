<script lang="ts">
  /**
   * Generic declaration-driven item page for kinds that do not declare records.
   * Built on `@calternal/canvas-shell`'s `ItemPageShell`, rendering `kind.page`
   * (declared fields + Markdown body), primary/menu `perform` operations, generic
   * `change` (Edit), and `Trash` (`remove`). Zero hardcoded kind or app names.
   */
  import { Icon } from "@calternal/ui";
  import { ItemPageShell } from "@calternal/canvas-shell";
  import {
    formatFieldValue,
    humanize,
    itemPrimaryActions,
    readRowValue,
    rowTitle,
    type DerivedAction,
  } from "./format";
  import type { ItemDetail, KindDecl, RowData } from "./types";

  let {
    row,
    detail = null,
    kind,
    onaction,
    onedit,
    onremove,
  }: {
    row: RowData;
    detail?: ItemDetail | null;
    kind: KindDecl;
    onaction: (row: RowData, action: DerivedAction) => void;
    onedit?: (row: RowData, detail: ItemDetail | null, kind: KindDecl) => void;
    onremove?: (row: RowData, kind: KindDecl) => void;
  } = $props();

  const mergedRow = $derived<RowData>(
    detail
      ? {
          ...row,
          fields: { ...row.fields, ...detail.fields },
          computed: { ...row.computed, ...detail.computed },
          state: { ...row.state, ...detail.state },
          when: detail.when ?? row.when,
          version: detail.version,
        }
      : row,
  );

  const title = $derived(rowTitle(mergedRow, kind));
  const primaryActions = $derived(itemPrimaryActions(mergedRow, kind));
  const canEdit = $derived(kind.editable === "outside" || "change" in kind.operations);
  const canRemove = $derived(kind.editable === "outside" || "remove" in kind.operations);

  const fieldEntries = $derived.by(() => {
    const seen = new Set<string>();
    const names: string[] = [];
    for (const entry of kind.page) {
      if (entry.kind === "field" && entry.name && !seen.has(entry.name)) {
        seen.add(entry.name);
        names.push(entry.name);
      } else if (entry.kind === "properties") {
        for (const k of Object.keys(kind.fields)) {
          if (!seen.has(k)) {
            seen.add(k);
            names.push(k);
          }
        }
      }
    }
    if (!names.length) {
      for (const k of Object.keys(kind.fields)) names.push(k);
    }
    return names
      .filter((n) => n !== "title")
      .map((name) => {
        const decl = kind.fields[name];
        const raw = readRowValue(mergedRow, name);
        return { name, label: humanize(name), decl, raw, formatted: formatFieldValue(raw, decl) };
      })
      .filter((e) => e.formatted !== "");
  });

  const showBody = $derived(kind.page.some((p) => p.kind === "body") || Boolean(detail?.body?.trim()));
  const bodyText = $derived(detail?.body?.trim() ?? "");
</script>

<ItemPageShell {title} itemId={row.id}>
  {#snippet actions()}
    {#each primaryActions as act (act.id)}
      <button
        type="button"
        class="act primary"
        data-primary-op={act.operation.name}
        onclick={() => onaction(mergedRow, act)}
      >
        {act.label}
      </button>
    {/each}
    {#if canEdit && onedit}
      <button type="button" class="act" onclick={() => onedit(mergedRow, detail, kind)}>
        <Icon name="edit" inline />
        <span>Edit</span>
      </button>
    {/if}
    {#if canRemove && onremove}
      <button type="button" class="act danger" onclick={() => onremove(mergedRow, kind)}>
        <Icon name="trash" inline />
        <span>Trash</span>
      </button>
    {/if}
  {/snippet}

  {#snippet properties()}
    {#each fieldEntries as entry (entry.name)}
      <dt>{entry.label}</dt>
      <dd data-field={entry.name}>
        {#if entry.decl?.type === "url" && typeof entry.raw === "string"}
          <a href={entry.raw} target="_blank" rel="noopener noreferrer">{entry.raw}</a>
        {:else if Array.isArray(entry.raw)}
          <span class="chips">
            {#each entry.raw as v (String(v))}
              <span class="chip">{formatFieldValue(v)}</span>
            {/each}
          </span>
        {:else if entry.decl?.choices?.length}
          <span class="pill" data-choice={String(entry.raw)}>{entry.formatted}</span>
        {:else}
          {entry.formatted}
        {/if}
      </dd>
    {/each}
  {/snippet}

  {#snippet body()}
    {#if showBody && bodyText}
      {#each bodyText.split(/\n{2,}/) as para, i (i)}
        <p>{para}</p>
      {/each}
    {/if}
  {/snippet}
</ItemPageShell>

<style>
  .act {
    display: inline-flex;
    align-items: center;
    gap: var(--s-2);
    min-height: 32px;
    padding: 0 var(--s-4);
    border: 0;
    border-radius: var(--r-pill);
    background: rgb(var(--ink-rgb) / 0.08);
    color: var(--ink);
    font: inherit;
    font-size: var(--text-label);
    font-weight: var(--strong-weight);
    cursor: pointer;
    transition: background-color var(--dur-fast) var(--ease-out), scale var(--dur-fast) var(--ease-out);
  }
  .act:hover { background: var(--hover); }
  .act:active { scale: var(--press-scale); }
  .act:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .act.primary { background: var(--accent-soft); color: var(--accent); }
  .act.danger { color: var(--danger); }
  .chips { display: inline-flex; flex-wrap: wrap; gap: var(--s-2); }
  .chip, .pill {
    display: inline-flex;
    align-items: center;
    padding: 1px var(--s-3);
    border-radius: var(--r-pill);
    background: rgb(var(--ink-rgb) / 0.08);
    color: var(--ink-2);
    font-size: var(--text-label);
  }
  dd a { color: var(--accent); text-decoration: underline; text-underline-offset: 2px; word-break: break-all; }
</style>

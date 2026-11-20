<script lang="ts">
  // An item opened as a window, drawn with its kind's page look: `records` draws the record file through the records
  // component; anything else (or no page) falls back to the inspector form over the declared fields. Knows no kind.
  import { kindOf, recordsOf, type Row } from "../lib/model";
  import { text, type Action } from "../lib/look";
  import Records from "./Records.svelte";
  let { row, onaction }: { row: Row; onaction: (a: Action, prefill?: Record<string, string>) => void } = $props();
  const k = $derived(kindOf(row.kind) ?? {});
</script>

{#if k.page?.records}
  <Records {row} win={recordsOf(row.item)} page={k.page.records} {onaction} />
{:else}
  <dl class="insp">
    {#each Object.entries(k.fields ?? {}) as [name, f] (name)}
      <dt>{(f as { label?: string }).label ?? name[0]!.toUpperCase() + name.slice(1)}</dt><dd>{text(row.values[name]) || (typeof row.values[name] === "boolean" ? (row.values[name] ? "Yes" : "No") : "—")}</dd>
    {/each}
  </dl>
{/if}

<style>
  .insp { display: grid; grid-template-columns: max-content 1fr; gap: var(--s-4) var(--s-6); margin: 0; padding: var(--s-6); }
  dt { color: var(--ink-3); } dd { margin: 0; }
</style>

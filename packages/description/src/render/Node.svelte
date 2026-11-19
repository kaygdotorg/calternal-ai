<script lang="ts">
  // The content nodes of a view (docs/description-contract.md 5.0): stack, text, notice, button, form, the bound list (single or multiple selection), detail and host components. Each is drawn from what the description says
  // and nothing else; the bind stores, selections and route are the app's (context).
  import type { ContentNode } from "../model";
  import { useApp, useActions } from "../invoke/context";
  import ContentList from "./ContentList.svelte";
  import Node from "./Node.svelte";
  import ActionButton from "./ActionButton.svelte";
  import FormNode from "./FormNode.svelte";
  import HostComponent from "./HostComponent.svelte";
  import { secondaryOf } from "./eval";

  let { node }: { node: ContentNode } = $props();
  const app = useApp(), { session } = useActions();
  /** True when an action's inputs read the ticked set of a multiple-selection list. */
  const setBind = (inputs: Record<string, unknown> | undefined): string | undefined => { for (const v of Object.values(inputs ?? {})) { const s = v as { selection?: string; set?: boolean }; if (s.selection && s.set) return s.selection; } return undefined; };
  const fmt = (v: unknown): string => (v === undefined || v === null ? "" : typeof v === "string" ? v : typeof v === "number" ? (v > 1e9 && v < 1e13 ? secondaryOf([{ field: "v", format: "relative_date" }], { v }) ?? String(v) : String(v)) : String(v));
</script>

{#if node.type === "stack"}
  <div class="stack" class:row={node.axis === "horizontal"} data-node="stack">
    {#if node.title}<h2 class="h">{node.title}</h2>{/if}
    {#each node.children as c, k (k)}<Node node={c} />{/each}
  </div>
{:else if node.type === "text"}
  <p class="text {node.style ?? 'body'} {node.tone ?? ''}" data-node="text">{node.text}</p>
{:else if node.type === "notice"}
  <div class="notice {node.tone ?? 'warn'}" role="status" data-node="notice"><strong>{node.title}</strong>{#if node.detail}<span>{node.detail}</span>{/if}{#if node.action}<ActionButton action={node.action} context={session.viewContexts[node.action.id]} />{/if}</div>
{:else if node.type === "button"}
  {@const sb = setBind(node.invoke.inputs)}
  <div data-node="button"><ActionButton action={node.invoke} context={session.viewContexts[node.invoke.id]} style={node.style ?? "quiet"} selection={sb ? app.selectionFor(sb) : undefined} setOf={sb} /></div>
{:else if node.type === "form"}
  <FormNode {node} />
{:else if node.type === "list"}
  {@const store = app.storeFor(node.bind)}
  {#if store}
    <div class="lw" data-node="list" data-bind={node.bind}>
      {#if node.title}<h3 class="lt">{node.title}</h3>{/if}
      <ContentList {store} selection={app.selectionFor(node.bind)} kindIcon={app.kindIconFor(node.bind)} label={node.title ?? node.bind} multiple={node.selectionMode === "multiple"} />
    </div>
  {/if}
{:else if node.type === "detail"}
  {@const store = app.storeFor(node.bind)}
  <dl class="detail" data-node="detail" data-bind={node.bind}>
    {#if store}{@const row = (store.row(0), store.row(0))}{#each node.fields as f (f.field)}<dt>{f.label}</dt><dd data-field={f.field}>{fmt(row?.fields[f.field])}</dd>{/each}{/if}
  </dl>
{:else if node.type === "component"}
  <HostComponent component={node.component} />
{/if}

<style>
  .stack { display: flex; flex-direction: column; gap: var(--s-4); min-height: 0; flex: 1; }
  .stack.row { flex-direction: row; }
  .h, .lt { margin: 0; font-size: var(--text-ui); font-weight: var(--strong-weight); }
  .text { margin: 0; } .text.title { font-size: var(--text-title, 28px); font-weight: var(--strong-weight); } .text.heading { font-weight: var(--strong-weight); } .text.caption { font-size: var(--text-label); color: var(--ink-3); }
  .good { color: var(--good); } .warn { color: var(--warn); } .bad { color: var(--danger); }
  .notice { display: grid; gap: var(--s-2); padding: var(--s-4) var(--s-5); border-radius: var(--r-select); background: color-mix(in oklab, currentColor 10%, transparent); }
  .lw { display: flex; flex-direction: column; min-height: 0; flex: 1; }
  .detail { display: grid; grid-template-columns: auto 1fr; gap: var(--s-2) var(--s-4); margin: 0; } .detail dt { color: var(--ink-3); } .detail dd { margin: 0; }
</style>

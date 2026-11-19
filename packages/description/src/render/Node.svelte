<script lang="ts">
  // The content nodes of a view (docs/description-contract.md 5.0): stack, text, notice and the bound list. Form, button, detail and component nodes arrive with the invoke step; until then they are drawn as an
  // explicit "not drawn yet" block (never silently dropped), so a fixture that uses them shows what is missing.
  import type { ContentNode } from "../model";
  import type { WindowStore } from "../windows/window-store.svelte";
  import ContentList from "./ContentList.svelte";
  import Node from "./Node.svelte";

  let { node, storeFor, kindIconFor, selectedId, onopen, onsettle }: {
    node: ContentNode; storeFor: (bindId: string) => WindowStore | undefined; kindIconFor: (bindId: string) => string | undefined;
    selectedId?: string | null; onopen?: (bind: string, id: string, index: number) => void; onsettle?: (bind: string, id: string, index: number) => void;
  } = $props();
</script>

{#if node.type === "stack"}
  <div class="stack" class:row={node.axis === "horizontal"} data-node="stack">
    {#if node.title}<h2 class="h">{node.title}</h2>{/if}
    {#each node.children as c, k (k)}<Node node={c} {storeFor} {kindIconFor} {selectedId} {onopen} {onsettle} />{/each}
  </div>
{:else if node.type === "text"}
  <p class="text {node.style ?? 'body'} {node.tone ?? ''}" data-node="text">{node.text}</p>
{:else if node.type === "notice"}
  <div class="notice {node.tone ?? 'warn'}" role="status" data-node="notice"><strong>{node.title}</strong>{#if node.detail}<span>{node.detail}</span>{/if}</div>
{:else if node.type === "list"}
  {@const store = storeFor(node.bind)}
  {#if store}
    <div class="lw" data-node="list" data-bind={node.bind}>
      {#if node.title}<h3 class="lt">{node.title}</h3>{/if}
      <ContentList {store} kindIcon={kindIconFor(node.bind)} label={node.title ?? node.bind} {selectedId} onopen={(id, i) => onopen?.(node.bind, id, i)} onsettle={(id, i) => onsettle?.(node.bind, id, i)} />
    </div>
  {/if}
{:else}
  <div class="todo" data-node-todo={node.type}>{node.type} is not drawn yet</div>
{/if}

<style>
  .stack { display: flex; flex-direction: column; gap: var(--s-4); min-height: 0; flex: 1; }
  .stack.row { flex-direction: row; }
  .h, .lt { margin: 0; font-size: var(--text-ui); font-weight: var(--strong-weight); }
  .text { margin: 0; } .text.title { font-size: var(--text-title, 28px); font-weight: var(--strong-weight); } .text.heading { font-weight: var(--strong-weight); } .text.caption { font-size: var(--text-label); color: var(--ink-3); }
  .good { color: var(--good); } .warn { color: var(--warn); } .bad { color: var(--danger); }
  .notice { display: grid; gap: var(--s-2); padding: var(--s-4) var(--s-5); border-radius: var(--r-select); background: color-mix(in oklab, currentColor 10%, transparent); }
  .lw { display: flex; flex-direction: column; min-height: 0; flex: 1; }
  .todo { padding: var(--s-3) var(--s-5); color: var(--ink-3); font-size: var(--text-label); border: 1px dashed var(--ink-3); border-radius: var(--r-select); }
</style>

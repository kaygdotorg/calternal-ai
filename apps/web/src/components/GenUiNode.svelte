<script lang="ts">
  import { checkAction } from '../lib/genui';
  import { hrefFor } from '../lib/route';
  import { app } from '../lib/store.svelte';
  import GenUiNode from './GenUiNode.svelte';
  import GenUiView from './GenUiView.svelte';
  import DocEmbed from './DocEmbed.svelte';

  let { node, ctx, depth = 0 }: { node: any; ctx: any; depth?: number } = $props();

  const blocked = $derived(node.type === 'button' ? checkAction(ctx.policy, node.action?.operation) : null);
  const res = $derived(node.type === 'button' ? ctx.result(node.id) : undefined);
  const spaceOk = $derived(node.type === 'resource' || node.type === 'document-editor' ? !!app.roleIn(node.space_id) : false);
</script>

{#if node.type === 'stack'}
  <div class="stack" data-node-id={node.id}>
    {#each node.children as child (child.id)}<GenUiNode node={child} {ctx} depth={depth + 1} />{/each}
  </div>
{:else if node.type === 'text'}
  <p class="text" data-node-id={node.id}>{node.text}</p>
{:else if node.type === 'stat'}
  <div class="stat" data-node-id={node.id}><span class="v">{node.value}</span><span class="l">{node.label}</span></div>
{:else if node.type === 'button'}
  <div class="btnwrap" data-node-id={node.id}>
    <button class="btn" disabled={!!blocked || res?.busy} aria-describedby={blocked ? `${node.id}-why` : undefined}
      onclick={() => ctx.run(node.id, node.action.operation, node.action.input)}>
      {res?.busy ? 'Running…' : node.label}
    </button>
    {#if blocked}<span id="{node.id}-why" class="why muted">{blocked}</span>{/if}
    {#if res?.error}<div class="banner err" role="alert">{res.error}</div>{/if}
    {#if res?.data}
      <div class="out" aria-live="polite">
        {#if res.data.view}<GenUiView view={res.data.view} policy={ctx.policy} context={ctx.context} depth={depth + 1} />{/if}
      </div>
    {/if}
  </div>
{:else if node.type === 'resource'}
  <div data-node-id={node.id}>
    {#if spaceOk}<a class="reslink" href={hrefFor({ name: 'notes', space: node.space_id, id: node.resource_id })}>{node.label}</a>
    {:else}<span class="reslink off" title="You do not have access to this space">{node.label} <span class="chip">no access</span></span>{/if}
  </div>
{:else if node.type === 'document-editor'}
  <div data-node-id={node.id}>
    {#if spaceOk}<DocEmbed space={node.space_id} id={node.resource_id} />
    {:else}<div class="banner">Document not available to your principal.</div>{/if}
  </div>
{:else}
  <div class="banner unsupported" data-node-id={String(node?.id ?? '')}>
    Unsupported element “{String(node?.type).slice(0, 40)}”. This client cannot render it, and it cannot trigger actions.
  </div>
{/if}

<style>
  .stack { display: flex; flex-wrap: wrap; gap: 10px; align-items: flex-start; }
  .stack > :global(*) { min-width: 0; }
  .stack > :global(p), .stack > :global(.btnwrap), .stack > :global(.banner), .stack > :global(.stack), .stack > :global(.out) { flex-basis: 100%; }
  .text { margin: 0; white-space: pre-wrap; overflow-wrap: anywhere; }
  .stat { display: inline-grid; gap: 2px; padding: 10px 14px; border-radius: 12px; background: var(--card-2); border: 1px solid var(--line); min-width: 96px; justify-self: start; }
  .v { font-size: 22px; font-weight: 650; font-variant-numeric: tabular-nums; }
  .l { font-size: 12px; color: var(--text-2); }
  .btnwrap { display: grid; gap: 6px; justify-items: start; }
  .why { font-size: 12px; }
  .out { width: 100%; }
  .reslink { color: var(--accent); }
  .reslink.off { color: var(--text-2); }
</style>

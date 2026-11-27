<script lang="ts">
  /**
   * Shared item-page typography and property grid from `apps/mockup/src/canvas/ItemPage.svelte`.
   * Provides the exact `--text-title` heading, 2-column `dl` property list, action bar,
   * and Markdown body spacing inside a canvas window card.
   */
  import type { Snippet } from "svelte";

  let {
    title,
    subtitle = "",
    itemId,
    actions,
    properties,
    body,
  }: {
    title: string;
    subtitle?: string;
    itemId?: string;
    actions?: Snippet;
    properties?: Snippet;
    body?: Snippet;
  } = $props();
</script>

<article class="ip" data-item-page={itemId}>
  <header class="head">
    <div class="titles">
      <h2>{title}</h2>
      {#if subtitle}<p class="m">{subtitle}</p>{/if}
    </div>
    {#if actions}
      <div class="acts">{@render actions()}</div>
    {/if}
  </header>
  {#if properties}
    <dl class="props">{@render properties()}</dl>
  {/if}
  {#if body}
    <div class="bd">{@render body()}</div>
  {/if}
</article>

<style>
  .ip { display: grid; gap: var(--s-5); align-content: start; max-width: 44rem; width: 100%; margin: 0 auto; padding: var(--s-4) var(--s-7) var(--s-8); box-sizing: border-box; }
  .head { display: flex; align-items: flex-start; justify-content: space-between; gap: var(--s-4); flex-wrap: wrap; }
  .titles { display: grid; gap: var(--s-1); min-width: 0; flex: 1; }
  h2 { margin: 0; font-size: var(--text-title); line-height: 1.2; font-weight: var(--strong-weight); color: var(--ink); overflow-wrap: anywhere; }
  .m { margin: 0; color: var(--ink-2); font-size: var(--text-label); line-height: 1.3; }
  .acts { display: flex; flex-wrap: wrap; align-items: center; gap: var(--s-2); }
  .props { display: grid; grid-template-columns: max-content minmax(0, 1fr); gap: var(--s-3) var(--s-6); margin: 0; padding: var(--s-4) var(--s-5); border-radius: var(--r-card); background: rgb(var(--ink-rgb) / 0.04); align-items: baseline; }
  .props :global(dt) { color: var(--ink-2); font-size: var(--text-label); font-weight: var(--strong-weight); }
  .props :global(dd) { margin: 0; color: var(--ink); font-size: var(--text-ui); min-width: 0; overflow-wrap: anywhere; }
  .bd { display: grid; gap: var(--s-3); font-size: var(--text-ui); line-height: 1.55; color: var(--ink); }
  .bd :global(p) { margin: 0; }
  .bd :global(ul), .bd :global(ol) { margin: 0; padding-inline-start: var(--s-8); }
  .bd :global(code) { padding: 1px 5px; border-radius: var(--r-select); background: rgb(var(--ink-rgb) / 0.07); font-family: var(--font-mono); }
  .bd :global(pre) { margin: 0; padding: var(--s-4); border-radius: var(--r-menu); background: rgb(var(--ink-rgb) / 0.07); overflow-x: auto; }
  .bd :global(a) { color: var(--accent); text-decoration: underline; text-underline-offset: 2px; }
  @media (max-width: 719px) { .ip { padding: var(--s-3) var(--s-5) var(--s-8); } }
</style>

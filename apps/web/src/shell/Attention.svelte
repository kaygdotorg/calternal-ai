<script lang="ts">
  // The attention card: what needs the person, at most three items. A neutral glass panel (a card because it asks for attention, but not tinted: colour is for
  // state, and this is set in the same face and size as every other section label). Absent when empty.
  import { Badge, Icon, Panel } from "@calternal/ui";
  import type { AttentionItem } from "../apps/registry";
  import { href } from "../lib/route.svelte";
  let { items }: { items: AttentionItem[] } = $props();
  const shown = $derived(items.slice(0, 3));
</script>

{#if shown.length}
  <Panel class="attn" radius="card" role="region" aria-label="Needs you">
    <h2>Needs you {#if items.length > 3}<Badge text={String(items.length)} />{/if}</h2>
    <ul>
      {#each shown as it (it.id)}
        <li><a href={href(it.href)}><span class="ic"><Icon name={it.icon} /></span><span class="tx"><span class="t">{it.title}</span><span class="s">{it.sub}</span></span></a></li>
      {/each}
    </ul>
  </Panel>
{/if}

<style>
  :global(.attn) { padding: var(--s-3); }
  h2 { display: flex; align-items: center; gap: var(--s-3); padding: var(--s-3) var(--s-4) var(--s-2); font-size: var(--text-label); font-weight: var(--strong-weight); color: var(--ink-3); }
  ul { display: grid; gap: var(--s-1); }
  a { display: flex; align-items: center; gap: var(--s-4); min-height: var(--row-h); padding: var(--s-2) var(--s-4); border-radius: var(--r-select); transition: background-color var(--dur-fast) var(--ease-out), scale var(--dur-fast) var(--ease-out); }
  a:active { scale: var(--press-scale); }
  @media (hover: hover) and (pointer: fine) { a:hover { background: var(--hover); } }
  .ic { color: var(--ink-2); display: grid; }
  .tx { display: grid; min-width: 0; }
  .t, .s { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .t { font-size: var(--text-ui); font-weight: var(--strong-weight); }
  .s { font-size: var(--text-label); color: var(--ink-3); }
</style>

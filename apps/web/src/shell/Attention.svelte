<script lang="ts">
  // The attention card: what needs the person, at most three items, a card because it asks for attention. Absent when empty.
  import { Badge, Card, Icon } from "@calternal/ui";
  import type { AttentionItem } from "../apps/registry";
  let { items }: { items: AttentionItem[] } = $props();
  const shown = $derived(items.slice(0, 3));
</script>

{#if shown.length}
  <Card label="Needs your attention" variant="attention" class="attn">
    <h2>Needs you {#if items.length > 3}<Badge text={String(items.length)} tone="accent" />{/if}</h2>
    <ul>
      {#each shown as it (it.id)}
        <li><a href={it.href}><span class="ic"><Icon name={it.icon} /></span><span class="tx"><span class="t">{it.title}</span><span class="s">{it.sub}</span></span></a></li>
      {/each}
    </ul>
  </Card>
{/if}

<style>
  :global(.attn) { padding: var(--s-3); }
  h2 { display: flex; align-items: center; gap: var(--s-3); padding: var(--s-3) var(--s-4) var(--s-2); font-size: var(--text-section); font-weight: var(--strong-weight); letter-spacing: 0.05em; text-transform: uppercase; color: var(--accent); }
  ul { display: grid; gap: var(--s-1); }
  a { display: flex; align-items: center; gap: var(--s-4); min-height: var(--row-h); padding: var(--s-2) var(--s-4); border-radius: calc(var(--r-card) - var(--s-3)); transition: background-color var(--dur-fast) var(--ease-out), scale var(--dur-fast) var(--ease-out); }
  a:active { scale: var(--press-scale); }
  @media (hover: hover) and (pointer: fine) { a:hover { background: color-mix(in oklab, var(--accent-base) 12%, transparent); } }
  .ic { color: var(--accent); display: grid; }
  .tx { display: grid; min-width: 0; }
  .t, .s { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .t { font-size: var(--text-ui); font-weight: var(--strong-weight); }
  .s { font-size: var(--text-label); color: var(--ink-3); }
</style>

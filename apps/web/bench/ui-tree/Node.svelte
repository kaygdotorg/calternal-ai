<script lang="ts">
  import { Row, VirtualList, Panel, Button } from "@calternal/ui";
  import Node from "./Node.svelte";
  import type { Node as N } from "./protocol";
  import { store } from "./store.svelte";
  let { id }: { id: string } = $props();
  const n = $derived(store.nodes[id] as N | undefined);
</script>

{#if n}
  {#if n.type === "stack"}
    <div class="stack">{#each store.kids[id] ?? [] as c (c)}<Node id={c} />{/each}</div>
  {:else if n.type === "strip"}
    <div class="strip">{#each store.kids[id] ?? [] as c (c)}<Node id={c} />{/each}</div>
  {:else if n.type === "window"}
    <Panel radius="card" class="win" style="width:{n.props.w}px"><h3>{n.props.title}</h3>{#each store.kids[id] ?? [] as c (c)}<Node id={c} />{/each}</Panel>
  {:else if n.type === "static-list"}
    <div class="sl">{#each Array.from({ length: n.props.n as number }) as _, i (i)}<Row title="Static row {i}" sub="a quiet line" icon="file" />{/each}</div>
  {:else if n.type === "vlist"}
    <div class="vl"><VirtualList count={store.counts[id] ?? (n.props.count as number)} estimate={44} overscan={400} label={n.props.label as string} startAtBottom={false} onrange={(a, b) => store.range(id, a, b)}>
      {#snippet row(i)}{@const r = store.rows[id]?.[i]}{#if r}<Row title={r.title} sub={r.sub} icon="file" />{:else}<Row title="…" />{/if}{/snippet}
    </VirtualList></div>
  {:else if n.type === "text"}
    <p class="msg">{n.props.text}</p>
  {:else if n.type === "button"}
    <Button text={n.props.label as string} variant="secondary" />
  {/if}
{/if}

<style>
  .stack { display: grid; grid-template-rows: auto 1fr auto; height: 100vh; gap: 8px; padding: 8px; box-sizing: border-box; }
  .strip { display: flex; gap: 8px; overflow: hidden; height: 360px; }
  .strip :global(.win) { flex: none; height: 100%; overflow: hidden; padding: 8px; box-sizing: border-box; }
  .vl { min-height: 0; position: relative; overflow: hidden; }
  .vl :global(.vl-scroll), .vl > :global(*) { height: 100%; }
  .msg { margin: 0; font: 15px/1.4 system-ui; max-height: 4.2em; overflow: hidden; white-space: pre-wrap; overflow-wrap: anywhere; }
  h3 { margin: 0 0 6px; font: 600 14px system-ui; }
  .sl { overflow: hidden; height: 250px; }
</style>

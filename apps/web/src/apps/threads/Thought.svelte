<script lang="ts">
  // A turn's reasoning as its own folded block, "Thought for 4 s": quiet row that opens in place into the reasoning text (muted, selectable). While the provider is still reasoning (`running`) the row pulses and
  // says "Thinking". A provider that shares only that it thought, not what, has an empty `text`: the row then has nothing to open and shows no chevron.
  import { untrack } from "svelte";
  import { Icon } from "@calternal/ui";
  import Md from "../../lib/Md.svelte";
  import "./block-head.css";
  let { text, ms, running = false }: { text: string; ms: number; running?: boolean } = $props();
  let open = $state(untrack(() => running && !!text));
  const secs = $derived(Math.max(1, Math.round(ms / 1000)));
  const took = $derived(secs >= 60 ? `${Math.floor(secs / 60)} min ${secs % 60} s` : `${secs} s`);
  const uid = $props.id();
</script>

<div class="thought" class:open>
  <button type="button" class="head block-head" aria-expanded={text ? open : undefined} aria-controls="{uid}-t" disabled={!text} onclick={() => (open = !open)}>
    <span class="ic"><Icon name="spark" inline /></span>
    <span class="sum" class:pulse={running}>{running ? "Thinking" : `Thought for ${took}`}</span>
    {#if text}<span class="chev"><Icon name="chevron" inline /></span>{/if}
  </button>
  {#if text}<div class="body" id="{uid}-t" inert={!open}><div class="in selectable"><Md {text} /></div></div>{/if}
</div>

<style>
  .thought { display: grid; border-radius: var(--r-card); background: rgb(var(--ink-rgb) / 0.05); overflow: clip; }
  .ic { display: grid; width: var(--s-7); place-items: center; }
  .sum { flex: 1; font-variant-numeric: tabular-nums; }
  .chev { display: grid; transition: rotate var(--dur-base) var(--ease-out); }
  .open .chev { rotate: 90deg; }
  .body { display: grid; grid-template-rows: 0fr; transition: grid-template-rows var(--dur-base) var(--ease-out); }
  .open .body { grid-template-rows: 1fr; }
  .in { min-height: 0; overflow: clip; color: var(--ink-3); font-size: var(--text-label); padding: 0 var(--s-5); }
  .open .in { padding-bottom: var(--s-4); }
  @media (hover: hover) and (pointer: fine) { .head:not(:disabled):hover { background: var(--hover); } }
</style>

<script lang="ts">
  // The card of a thread row's press (lib/press.ts): the thread's title and state and its last messages, as the open thread draws them but quieter, from what was
  // already read (`recent` in conversation.svelte.ts): nothing is fetched on a press, and a thread never opened says so. Registered as the `thread` preview.
  import { recent } from "./conversation.svelte";
  import { store } from "./store.svelte";
  import type { Block } from "./model";
  let { item }: { item: string } = $props();
  const t = $derived(store.get(item));
  const msgs = $derived(recent.get(item) ?? []);
  const text = (blocks: Block[]) => blocks.filter((b): b is Extract<Block, { type: "text" }> => b.type === "text").map((b) => b.text).join("\n\n").replace(/[#*`>_]/g, "").trim();
</script>

<div class="pv">
  <p class="ti">{t?.title ?? "Thread"}</p>
  {#each msgs as m (m.id)}
    {@const x = text(m.blocks)}
    {#if x}<p class="m {m.role}">{x.length > 280 ? x.slice(0, 280) + "…" : x}</p>{/if}
  {:else}
    <p class="none">Not opened yet: open it to read it.</p>
  {/each}
</div>

<style>
  .pv { display: grid; gap: var(--s-3); padding: var(--s-5) var(--s-5) var(--s-6); font-size: var(--text-label); color: var(--ink-2); }
  .ti { color: var(--ink); font-size: var(--text-ui); font-weight: var(--strong-weight); }
  .m { line-height: 1.45; display: -webkit-box; -webkit-line-clamp: 4; line-clamp: 4; -webkit-box-orient: vertical; overflow: hidden; }
  .m.user { justify-self: end; max-width: 88%; padding: var(--s-2) var(--s-4); border-radius: var(--r-card); background: rgb(var(--ink-rgb) / 0.07); color: var(--ink); }
  .none { color: var(--ink-3); }
</style>

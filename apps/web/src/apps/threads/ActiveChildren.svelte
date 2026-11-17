<script lang="ts">
  // A thin bar above the composer with one segment per child thread that is still running. It brightens when the pointer is over it (or a segment has focus),
  // each segment links to its thread, and a tooltip says how many are active. Running segments pulse calmly. Nothing shows when no child is running.
  import { Icon, Panel, tip } from "@calternal/ui";
  import { href } from "../../lib/route.svelte";
  import { store } from "./store.svelte";
  let { id }: { id: string } = $props();
  const active = $derived(store.threads.filter((t) => t.parentId === id && t.state === "running"));
  const label = $derived(`${active.length} active child ${active.length === 1 ? "thread" : "threads"}`);
</script>

{#if active.length}
  <div class="wrap" use:tip={{ label }}>
    <Panel radius="pill" dense class="kids" role="group" aria-label={label}>
      <Icon inline name="branch" />
      {#each active as t (t.id)}<a class="seg pulse" href={href(`/threads/${t.id}`)} aria-label={t.title}></a>{/each}
      <span class="n">{active.length}</span>
    </Panel>
  </div>
{/if}

<style>
  /* A glass pill of its own (a bare bar over the scrolling messages read as a stray underline): one segment per running child, brighter on hover. */
  .wrap { margin: 0 var(--s-6) var(--s-2); width: fit-content; }
  :global(.kids) { background: rgb(var(--tint) / 0.92); display: inline-flex; align-items: center; gap: var(--s-2); height: var(--pill-h); padding: 0 var(--s-5); opacity: 0.8; transition: opacity var(--dur-base) var(--ease-out); }
  :global(.kids:hover), :global(.kids:focus-within) { opacity: 1; }
  /* The link is a full-size target (the pill height); the bar drawn in it is thin. */
  .n { font-size: var(--text-label); font-variant-numeric: tabular-nums; color: var(--ink-2); }
  .seg { width: max(2rem, var(--pill-h)); height: var(--pill-h); display: grid; align-items: center; }
  .seg::before { content: ""; height: var(--s-2); border-radius: var(--r-pill); background: var(--accent); }
</style>

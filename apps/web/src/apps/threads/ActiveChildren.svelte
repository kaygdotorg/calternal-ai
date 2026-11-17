<script lang="ts">
  // A thin bar above the composer with one segment per child thread that is still running. It brightens when the pointer is over it (or a segment has focus),
  // each segment links to its thread, and a tooltip says how many are active. Running segments pulse calmly. Nothing shows when no child is running.
  import { tip } from "@calternal/ui";
  import { href } from "../../lib/route.svelte";
  import { store } from "./store.svelte";
  let { id }: { id: string } = $props();
  const active = $derived(store.threads.filter((t) => t.parentId === id && t.state === "running"));
  const label = $derived(`${active.length} active child ${active.length === 1 ? "thread" : "threads"}`);
</script>

{#if active.length}
  <div class="kids" role="group" aria-label={label} use:tip={{ label }}>
    {#each active as t (t.id)}<a class="seg pulse" href={href(`/threads/${t.id}`)} aria-label={t.title}></a>{/each}
  </div>
{/if}

<style>
  .kids { display: flex; gap: var(--s-2); padding: 0 var(--s-6); opacity: 0.55; transition: opacity var(--dur-base) var(--ease-out); }
  .kids:hover, .kids:focus-within { opacity: 1; }
  /* The link is a full-size target (the pill height); the bar drawn in it is thin. */
  .seg { flex: 1; max-width: 6rem; height: var(--pill-h); display: grid; align-items: center; }
  .seg::before { content: ""; height: var(--s-2); border-radius: var(--r-pill); background: var(--accent); }
  .seg:focus-visible { outline-offset: 2px; }
</style>

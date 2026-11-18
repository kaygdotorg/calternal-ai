<script lang="ts">
  // A chip in the composer's context row with one segment per child thread that is still running. It brightens when the pointer is over it (or a segment has focus),
  // each segment links to its thread, and a tooltip says how many are active. Running segments pulse calmly. Nothing shows when no child is running.
  import { Icon, tip } from "@calternal/ui";
  import { href } from "../../lib/route.svelte";
  import { store } from "./store.svelte";
  let { id }: { id: string } = $props();
  const active = $derived(store.threads.filter((t) => t.parentId === id && t.state === "running"));
  const label = $derived(`${active.length} active child ${active.length === 1 ? "thread" : "threads"}`);
</script>

{#if active.length}
  <!-- A chip in the composer's context row, not a pill of its own floating over the messages above it: nothing is blurred under it, and the list's reserved room (the composer's height) already covers it. -->
  <span class="kids" role="group" aria-label={label} use:tip={{ label }}>
    <Icon inline name="branch" />
    {#each active as t (t.id)}<a class="seg pulse" href={href(`/threads/${t.id}`)} aria-label={t.title}></a>{/each}
    <span class="n">{active.length}</span>
  </span>
{/if}

<style>
  /* The same chip anatomy as the composer's other context chips (a tinted pill, label size); one segment per running child, brighter on hover. */
  .kids { display: inline-flex; align-items: center; gap: var(--s-2); padding: 0 var(--s-4); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.06); color: var(--ink-2); font-size: var(--text-label); }
  .kids:hover, .kids:focus-within { background: rgb(var(--ink-rgb) / 0.1); }
  .n { font-variant-numeric: tabular-nums; }
  /* Each segment is a link with a full-size target; the bar drawn in it is thin. */
  .seg { width: 1.5rem; height: var(--row-h); display: grid; align-items: center; border-radius: var(--r-pill); }
  .seg::before { content: ""; height: var(--s-2); border-radius: var(--r-pill); background: var(--accent); }
</style>

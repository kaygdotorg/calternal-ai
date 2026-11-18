<script lang="ts">
  // Who else is in the note right now: up to three avatars and a count, one control-height strip in the note's header. The names are in the accessible label and the tooltip, never only in colour. Nothing is drawn when the
  // person is alone.
  import { Avatar, tip } from "@calternal/ui";
  import type { Here } from "./live.svelte";
  let { here }: { here: Here[] } = $props();
  const shown = $derived(here.slice(0, 3));
  const label = $derived(`Here now: ${here.map((h) => h.name).join(", ")}`);
</script>

{#if here.length}
  <!-- A press shows every name beside the cursors until the next press elsewhere (a touch screen has no hover); the editor listens for it. -->
  <button type="button" class="presence" data-presence aria-label="{label}. Show their names" use:tip={{ label, side: "bottom" }} onclick={() => dispatchEvent(new Event("calternal-live-labels"))}>
    {#each shown as h (h.client)}<span class="p" style:--c={h.color}><Avatar name={h.name} size="xs" /></span>{/each}
    {#if here.length > shown.length}<span class="more">+{here.length - shown.length}</span>{/if}
  </button>
{/if}

<style>
  .presence { display: inline-flex; align-items: center; flex: none; border-radius: var(--r-pill); min-height: var(--pill-h); padding: 0 var(--s-2); }
  .p { display: inline-grid; border-radius: 50%; box-shadow: 0 0 0 2px var(--c); margin-left: calc(var(--s-2) * -1); }
  .p:first-child { margin-left: 0; }
  .more { margin-left: var(--s-2); font-size: var(--text-label); color: var(--ink-3); }
</style>

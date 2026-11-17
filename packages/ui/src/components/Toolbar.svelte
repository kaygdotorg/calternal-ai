<script lang="ts">
  // One pill holding its buttons (IconButton, Pill with glass={false}): a pill within a pill, concentric. Arrow keys move between
  // the buttons (roving focus). `label` is required: a toolbar without a name is unreachable to screen readers.
  import type { Snippet } from "svelte";
  import Glass from "./Glass.svelte";
  import { adaptive } from "../lib/material";
  import { roving } from "../lib/roving";
  let { label, glass = true, children, class: klass }: { label: string; glass?: boolean; children: Snippet; class?: string } = $props();
</script>

<div class={["tb", klass]} role="toolbar" aria-label={label} use:adaptive use:roving={{ selector: "button:not(:disabled), a[href]" }}>
  {#if glass}<Glass />{/if}
  {@render children()}
</div>

<style>
  .tb { position: relative; isolation: isolate; display: inline-flex; align-items: center; gap: var(--s-1); padding: var(--s-2); border-radius: var(--r-pill); color: var(--ink); }
</style>

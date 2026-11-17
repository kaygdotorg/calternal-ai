<script lang="ts">
  // A card: a landmark region with a required `label`. `glass` cards blur and dim whatever is behind them (the shell's middle and right
  // cards); `solid` cards sit on paper; `attention` is the tinted card that asks for the user's attention. Its radius is a token.
  import type { Snippet } from "svelte";
  import Glass from "./Glass.svelte";
  import { adaptive } from "../lib/material";
  let {
    label,
    variant = "glass",
    children,
    class: klass,
    ...rest
  }: { label: string; variant?: "glass" | "solid" | "attention"; children: Snippet; class?: string } & Record<string, unknown> = $props();
</script>

<section class={["card", klass]} data-variant={variant} aria-label={label} use:adaptive={"card"} {...rest}>
  {#if variant === "glass"}<Glass flat />{/if}
  {@render children()}
</section>

<style>
  .card { position: relative; isolation: isolate; border-radius: var(--r-pane); min-width: 0; min-height: 0; }
  .card[data-variant="glass"] { box-shadow: var(--shadow-card); }
  .card[data-variant="solid"] { background: var(--paper); box-shadow: var(--shadow-card); }
  .card[data-variant="attention"] { background: color-mix(in oklab, var(--accent-base) 12%, rgb(var(--tint) / 0.6)); border-radius: var(--r-card); box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--accent-base) 22%, transparent); }
</style>

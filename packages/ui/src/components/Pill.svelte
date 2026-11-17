<script lang="ts">
  // A pill: a round button on its own glass (`glass`, the default) or a quiet tint. Icon only, or icon + visible label (`showLabel`).
  // `label` is required: it is the accessible name either way. `children` replaces the visible text (several lengths of it, say) while `label` stays the full name.
  import type { HTMLButtonAttributes } from "svelte/elements";
  import Glass from "./Glass.svelte";
  import Icon from "./Icon.svelte";
  import { adaptive } from "../lib/material";
  import { tip } from "../lib/tooltip";
  import type { IconName } from "../icons";
  let {
    label,
    icon,
    showLabel = false,
    glass = true,
    children,
    el = $bindable(),
    class: klass,
    ...rest
  }: { el?: HTMLButtonElement; label: string; icon?: IconName; showLabel?: boolean; glass?: boolean; children?: import("svelte").Snippet; class?: string } & Omit<HTMLButtonAttributes, "aria-label"> = $props();
</script>

<button bind:this={el} type="button" class={["pill", klass]} class:wide={showLabel} class:flat={!glass} aria-label={showLabel && !children ? undefined : label} use:adaptive use:tip={{ label: showLabel && !children ? "" : label }} {...rest}>
  {#if glass}<Glass />{/if}
  {#if icon}<Icon name={icon} />{/if}
  {#if showLabel}{#if children}{@render children()}{:else}<span>{label}</span>{/if}{/if}
</button>

<style>
  .pill {
    position: relative; isolation: isolate; display: inline-flex; align-items: center; justify-content: center; gap: var(--s-3); flex: none;
    min-width: var(--pill-h); height: var(--pill-h); padding: 0; border-radius: var(--r-pill);
    color: var(--ink); font-size: var(--text-ui); font-weight: var(--strong-weight); white-space: nowrap;
    transition: scale var(--dur-fast) var(--ease-out), background-color var(--dur-fast) var(--ease-out);
  }
  .wide { padding: 0 var(--s-6) 0 var(--s-5); }
  .flat { background: rgb(var(--ink-rgb) / 0.065); }
  .pill:active:not(:disabled) { scale: var(--press-scale); }
  .pill:disabled { opacity: 0.4; }
  .pill[aria-expanded="true"], .pill[aria-pressed="true"] { background: rgb(var(--ink-rgb) / 0.1); }
</style>

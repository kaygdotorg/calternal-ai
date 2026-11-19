<script lang="ts">
  // A text button: a pill filled with a tint, never an outlined box. `text` is required and is the accessible name.
  import type { HTMLButtonAttributes } from "svelte/elements";
  import Icon from "./Icon.svelte";
  import Spinner from "./Spinner.svelte";
  import type { IconName } from "../icons";
  let {
    text,
    icon,
    variant = "secondary",
    busy = false,
    class: klass,
    ...rest
  }: { text: string; icon?: IconName; variant?: "primary" | "secondary" | "quiet" | "destructive"; /** Work started by this button is in progress: a ring grows in before the label (the width eases open) and the button stays at full strength, disabled. */ busy?: boolean; class?: string } & Omit<HTMLButtonAttributes, "children"> = $props();
</script>

<button type="button" class={["btn", variant, klass]} aria-busy={busy || undefined} {...rest} disabled={rest.disabled || busy}>
  {#if busy}<span class="bz"><Spinner /></span>{:else if icon}<Icon name={icon} />{/if}<span>{text}</span>
</button>

<style>
  .btn {
    display: inline-flex; align-items: center; justify-content: center; gap: var(--s-3); white-space: nowrap;
    min-height: var(--pill-h); padding: 0 var(--s-6); border-radius: var(--r-pill);
    font-size: var(--text-ui); font-weight: var(--strong-weight);
    transition: scale var(--dur-fast) var(--ease-out), background-color var(--dur-fast) var(--ease-out), color var(--dur-fast) var(--ease-out);
  }
  .btn:active:not(:disabled) { scale: var(--press-scale); }
  .btn:disabled { opacity: 0.45; }
  .btn[aria-busy="true"]:disabled { opacity: 1; }
  /* The ring grows in before the label: width and opacity over `--dur-base`; the gap of the flex row eases in with it. */
  .bz { display: inline-grid; place-items: center; overflow: hidden; animation: bz-in var(--dur-base) var(--ease-out) backwards; }
  @keyframes bz-in { from { width: 0; opacity: 0; margin-inline-end: calc(var(--s-3) * -1); } }
  .primary { background: var(--accent-base); color: #fff; }
  .primary:not(:disabled):hover { background: color-mix(in oklab, var(--accent-base) 90%, black); }
  .secondary { background: rgb(var(--ink-rgb) / 0.07); color: var(--ink); }
  .quiet { color: var(--ink-2); }
  .destructive { background: color-mix(in oklab, var(--danger) 13%, transparent); color: var(--danger); }
  @media (hover: hover) and (pointer: fine) {
    .secondary:not(:disabled):hover { background: rgb(var(--ink-rgb) / 0.1); }
    .quiet:not(:disabled):hover { background: var(--hover); color: var(--ink); }
    .destructive:not(:disabled):hover { background: color-mix(in oklab, var(--danger) 20%, transparent); }
  }
</style>

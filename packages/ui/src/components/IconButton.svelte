<script lang="ts">
  // A round button holding one icon. `small` is the bare in-text tier (hover actions beside a message or row): 24 px, the inline glyph, no pill. `label` is required: it is the accessible name and the tooltip.
  import type { HTMLButtonAttributes } from "svelte/elements";
  import Icon from "./Icon.svelte";
  import { tip } from "../lib/tooltip";
  import type { IconName } from "../icons";
  let {
    icon,
    label,
    pressed,
    tone = "quiet",
    small = false,
    el = $bindable(),
    class: klass,
    ...rest
  }: { icon: IconName; label: string; pressed?: boolean; tone?: "quiet" | "tint" | "accent"; small?: boolean; el?: HTMLButtonElement; class?: string } & Omit<HTMLButtonAttributes, "aria-label" | "title"> = $props();
</script>

<button bind:this={el} type="button" class={["ib", tone, klass]} class:small aria-label={label} use:tip={{ label }} aria-pressed={pressed} {...rest}>
  <Icon name={icon} inline={small} />
</button>

<style>
  .ib {
    position: relative; flex: none; display: inline-grid; place-items: center;
    width: var(--ib, var(--pill-h)); height: var(--ib, var(--pill-h));
    border-radius: var(--r-pill); color: var(--ink-2);
    transition: scale var(--dur-fast) var(--ease-out), background-color var(--dur-fast) var(--ease-out), color var(--dur-fast) var(--ease-out);
  }
  .ib:active:not(:disabled) { scale: var(--press-scale); }
  .ib:disabled { opacity: 0.4; }
  .small { --ib: var(--ib-small); }
  .tint { background: rgb(var(--ink-rgb) / 0.065); }
  .accent { background: var(--accent-base); color: #fff; }
  @media (hover: hover) and (pointer: fine) {
    .ib.quiet:not(:disabled):hover, .ib.tint:not(:disabled):hover { background: rgb(var(--ink-rgb) / 0.09); color: var(--ink); }
  }
  .ib[aria-pressed="true"] { background: rgb(var(--ink-rgb) / 0.12); color: var(--ink); }
</style>

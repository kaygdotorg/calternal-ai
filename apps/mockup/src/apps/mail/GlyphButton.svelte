<script lang="ts">
  // The icon set's round button (`IconButton`) for a glyph the set does not have yet: the same size token, tint tiers, press scale and tooltip, drawing through `Glyph`.
  import { tip } from "@calternal/ui";
  import type { HTMLButtonAttributes } from "svelte/elements";
  import Glyph from "../../shell/AppIcon.svelte";
  let { icon, label, pressed, tone = "quiet", small = false, class: klass, ...rest }: { icon: string; label: string; pressed?: boolean; tone?: "quiet" | "tint"; small?: boolean; class?: string } & Omit<HTMLButtonAttributes, "aria-label" | "title"> = $props();
</script>

<button type="button" class={["ib", tone, klass]} class:small aria-label={label} use:tip={{ label }} aria-pressed={pressed} {...rest}><Glyph name={icon} inline={small} /></button>

<style>
  .ib { position: relative; flex: none; display: inline-grid; place-items: center; width: var(--ib, var(--pill-h)); height: var(--ib, var(--pill-h)); border-radius: var(--r-pill); color: var(--ink-2); transition: scale var(--dur-fast) var(--ease-out), background-color var(--dur-fast) var(--ease-out), color var(--dur-fast) var(--ease-out); }
  .ib:active:not(:disabled) { scale: var(--press-scale); } .ib:disabled { opacity: 0.4; } .small { --ib: var(--ib-small); } .tint { background: rgb(var(--ink-rgb) / 0.065); }
  @media (hover: hover) and (pointer: fine) { .ib.quiet:not(:disabled):hover, .ib.tint:not(:disabled):hover { background: rgb(var(--ink-rgb) / 0.09); color: var(--ink); } }
  .ib[aria-pressed="true"] { background: rgb(var(--ink-rgb) / 0.12); color: var(--ink); }
</style>

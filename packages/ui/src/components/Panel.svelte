<script lang="ts">
  // THE glass panel primitive. Every floating panel in the product is one of these (the composer, the request bar, menus, popovers, sheets, the title
  // pill): a stacking context with the one glass inside it and the adaptive material applied to it (lib/material.ts), so its tint and blur follow the
  // contrast of what is behind it. A component using a Panel never sets its own background, blur, edge or shadow; it only lays out its contents.
  //   radius  "pill" (half its height), "card" (14) or "pane" (20)
  //   floating  hidden until its material is ready (lib/ready.ts), then eased in
  //   dense   one step denser, for lines of words on busy ground (menus, sheets)
  import type { Snippet } from "svelte";
  import type { HTMLAttributes } from "svelte/elements";
  import Glass from "./Glass.svelte";
  import { adaptive } from "../lib/material";
  import { ready } from "../lib/ready";
  let { radius = "pane", dense = false, floating = false, children, class: klass, el = $bindable(), ...rest }: { radius?: "pill" | "card" | "pane"; dense?: boolean; floating?: boolean; children: Snippet; class?: string; el?: HTMLElement } & HTMLAttributes<HTMLDivElement> = $props();
  // A floating panel (a sheet, a popover) appears only once its material is ready; an inline one (the composer) is simply there.
  const gate = (node: HTMLElement, on: boolean) => (on ? ready(node) : undefined);
</script>

<div bind:this={el} class={["panel", klass]} data-radius={radius} use:adaptive use:gate={floating} {...rest}>
  <Glass {dense} />
  {@render children()}
</div>

<style>
  :where(.panel) { position: relative; isolation: isolate; }
  .panel[data-radius="pill"] { border-radius: var(--r-pill); }
  .panel[data-radius="card"] { border-radius: var(--r-card); }
  .panel[data-radius="pane"] { border-radius: var(--r-pane); }
</style>

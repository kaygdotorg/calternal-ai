<script lang="ts">
  // A floating panel anchored to something (a button, a point). Drawn at the end of <body>, outside every clipping and stacking context of the app,
  // placed by lib/place.ts (flipped, shifted and height-limited so it always fits the window), shown only when its material is ready (lib/ready.ts),
  // closed by Escape or a press outside, with focus returned to `trigger`. Menus, the where-it-runs picker and context menus are built on it.
  import type { Snippet } from "svelte";
  import { place, type Anchor, type PlaceOptions } from "../lib/place";
  import { ready } from "../lib/ready";
  import Panel from "./Panel.svelte";
  let {
    open,
    anchor,
    label,
    role = "dialog",
    prefer = "top",
    dense = true,
    trigger,
    onclose,
    children,
    ...rest
  }: { open: boolean; anchor: Anchor | null | undefined; label: string; role?: "dialog" | "menu" | "listbox"; prefer?: PlaceOptions["prefer"]; dense?: boolean; trigger?: HTMLElement | null; onclose: () => void; children: Snippet } & Record<string, unknown> = $props();
  let box = $state<HTMLElement>();
  // Closing keeps the panel for one short fade (`data-leaving`, glass.css) so it does not vanish in a frame; opening is instant once the material is ready.
  let leaving = $state(false);
  let present = $state(false);
  $effect(() => {
    if (open) { present = true; leaving = false; return; }
    if (!present) return;
    leaving = true;
    const t = setTimeout(() => { present = false; leaving = false; }, 200);
    return () => clearTimeout(t);
  });
  const portal = (node: HTMLElement) => {
    document.body.append(node);
    return { destroy: () => node.remove() };
  };
  $effect(() => {
    if (!open || !box || !anchor) return;
    const go = () => place(box!, anchor, { prefer });
    go();
    const away = (e: PointerEvent) => {
      const t = e.target as Node;
      if (!box!.contains(t) && !trigger?.contains(t)) onclose();
    };
    const esc = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        onclose();
        trigger?.focus();
      }
    };
    addEventListener("pointerdown", away, true);
    addEventListener("keydown", esc, true);
    addEventListener("resize", go);
    return () => {
      removeEventListener("pointerdown", away, true);
      removeEventListener("keydown", esc, true);
      removeEventListener("resize", go);
    };
  });
</script>

{#if present}
  <div class="pop" use:portal use:ready bind:this={box} data-leaving={leaving ? "" : undefined} {role} aria-label={label} {...rest}>
    <Panel radius="card" {dense}>{@render children()}</Panel>
  </div>
{/if}

<style>
  .pop { position: fixed; z-index: 60; overscroll-behavior: contain; scrollbar-width: none; }
  .pop :global(.panel) { min-width: 100%; }
</style>

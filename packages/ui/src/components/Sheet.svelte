<script lang="ts">
  // A modal sheet: slides up from the bottom (a phone) or in from the side. A real dialog: `label` is required, focus moves in and is
  // trapped, Escape and the scrim close it, and focus returns to where it came from. The scrim dims the page; the sheet is glass.
  import type { Snippet } from "svelte";
  import Glass from "./Glass.svelte";
  import { adaptive } from "../lib/material";
  let { open, label, onclose, side = "bottom", children }: { open: boolean; label: string; onclose: () => void; side?: "bottom" | "right"; children: Snippet } = $props();
  let box = $state<HTMLElement>();
  let from: HTMLElement | null = null;
  const FOCUSABLE = 'a[href], button:not(:disabled), input:not(:disabled), textarea:not(:disabled), select:not(:disabled), [tabindex]:not([tabindex="-1"])';
  $effect(() => {
    if (!open || !box) return;
    from = document.activeElement as HTMLElement | null;
    (box.querySelector<HTMLElement>(FOCUSABLE) ?? box).focus();
    return () => from?.focus?.();
  });
  function onkey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.stopPropagation();
      onclose();
    } else if (e.key === "Tab" && box) {
      const f = [...box.querySelectorAll<HTMLElement>(FOCUSABLE)];
      if (!f.length) return e.preventDefault();
      const i = f.indexOf(document.activeElement as HTMLElement);
      const next = e.shiftKey ? (i <= 0 ? f.length - 1 : i - 1) : i === f.length - 1 ? 0 : i + 1;
      e.preventDefault();
      f[next]!.focus();
    }
  }
</script>

{#if open}
  <div class="scrim" role="presentation" onclick={onclose}></div>
  <div bind:this={box} class="sheet {side}" role="dialog" aria-modal="true" aria-label={label} tabindex="-1" onkeydown={onkey} use:adaptive>
    <Glass dense />
    {@render children()}
  </div>
{/if}

<style>
  .scrim { position: fixed; inset: 0; z-index: 40; background: var(--scrim); animation: fade var(--dur-base) var(--ease-out); }
  .sheet { position: fixed; z-index: 41; isolation: isolate; overflow: auto; max-height: 88dvh; color: var(--ink); }
  .bottom { left: var(--inset); right: var(--inset); bottom: max(var(--inset), var(--safe-bottom)); border-radius: var(--r-pane); animation: up var(--dur-slow) var(--spring); }
  .right { top: var(--inset); bottom: var(--inset); right: var(--inset); width: min(420px, 92vw); border-radius: var(--r-pane); animation: in var(--dur-slow) var(--spring); }
  @keyframes fade { from { opacity: 0; } }
  @keyframes up { from { translate: 0 24px; opacity: 0; } }
  @keyframes in { from { translate: 24px 0; opacity: 0; } }
</style>

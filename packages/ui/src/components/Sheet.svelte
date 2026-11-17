<script lang="ts">
  // A modal sheet: slides up from the bottom (a phone), in from the side, or down from the top edge (`top`, the palette). A real dialog: `label` is required, focus moves in and is
  // trapped, Escape and the scrim close it, and focus returns to where it came from. The scrim dims the page; the sheet is glass.
  import type { Snippet } from "svelte";
  import Panel from "./Panel.svelte";
  let { open, label, onclose, side = "bottom", children }: { open: boolean; label: string; onclose: () => void; side?: "bottom" | "right" | "top"; children: Snippet } = $props();
  let box = $state<HTMLElement>();
  // Drawn at the end of <body>, outside every stacking context of the app (a card with a transform would otherwise trap the sheet beneath the bottom bar).
  const portal = (node: HTMLElement) => {
    document.body.append(node);
    return { destroy: () => node.remove() };
  };
  let from: HTMLElement | null = null;
  const FOCUSABLE = 'a[href], button:not(:disabled), input:not(:disabled), textarea:not(:disabled), select:not(:disabled), [tabindex]:not([tabindex="-1"])';
  $effect(() => {
    if (!open || !box) return;
    from = document.activeElement as HTMLElement | null;
    if (!box.contains(document.activeElement)) box.focus(); // (a field inside that took focus keeps it) the dialog itself: announced by name, and no focus ring until the person presses Tab
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
  <div class="layer" use:portal>
    <div class="scrim" role="presentation" onclick={onclose}></div>
    <Panel bind:el={box} floating dense radius="pane" class="sheet {side}" role="dialog" aria-modal="true" aria-label={label} tabindex={-1} onkeydown={onkey}>
      {@render children()}
    </Panel>
  </div>
{/if}

<style>
  .scrim { position: fixed; inset: 0; z-index: 40; background: var(--scrim); }
  :global(.sheet:focus) { outline: none; }
  :global(.sheet) { position: fixed; z-index: 41; isolation: isolate; overflow: auto; max-height: 88dvh; color: var(--ink); overscroll-behavior: contain; }
  :global(.sheet.bottom) { left: var(--inset); right: var(--inset); bottom: var(--inset); border-radius: var(--r-pane); --origin: bottom center; translate: 0 0; }
  :global(.sheet.right) { top: var(--inset); bottom: var(--inset); right: var(--inset); width: min(480px, 92vw); border-radius: var(--r-pane); --origin: right center; }
  /* The palette: centred under the top edge. */
  :global(.sheet.top) { top: 10dvh; left: 50%; width: min(40rem, calc(100vw - 2 * var(--inset))); border-radius: var(--r-pane); --origin: top center; translate: -50% 0; max-height: 80dvh; }
  :global(.sheet.top[data-surface]:not([data-ready])) { translate: -50% calc(var(--s-8) * -1); }
  :global(.sheet[data-surface]:not([data-ready])) { translate: 0 var(--s-8); }
  :global(.sheet.right[data-surface]:not([data-ready])) { translate: var(--s-8) 0; }
</style>

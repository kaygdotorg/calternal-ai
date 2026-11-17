<script lang="ts">
  // A modal sheet: slides up from the bottom (a phone), in from the side, down from the top edge (`top`, the palette), or centred in the window (`center`, onboarding: it belongs to no region). A real dialog: `label` is required, focus moves in and is
  // trapped, Escape and the scrim close it, and focus returns to where it came from. The scrim dims the page; the sheet is glass.
  import type { Snippet } from "svelte";
  import Panel from "./Panel.svelte";
  let { open, label, onclose, side = "bottom", children }: { open: boolean; label: string; onclose: () => void; side?: "bottom" | "right" | "top" | "center"; children: Snippet } = $props();
  let box = $state<HTMLElement>();
  // Drawn at the end of <body>, outside every stacking context of the app (a card with a transform would otherwise trap the sheet beneath the bottom bar).
  const portal = (node: HTMLElement) => {
    document.body.append(node);
    return { destroy: () => node.remove() };
  };
  let from: HTMLElement | null = null;
  // Where focus was before any sheet: the last element focused outside a sheet. (A caller may move focus into a field of the sheet in the same turn it opens, before this sheet looks at `activeElement`, and Escape would then return focus to that field, gone with the sheet.)
  let outside: HTMLElement | null = null;
  $effect(() => {
    const on = (e: FocusEvent) => { const t = e.target as HTMLElement | null; if (t && t !== document.body && !t.closest?.(".sheet")) outside = t; };
    document.addEventListener("focusin", on);
    return () => document.removeEventListener("focusin", on);
  });
  const FOCUSABLE = 'a[href], button:not(:disabled), input:not(:disabled), textarea:not(:disabled), select:not(:disabled), [tabindex]:not([tabindex="-1"])';
  $effect(() => {
    if (!open || !box) return;
    from = outside ?? (document.activeElement as HTMLElement | null);
    if (!box.contains(document.activeElement)) box.focus(); // (a field inside that took focus keeps it) the dialog itself: announced by name, and no focus ring until the person presses Tab
    return () => from?.focus?.();
  });
  // Drag the grabber (bottom sheets) down to dismiss: the sheet follows the finger with a transform, and goes when pulled past 80 px or flicked.
  let dy = 0, y0 = 0, t0 = 0, dragging = false;
  function gdown(e: PointerEvent) { y0 = e.clientY; t0 = e.timeStamp; dy = 0; dragging = true; (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId); box!.style.transition = "none"; }
  function gmove(e: PointerEvent) { if (!dragging) return; dy = Math.max(0, e.clientY - y0); box!.style.translate = `0 ${dy}px`; }
  function gup(e: PointerEvent) {
    if (!dragging) return;
    dragging = false;
    box!.style.transition = "";
    const fast = dy / Math.max(1, e.timeStamp - t0) > 0.6;
    if (dy > 80 || (fast && dy > 24)) onclose(); else box!.style.translate = "";
  }
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
      {#if side === "bottom"}<div class="grab" role="presentation" onpointerdown={gdown} onpointermove={gmove} onpointerup={gup} onpointercancel={gup}><i></i></div>{/if}
      {@render children()}
    </Panel>
  </div>
{/if}

<style>
  /* The grabber: a touch target at the top of a bottom sheet, drawn only where there is no hover. */
  .grab { display: none; touch-action: none; height: var(--s-8); place-items: center; }
  .grab i { width: var(--s-10); height: var(--s-2); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.25); }
  @media (hover: none) { .grab { display: grid; } }
  .scrim { position: fixed; inset: 0; z-index: 40; background: var(--scrim); }
  :global(.sheet:focus) { outline: none; }
  :global(.sheet) { position: fixed; z-index: 41; isolation: isolate; overflow: auto; max-height: 88dvh; color: var(--ink); overscroll-behavior: contain; }
  :global(.sheet.bottom) { left: var(--inset); right: var(--inset); bottom: var(--inset); border-radius: var(--r-pane); --origin: bottom center; translate: 0 0; }
  :global(.sheet.right) { top: var(--inset); bottom: var(--inset); right: var(--inset); width: min(480px, 92vw); border-radius: var(--r-pane); --origin: right center; }
  /* The palette: centred under the top edge. */
  :global(.sheet.top) { top: 10dvh; left: 50%; width: min(40rem, calc(100vw - 2 * var(--inset))); border-radius: var(--r-pane); --origin: top center; translate: -50% 0; max-height: 80dvh; }
  /* Centred: a window-level dialog that sits over the whole app, not in a sidebar. */
  :global(.sheet.center) { top: 50%; left: 50%; width: min(40rem, calc(100vw - 2 * var(--inset))); border-radius: var(--r-pane); --origin: center; translate: -50% -50%; max-height: 88dvh; }
  :global(.sheet.center[data-surface]:not([data-ready])) { translate: -50% calc(-50% + var(--s-8)); }
  :global(.sheet.top[data-surface]:not([data-ready])) { translate: -50% calc(var(--s-8) * -1); }
  :global(.sheet[data-surface]:not([data-ready])) { translate: 0 var(--s-8); }
  :global(.sheet.right[data-surface]:not([data-ready])) { translate: var(--s-8) 0; }
</style>

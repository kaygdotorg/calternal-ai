<script lang="ts">
  // The handle that resizes a sidebar: at the vertical centre of the gap between the sidebar and the middle card, a slim bar that shows on hover, focus or while dragging (the hit area is wider than the bar).
  // Pointer: drag it; the width follows the pointer (one write per frame). Keyboard: it is a separator you can focus; Left and Right (or Up and Down) move it 16 px, with Shift 64 px; Home and End go to the
  // smallest and the widest; Enter or a double-click puts the width back. The width is the person's, kept for this device (shell.svelte.ts), and is limited to a range so neither the sidebar nor the card can be
  // squeezed to nothing. Widths are unscaled CSS pixels; the interface size scales them like everything else.
  import { shell, WIDTHS, type Side } from "./shell.svelte";
  let { side }: { side: Side } = $props();
  const r = $derived(WIDTHS[side]);
  const width = $derived(shell.widths[side] ?? r.def);
  let drag = $state(false);
  let el = $state<HTMLElement>();
  let frame = 0;
  function at(clientX: number): number {
    const scale = parseFloat(getComputedStyle(document.documentElement).getPropertyValue("--ui-scale")) || 1;
    const inset = parseFloat(getComputedStyle(document.documentElement).getPropertyValue("--inset")) || 8;
    return (side === "left" ? clientX - inset / 2 : innerWidth - clientX - inset / 2) / scale;
  }
  function down(e: PointerEvent) {
    if (e.button !== 0) return;
    drag = true;
    el?.setPointerCapture(e.pointerId);
    e.preventDefault();
  }
  function move(e: PointerEvent) {
    if (!drag) return;
    const x = e.clientX;
    cancelAnimationFrame(frame);
    frame = requestAnimationFrame(() => shell.setWidth(side, at(x), false));
  }
  function up() {
    if (!drag) return;
    drag = false;
    cancelAnimationFrame(frame);
    shell.setWidth(side, shell.widths[side] ?? r.def, true); // keep it
  }
  function key(e: KeyboardEvent) {
    const step = e.shiftKey ? 64 : 16;
    const grow = side === "left" ? "ArrowRight" : "ArrowLeft", shrink = side === "left" ? "ArrowLeft" : "ArrowRight";
    let w = width;
    if (e.key === grow) w += step;
    else if (e.key === shrink) w -= step;
    else if (e.key === "ArrowUp") w += step;
    else if (e.key === "ArrowDown") w -= step;
    else if (e.key === "Home") w = r.min;
    else if (e.key === "End") w = r.max;
    else if (e.key === "Enter") { shell.setWidth(side, null, true); e.preventDefault(); return; }
    else return;
    e.preventDefault();
    shell.setWidth(side, w, true);
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div class="grip {side}" class:drag bind:this={el} role="separator" aria-orientation="vertical" aria-label={side === "left" ? "Resize the sidebar" : "Resize the side panel"} aria-valuemin={r.min} aria-valuemax={r.max} aria-valuenow={Math.round(width)} tabindex="0"
  onpointerdown={down} onpointermove={move} onpointerup={up} onpointercancel={up} ondblclick={() => shell.setWidth(side, null, true)} onkeydown={key}><i></i></div>

<style>
  /* Inside the middle region (the landmark), centred in the gap beside the card (one inset wide); the hit area is wider than the gap and reaches under the card's edge. */
  .grip { position: absolute; z-index: 5; top: 0; bottom: 0; width: var(--s-7); display: grid; place-items: center; cursor: col-resize; touch-action: none; }
  .grip.left { left: calc(var(--inset) / 2 - var(--s-7) / 2); }
  .grip.right { right: calc(var(--inset) / 2 - var(--s-7) / 2); }
  .grip i { display: block; width: var(--s-2); height: var(--s-10); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.4); opacity: 0; transition: opacity var(--dur-base) var(--ease-out), height var(--dur-base) var(--ease-out), background-color var(--dur-base) var(--ease-out); }
  .grip:hover i, .grip:focus-visible i, .grip.drag i { opacity: 1; }
  .grip.drag i { height: calc(var(--s-10) + var(--s-4)); background: var(--accent); }
  /* Touch has no hover and no precise pointer: the handles are not offered (a phone has no sidebars to resize at all). */
  @media (pointer: coarse) { .grip { display: none; } }
  .grip:focus-visible { outline: none; }
  .grip:focus-visible i { outline: 2px solid var(--accent); outline-offset: 2px; }
</style>

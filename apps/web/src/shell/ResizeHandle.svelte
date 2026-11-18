<script lang="ts">
  // The handle that resizes a sidebar: at the vertical centre of the gap between the sidebar and the middle card, a slim bar that shows on hover, focus or while dragging (the hit area is wider than the bar).
  // Pointer: drag it; the width follows the pointer (one write per frame). Keyboard: it is a separator you can focus; Left and Right (or Up and Down) move it 16 px, with Shift 64 px; Home and End go to the
  // smallest and the widest; Enter or a double-click puts the width back. The width is the person's, kept for this device (shell.svelte.ts), and is limited to a range so neither the sidebar nor the card can be
  // squeezed to nothing. Widths are unscaled CSS pixels; the interface size scales them like everything else.
  import { Grip } from "@calternal/ui";
  import { shell, WIDTHS, type Side } from "./shell.svelte";
  let { side }: { side: Side } = $props();
  const r = $derived(WIDTHS[side]);
  const width = $derived(shell.widths[side] ?? r.def);
  function at(clientX: number): number {
    const scale = parseFloat(getComputedStyle(document.documentElement).getPropertyValue("--ui-scale")) || 1;
    const inset = parseFloat(getComputedStyle(document.documentElement).getPropertyValue("--inset")) || 8;
    return (side === "left" ? clientX - inset / 2 : innerWidth - clientX - inset / 2) / scale;
  }
  const move = (e: PointerEvent) => shell.setWidth(side, at(e.clientX), false);
  const end = () => shell.setWidth(side, shell.widths[side] ?? r.def, true); // keep it
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

<!-- Inside the middle region (the landmark), centred in the gap beside the card (one inset wide); the hit area is wider than the gap and reaches under the card's edge. The handle itself is the shared Grip. -->
<Grip class={side} label={side === "left" ? "Resize the sidebar" : "Resize the side panel"} min={r.min} max={r.max} now={width} style={side === "left" ? "left: calc(var(--inset) / 2 - var(--s-7) / 2)" : "right: calc(var(--inset) / 2 - var(--s-7) / 2)"} onmove={move} onend={end} onkey={key} onreset={() => shell.setWidth(side, null, true)} />

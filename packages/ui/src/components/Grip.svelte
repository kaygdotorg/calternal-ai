<script lang="ts">
  // The one drag handle: a slim bar at the centre of the gap between two things that can be resized (the sidebars and the middle card, two windows of the canvas), shown on hover, focus or while dragging; the hit area is wider than the bar.
  // It owns the pointer capture, the one-write-per-frame throttling, the separator role and the focus ring; what the movement means is the caller's: `onstart` gets the press, `onmove` gets the pointer event (once per frame), `onend` is called on release, `onkey` gets a key
  // when the handle is focused (it should call `preventDefault` for the keys it uses), `onreset` is a double-click. Placement is the caller's (`style`: where the gap is). `orientation="horizontal"` is the same handle turned for a boundary between rows.
  // Touch has no hover and no precise pointer: the handle is not offered there.
  let { orientation = "vertical", label, min, max, now, style, class: klass, onstart, onmove, onend, onkey, onreset }: {
    orientation?: "vertical" | "horizontal"; label: string; min?: number; max?: number; now?: number; style?: string; class?: string;
    onstart?: (e: PointerEvent) => void; onmove: (e: PointerEvent) => void; onend?: () => void; onkey?: (e: KeyboardEvent) => void; onreset?: () => void;
  } = $props();
  let drag = $state(false);
  let el = $state<HTMLElement>();
  let frame = 0;
  function down(e: PointerEvent) {
    if (e.button !== 0) return;
    drag = true;
    onstart?.(e);
    el?.setPointerCapture(e.pointerId);
    e.preventDefault();
  }
  function move(e: PointerEvent) {
    if (!drag) return;
    cancelAnimationFrame(frame);
    frame = requestAnimationFrame(() => onmove(e));
  }
  function up() {
    if (!drag) return;
    drag = false;
    cancelAnimationFrame(frame);
    onend?.();
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div class={["grip", orientation, klass]} class:drag bind:this={el} role="separator" aria-orientation={orientation} aria-label={label} aria-valuemin={min} aria-valuemax={max} aria-valuenow={now === undefined ? undefined : Math.round(now)} tabindex="0" {style}
  onpointerdown={down} onpointermove={move} onpointerup={up} onpointercancel={up} ondblclick={() => onreset?.()} onkeydown={(e) => onkey?.(e)}><i></i></div>

<style>
  .grip { position: absolute; z-index: 5; display: grid; place-items: center; touch-action: none; }
  .grip.vertical { top: 0; bottom: 0; width: var(--s-7); cursor: col-resize; }
  .grip.horizontal { left: 0; right: 0; height: var(--s-7); cursor: row-resize; }
  .grip i { display: block; border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.4); opacity: 0; transition: opacity var(--dur-base) var(--ease-out), background-color var(--dur-base) var(--ease-out); }
  .vertical i { width: var(--s-2); height: calc(var(--s-10) + var(--s-6)); }
  .horizontal i { height: var(--s-2); width: calc(var(--s-10) + var(--s-6)); }
  .grip:hover i, .grip:focus-visible i, .grip.drag i { opacity: 1; }
  .grip.drag i { background: var(--accent); }
  @media (pointer: coarse) { .grip { display: none; } }
  /* The slim bar draws the ring (round, like the bar); the hit area around it shows none. */
  .grip:focus-visible { outline: none !important; }
  .grip:focus-visible i { outline: 2px solid var(--accent); outline-offset: 2px; }
</style>

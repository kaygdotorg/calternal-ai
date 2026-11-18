<script lang="ts">
  // One toast, drawn inside the stack's item (which handles stacking, swipe and timers). It opens
  // from a small dot into a capsule that fits its words, and stays the same toast as those words
  // change: the count rolls up (×12), "12 blocks deleted" turns into "Undone", a wait turns into
  // its result. The glass springs to fit each time; nothing jumps.
  //
  // The words set the toast's real size straight away (so the stack always uses the true
  // height); only the glass shape behind them animates, and it clips the words to itself.
  //
  // The glass is ALWAYS its final size and is only SCALED (transform, never width or height, #1030):
  // a backdrop-blurred box that changes size every frame re-lays out and re-blurs on the main thread
  // (30-50 ms frames in real Safari as a toast opened). The dot is the same capsule scaled to a 12 px
  // height, so its corners stay round; at rest the transform is removed, so nothing stays promoted.
  // Its size comes only from the ResizeObserver entry, never offsetWidth/offsetHeight: those force a
  // layout in the very frame a toast opens (22 ms of forced layout per drop, measured in Chromium).
  //
  // What it says is ToastRow, shared with the toast shown in the app's bar (#145).
  import { onMount } from "svelte";
  import { views, type ToastView } from "./notify.svelte";
  import ToastRow from "./ToastRow.svelte";
  import { Spring, run, prefersReducedMotion } from "../lib/spring-engine";
  import { modality } from "../lib/input";

  let { id }: { id: string; closeToast?: () => void } = $props();

  // Keep the last thing it said while it leaves (the view is gone by then).
  // svelte-ignore state_referenced_locally
  let last = $state<ToastView | undefined>(views.get(id));
  $effect(() => {
    const v = views.get(id);
    if (v) last = v;
  });
  const v = $derived(last);

  let shape = $state<HTMLDivElement>();
  let inner = $state<HTMLDivElement>();
  const SEED = 12;
  const W = new Spring(SEED, { damping: 0.78, response: 0.42 }, 0.3);
  const H = new Spring(SEED, { damping: 0.86, response: 0.38 }, 0.3);
  let size = { w: SEED, h: SEED }; // the words' own size, last measured
  let open = $state(false); // the dot has started to open: the words may show
  let stop: (() => void) | null = null;
  let seed: ReturnType<typeof setTimeout> | undefined;
  const reduce = prefersReducedMotion();

  /** Draws the springs' current size: a scale of the glass from its real (final) size, and the words' clip. */
  function paint() {
    if (!shape || !inner) return;
    const sx = size.w ? W.value / size.w : 1;
    const sy = size.h ? H.value / size.h : 1;
    shape.style.transform = Math.abs(sx - 1) < 0.001 && Math.abs(sy - 1) < 0.001 ? "" : `scale(${sx}, ${sy})`;
    const x = (size.w - W.value) / 2;
    const y = (size.h - H.value) / 2;
    inner.style.clipPath = `inset(${y}px ${x}px round ${Math.min(20, H.value / 2)}px)`;
  }
  /** The words' border box as the ResizeObserver reports it (every engine we support has `borderBoxSize`),
   * rounded like `offsetWidth` was so the glass lands on the same pixels. */
  const boxOf = (e: ResizeObserverEntry) => ({ w: Math.round(e.borderBoxSize[0]!.inlineSize), h: Math.round(e.borderBoxSize[0]!.blockSize) });
  /** The glass takes the words' new size at once and is redrawn in the same frame, so the scale keeps it
   * looking the size the springs are at (no one-frame jump to the new size). */
  function place(next: { w: number; h: number }) {
    size = next;
    if (!shape) return;
    shape.style.width = `${next.w}px`;
    shape.style.height = `${next.h}px`;
    paint();
  }
  /** Springs the glass to the words' last measured size (or snaps, without motion). */
  function fit(animate: boolean) {
    if (!inner) return;
    if (!animate || reduce) {
      W.snap(size.w);
      H.snap(size.h);
      paint();
      return;
    }
    W.target = size.w;
    H.target = size.h;
    if (!stop) stop = run([W, H], paint, () => (stop = null));
  }

  onMount(() => {
    if (!inner) return;
    let first = true;
    // New words resize `.inner`; the observer reports it after layout and before that frame is
    // painted, and `place` redraws the old size as a scale in that same frame, so no frame shows
    // the glass already settled. Its first report arrives before the first paint too.
    const ro = new ResizeObserver((entries) => {
      const next = boxOf(entries[entries.length - 1]!);
      if (!next.w || !next.h) return;
      if (!first) {
        place(next);
        if (open) fit(true);
        return;
      }
      first = false;
      if (reduce) {
        place(next);
        open = true;
        fit(false);
        return;
      }
      // The seed: a dot, for a beat (the capsule scaled to a 12 px height). Then it opens into the capsule.
      W.snap(next.w * (SEED / next.h));
      H.snap(SEED);
      place(next);
      seed = setTimeout(() => {
        open = true;
        fit(true);
      }, 90);
    });
    ro.observe(inner);
    return () => {
      clearTimeout(seed);
      ro.disconnect();
      stop?.();
    };
  });
</script>

{#if v}
  <div class="slot" data-instant={modality() === "key"} data-phase={v.phase} data-severity={v.severity} class:open>
    <div class="shape glass dense" bind:this={shape} aria-hidden="true"></div>
    <div class="inner" data-instant={modality() === "key"} bind:this={inner}>
      <ToastRow {v} />
    </div>
  </div>
{/if}

<style>
  .slot {
    position: relative;
    display: inline-grid;
    place-items: center;
    font-family: var(--toast-font, var(--font-ui, system-ui, sans-serif));
    font-size: calc(13.5 * var(--type-px, 0.0625rem));
    font-weight: 500;
    color: var(--toast-ink, var(--ink, #1d1b19));
  }
  .shape {
    position: absolute;
    left: 50%;
    top: 50%;
    right: auto;
    bottom: auto;
    translate: -50% -50%;
    z-index: 0;
    width: 12px;
    height: 12px;
    border-radius: var(--r-card);
    /* Its fill, blur, shade and edge are the shared floating glass (`float-glass`, #319). */
    /* Opening: the dot pops in first. */
    animation: seed 180ms cubic-bezier(0.2, 0.9, 0.3, 1.3) both;
  }
  @keyframes seed {
    from {
      scale: 0.2;
      opacity: 0;
    }
  }
  .inner {
    position: relative;
    display: flex;
    align-items: center;
    gap: 8px;
    width: max-content;
    max-width: var(--toast-max, min(440px, calc(100vw - 32px)));
    min-height: 40px;
    padding: 6px 6px 6px 16px;
    box-sizing: border-box;
    clip-path: inset(50%);
    opacity: 0;
    transition: opacity 160ms ease-out;
  }
  .open .inner {
    opacity: 1;
    transition-delay: 60ms;
  }
  .inner:not(:has(:global(.act))) {
    padding-right: 16px;
  }
  @media (prefers-reduced-motion: reduce) {
    .shape {
      animation: none;
    }
  }
</style>

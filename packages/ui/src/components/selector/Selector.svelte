<script lang="ts" generics="V">
  // One selector for every single choice (#42). A single capsule sits behind plain options. When
  // the choice changes, the capsule's leading edge stretches toward the new option and its
  // trailing edge catches up; the labels never move or stretch. Click again mid-flight and it
  // turns from wherever it is. The choice itself changes at once; the capsule just shows it.
  //
  // It's a tab list: arrow keys move and choose (Home and End too), only the chosen option is in
  // the Tab order, and disabled options are skipped. From the keyboard the capsule moves straight
  // there, like every keyboard action in Calternal.
  import { onMount, type Snippet } from "svelte";
  import type { Action } from "svelte/action";
  import { Spring, run, prefersReducedMotion } from "../../lib/spring-engine";
  import { edgeConfigs, step, type Edges, type SelectorOption } from "./edges";
  import { measureOnMount, measureOnFrame } from "./measure-batch";
  import { modality } from "../../lib/input";
  import Icon from "../Icon.svelte";
  /** Keyboard actions never animate: true after a key, so the element that moves can say so on itself. */
  const instant = (): true | undefined => (modality() === "key" ? true : undefined);

  let {
    options,
    value = $bindable(),
    label,
    onchange,
    onpick,
    tooltip,
    size = "md",
    fill = true,
    class: className = "",
    optionIcon,
    optionContent,
  }: {
    options: SelectorOption<V>[];
    /** The chosen value; `null` shows no capsule (nothing chosen). */
    value: V | null;
    /** The group's accessible name. */
    label: string;
    /** Called when the choice changes. */
    onchange?: (value: V) => void;
    /** Called on every pick, even of the option that's already chosen (for "pick again to clear"). */
    onpick?: (value: V) => void;
    /** A tooltip action for icon-only options (the app's warm tooltip). */
    tooltip?: Action<HTMLElement, { label: string }>;
    size?: "sm" | "md" | "lg";
    /** Options share the width equally (true) or size to their content (false). */
    fill?: boolean;
    class?: string;
    /** An icon for every option, drawn from its value (instead of one snippet per option). */
    optionIcon?: Snippet<[SelectorOption<V>]>;
    /** A whole custom option body (a font preview tile, say); the label stays its name. */
    optionContent?: Snippet<[SelectorOption<V>]>;
  } = $props();

  let track = $state<HTMLDivElement>();
  let capsule = $state<HTMLDivElement>();
  // The option buttons, in order (read from the DOM when needed, never per frame).
  const buttons = (): HTMLButtonElement[] => (track ? [...track.querySelectorAll<HTMLButtonElement>(":scope > .opt")] : []);
  let rects: Edges[] = [];
  let shown = $state(false); // the capsule is visible (there's a choice, and it's been placed)
  let reduced = $state(false);

  const L = new Spring(0, { damping: 1, response: 0.3 }, 0.1);
  const R = new Spring(0, { damping: 1, response: 0.3 }, 0.1);
  let stop: (() => void) | null = null;

  const index = $derived(options.findIndex((o) => o.value === value));
  // The option in the Tab order: the chosen one, or the first that can be chosen.
  const focusable = $derived(index >= 0 && !options[index]?.disabled ? index : Math.max(0, options.findIndex((o) => !o.disabled)));

  /** Reads every option's place in one go (only on change, resize or font load; never per frame). */
  function measure() {
    rects = buttons().map((b) => ({ left: b.offsetLeft, right: b.offsetLeft + b.offsetWidth }));
  }

  function paint() {
    if (!capsule) return;
    capsule.style.translate = `${L.value}px 0`;
    capsule.style.width = `${Math.max(0, R.value - L.value)}px`;
  }

  /** Puts the capsule on the chosen option: gliding, or straight there. */
  function place(animate: boolean) {
    const target = rects[index];
    if (!target || index < 0) {
      shown = false;
      return;
    }
    if (!shown || !animate || reduced) {
      stop?.();
      stop = null;
      L.snap(target.left);
      R.snap(target.right);
      paint();
      shown = true;
      return;
    }
    const cfg = edgeConfigs({ left: L.value, right: R.value }, target);
    L.config = cfg.left;
    R.config = cfg.right;
    L.target = target.left;
    R.target = target.right;
    if (!stop)
      stop = run([L, R], paint, () => {
        stop = null;
      });
  }

  function choose(i: number, animate: boolean) {
    const o = options[i];
    if (!o || o.disabled) return;
    onpick?.(o.value);
    if (o.value === value) return;
    nextAnimate = animate;
    value = o.value; // the state changes at once; the capsule follows (see the effect below)
    onchange?.(o.value);
  }

  function key(e: KeyboardEvent, i: number) {
    const rtl = track ? getComputedStyle(track).direction === "rtl" : false;
    const fwd = rtl ? "ArrowLeft" : "ArrowRight";
    const back = rtl ? "ArrowRight" : "ArrowLeft";
    const disabled = options.map((o) => !!o.disabled);
    let next = -1;
    if (e.key === fwd || e.key === "ArrowDown") next = step(disabled, i, 1);
    else if (e.key === back || e.key === "ArrowUp") next = step(disabled, i, -1);
    else if (e.key === "Home") next = step(disabled, -1, 1);
    else if (e.key === "End") next = step(disabled, options.length, -1);
    if (next < 0) return;
    e.preventDefault();
    const target = buttons()[next];
    target?.focus();
    target?.scrollIntoView({ block: "nearest", inline: "nearest" });
    choose(next, false);
  }

  // Whatever changed the value (a click, a key, a URL, another control), the capsule follows it:
  // gliding, except straight there after a key press.
  let nextAnimate = true;
  let lastIndex = -2;
  $effect(() => {
    const i = index;
    if (i === lastIndex) return;
    const first = lastIndex === -2;
    lastIndex = i;
    if (first) return;
    if (!rects.length) measure();
    place(nextAnimate);
    nextAnimate = true;
  });

  onMount(() => {
    reduced = prefersReducedMotion();
    const cancelMount = measureOnMount(measure, () => place(false)); // before first paint, with no sweep in from 0
    // Re-measure when anything changes size: the group, an option, or the fonts arriving.
    let alive = true;
    let cancelResize: (() => void) | undefined;
    const remeasure = () => {
      if (!alive || cancelResize) return;
      cancelResize = measureOnFrame(measure, () => {
        cancelResize = undefined;
        place(!!stop); // a resize mid-glide retargets; otherwise it just sits in the new place
      });
    };
    const ro = new ResizeObserver(remeasure);
    if (track) ro.observe(track);
    for (const b of buttons()) ro.observe(b);
    // `fonts.ready` is a getter that forces a style update, so it is only asked while a font is still loading (`status` is free).
    if (document.fonts && document.fonts.status !== "loaded") document.fonts.ready.then(remeasure);
    const mq = matchMedia("(prefers-reduced-motion: reduce)");
    const onMq = () => (reduced = mq.matches);
    mq.addEventListener("change", onMq);
    return () => {
      alive = false;
      cancelMount();
      cancelResize?.();
      ro.disconnect();
      mq.removeEventListener("change", onMq);
      stop?.();
    };
  });

  // Tooltips only where an option needs one (icon-only); the action is the app's own.
  const tip: Action<HTMLElement, { label: string; on: boolean }> = (node, p) => {
    let inner = p.on && tooltip ? tooltip(node, { label: p.label }) : undefined;
    return {
      update(n) {
        inner?.update?.({ label: n.label });
      },
      destroy() {
        inner?.destroy?.();
      },
    };
  };
</script>

<div class="selector {size} {className}" data-instant={instant()} class:fill role="tablist" aria-label={label} bind:this={track}>
  <div class="capsule" class:shown class:reduced bind:this={capsule} aria-hidden="true"></div>
  {#each options as o, i (o.value)}
    <button
      type="button"
      role="tab"
      class="opt bounce"
      class:icon-only={o.iconOnly}
      aria-selected={i === index}
      aria-disabled={o.disabled ? "true" : undefined}
      aria-label={o.iconOnly ? o.label : undefined}
      title={!tooltip && (o.iconOnly || o.hint) ? (o.hint ?? o.label) : undefined}
      tabindex={i === focusable ? 0 : -1}
      use:tip={{ label: o.hint ?? o.label, on: !!(o.iconOnly || o.hint) }}
      onclick={() => choose(i, true)}
      onkeydown={(e) => key(e, i)}
    >
      {#if optionContent}{@render optionContent(o)}{:else}
      {#if o.icon}<span class="icon">{@render o.icon()}</span>{:else if o.iconName}<span class="icon"><Icon name={o.iconName} /></span>{:else if optionIcon}<span class="icon">{@render optionIcon(o)}</span>{/if}
      {#if !o.iconOnly}<span class="lbl" data-text={o.label}>{o.label}</span>{/if}
      {/if}
    </button>
  {/each}
</div>

<style>
  .selector {
    --h: var(--row-h);
    --pad: 2px;
    position: relative;
    display: flex;
    gap: 2px;
    padding: var(--pad);
    /* A pill within a pill: the track and the chosen option are both full pills, the option inset by the track's padding (concentric). */
    border-radius: var(--r-pill);
    background: var(--selector-track, rgb(var(--ink-rgb) / 0.07));
    overflow-x: auto;
    scrollbar-width: none;
    isolation: isolate;
  }
  /* So the options can ask how wide the group is (below). Only a group that fills its row has a
     width to ask about: size containment gives a group that sizes to its words no inline size at
     all, and in a shrink-to-fit place (a grid cell, a flex item) it collapsed to its padding and
     scrolled its words sideways. */
  .fill {
    container-type: inline-size;
  }
  .sm {
    --h: calc(var(--row-h) - var(--s-3));
  }
  .lg {
    --h: var(--pill-h);
  }
  .selector::-webkit-scrollbar {
    display: none;
  }
  /* The capsule: tonal, never an accent blob. It only ever moves by translate and width. */
  .capsule {
    position: absolute;
    /* Under every option, by the stacking rules themselves (the track is its own stacking context),
       so no browser can paint it over a label. */
    z-index: -1;
    top: var(--pad);
    bottom: var(--pad);
    left: 0;
    width: 0;
    border-radius: var(--r-pill);
    background: var(--selector-capsule, var(--paper));
    box-shadow: var(--selector-capsule-shadow, 0 0 0 1px rgb(var(--ink-rgb) / 0.1), 0 1px 2px rgb(0 0 0 / 0.06));
    opacity: 0;
    transition: opacity 120ms ease-out;
    pointer-events: none;
  }
  .capsule.shown {
    opacity: 1;
  }
  /* Reduced motion: a short, plain change instead of a spring. */
  .capsule.reduced {
    transition:
      opacity 120ms ease-out,
      translate 140ms ease-out,
      width 140ms ease-out;
  }
  .opt {
    position: relative;
    z-index: 1;
    flex: 0 0 auto;
    min-height: var(--h);
    /* Tight on a narrow phone (320): less air and a hair smaller words, so every label fits
       without the group scrolling sideways. Wider, it's the usual size. */
    padding: 0 clamp(5px, 1.5vw, calc(var(--h) / 3));
    border: 0;
    border-radius: var(--r-pill);
    background: none;
    font: inherit;
    font-size: min(var(--selector-font-size, var(--text-ui)), max(var(--text-label), 3.45vw));
    font-weight: 500;
    color: var(--ink-2);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--selector-option-gap, 6px);
    flex-direction: var(--selector-option-direction, row);
    white-space: nowrap;
    cursor: pointer;
    outline: none;
    -webkit-tap-highlight-color: transparent;
    transition:
      color 250ms ease-out,
      scale 150ms ease-out;
  }
  .fill .opt {
    flex: 1 1 0;
  }
  /* On a phone the words differ a lot in length, and equal widths would squeeze the longest one
     against its neighbours. There, each option keeps its own words and padding, and the spare
     room is shared out equally, so the air around every label is the same. */
  @container (max-width: 420px) {
    .fill .opt:not(.icon-only) {
      flex: 1 1 auto;
    }
  }
  /* Large text (#351, `data-text` from lib/text-size): a label may take two lines, broken
     between words, rather than push the group past the screen and scroll it sideways. */
  :global(:root[data-text="large"]) .fill .opt:not(.icon-only) {
    flex: 1 1 auto;
    white-space: normal;
    line-height: 1.15;
    padding-block: 4px;
    text-align: center;
    /* Like the system's own segmented controls, the words stop growing on a narrow phone (about
       18 px at 320), so four of them still share one row. */
    font-size: min(var(--selector-font-size, var(--text-ui)), 5.6vw);
  }
  /* Equal segments (the Style sheet's tabs): each takes the same share, whatever its words, even on a phone. */
  .selector.equal .opt:not(.icon-only) {
    flex: 1 1 0;
  }
  .opt.icon-only {
    min-width: var(--h);
    padding: 0;
  }
  /* Touch: a 44 px hit area without changing how it looks. */
  @media (pointer: coarse) {
    /* Short words (Light, Dark) still get a 44 px wide target (#346). */
    .opt:not(.icon-only) {
      min-width: 44px;
    }
    .opt::after {
      content: "";
      position: absolute;
      inset: -6px -1px;
    }
  }
  .opt[aria-selected="true"] {
    color: var(--ink);
    font-weight: 600;
  }
  @media (hover: hover) and (pointer: fine) {
    .opt:not([aria-selected="true"]):not([aria-disabled="true"]):hover {
      color: var(--ink);
    }
  }
  .opt[aria-disabled="true"] {
    color: var(--ink-3);
    cursor: default;
  }
  /* Focus is its own ring, distinct from the capsule, and only from the keyboard. */
  .opt:focus-visible {
    box-shadow: 0 0 0 2px var(--accent);
  }
  :global(:root[data-input="pointer"]) .opt:focus-visible {
    box-shadow: none;
  }
  .icon {
    display: grid;
    place-items: center;
  }
  /* The bold twin reserves the label's width, so turning bold never nudges anything. */
  .lbl {
    display: inline-grid;
    text-align: center;
  }
  .lbl::after {
    content: attr(data-text);
    font-weight: 600;
    visibility: hidden;
    height: 0;
    overflow: hidden;
    user-select: none;
    pointer-events: none;
  }
</style>

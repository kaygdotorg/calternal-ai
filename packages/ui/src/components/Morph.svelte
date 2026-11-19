<script lang="ts">
  // A capsule that springs to the size of whatever it shows: a row of buttons, or a whole sheet.
  // The bottom bar is its one user: tabs, note tools, text bar, block bar, and every sheet. It is
  // anchored at its bottom edge, so sheets grow up out of the bar.
  //
  // `halo` adds a soft blur just around the capsule (#145): a capsule a little larger than the bar,
  // blurring what's behind it more towards its middle and fading out at its edges. It takes the
  // same size and spring as the capsule, so the two morph as one. It sits beside the capsule, not
  // inside it: the capsule's own glass would cut it off from the page it blurs.
  //
  // `away` hides the capsule in place (fades and shrinks a little), for a moment such as the phone
  // keyboard closing under it.
  //
  // The glass is a layer of its own with nothing inside it (#287). The capsule's contents sit on a
  // second layer above it. A morph's fades and a hover's tint only ever redraw that top layer: the
  // glass is never repainted by a hover, and never gains or loses a child layer mid-morph, so the
  // browser has no reason to rebuild its blur for a frame.
  import { tick, type Snippet } from "svelte";
  import { cubicOut } from "svelte/easing";
  import { modality } from "../lib/input";
  import { currentLayer, followLayer, measureLayer, type SizerDeps } from "../lib/morph-size";
  import { adaptive } from "../lib/material";
  import Glass from "./Glass.svelte";

  let {
    key,
    role = "toolbar",
    label,
    dragY = 0,
    dragging = false,
    halo = false,
    away = false,
    onsize,
    plain,
    content,
  }: {
    key: string;
    role?: string;
    label: string;
    dragY?: number;
    /** A finger is moving the sheet: it follows 1:1, with no spring behind it. */
    dragging?: boolean;
    halo?: boolean;
    away?: boolean;
    /** Told the capsule's new size as soon as it is known (its target, before the spring gets there). */
    onsize?: (w: number, h: number) => void;
    /** Layers that only fade, with no blur clearing (a tall sheet: a filter on a layer that big costs more than it shows). */
    plain?: (key: string) => boolean;
    content: Snippet<[string]>;
  } = $props();

  let size = $state<{ w: number; h: number } | null>(null);
  let box = $state<HTMLDivElement>();
  let haloEl = $state<HTMLDivElement>();
  let clipEl = $state<HTMLDivElement>();

  const morphMs = () => parseFloat(getComputedStyle(document.documentElement).getPropertyValue("--bar-morph-ms")) || 320;
  let ready = $state(false);

  /** What the capsule needs to follow a layer: the key it should show, its size, and how a new size springs in. */
  const deps: Pick<SizerDeps, "key" | "size" | "apply"> = {
    key: () => key,
    size: () => size,
    apply: (next) => {
      // A new size is a deliberate morph (new contents: a sheet opened, typing began): the capsule
      // springs to it and, being centred, its edges move. It says so on itself, until when, so a
      // layout-shift check can tell this motion from content jumping.
      if (box && size) box.dataset.morphUntil = String(Math.round(performance.now() + morphMs() + 150));
      // The glass and the clip move with the capsule's edges; they say so too.
      for (const el of [haloEl, clipEl]) if (el && box) el.dataset.morphUntil = box.dataset.morphUntil;
      size = next;
      onsize?.(next.w, next.h);
      if (!ready) requestAnimationFrame(() => requestAnimationFrame(() => (ready = true)));
    },
  };

  /** The capsule follows the layer shown now (lib/morph-size.ts): measured as it mounts, in the next frame and on every observer call, never from a replaced layer. */
  function measure(node: HTMLElement, k: string) {
    const f = followLayer(node, deps);
    return { destroy: f.stop };
  }

  // The key changed: look up the layer it shows and measure it, now (after the page has the new layer) and in the next frame.
  // Not left to the layer's own action and observer: when a sheet closed in the e2e, those never reported the new layer.
  $effect(() => {
    void key;
    const settle = () => {
      const el = currentLayer(clipEl, key);
      if (el) measureLayer(el, deps);
    };
    void tick().then(settle);
    const id = requestAnimationFrame(settle);
    return () => cancelAnimationFrame(id);
  });

  const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;

  // The bar becoming a sheet (and back) is one glass shape whose edges ride the sheet spring (IN4, IN8:
  // stiffness 400, damping 36, no bounce). `lastRole` is still the old one while the new layer is made.
  let lastRole = role;
  const sheetSwap = () => role === "dialog" || lastRole === "dialog";
  let sheetish = $state(role === "dialog");
  let sheetTimer = 0;
  $effect(() => {
    lastRole = role;
    clearTimeout(sheetTimer);
    if (role === "dialog") sheetish = true;
    else sheetTimer = window.setTimeout(() => (sheetish = false), 520);
  });
  // The new contents fade in as the old ones fade out, at once (a crossfade): a delay on the new ones
  // left the capsule empty for a few frames in the middle of the morph, which read as a flicker. Opacity and transform only, so nothing behind the glass repaints.
  // From the keyboard nothing animates (#344): no swap here, and the capsule's size snaps because it carries `data-instant` (styles/base.css).
  const isPlain = (node: Element) => !!plain?.((node as HTMLElement).dataset.key ?? "");
  // The three looks a layer can fade with (shared by enter and exit): opacity alone, opacity with the blur clearing, opacity with a small rise.
  const op = (t: number) => `opacity: ${t}`;
  const blur = (t: number) => `opacity: ${t}; filter: blur(${(1 - t) * 4}px)`;
  const rise = (t: number) => `opacity: ${t}; transform: translateY(${(1 - t) * 3}px)`;
  function enter(node: Element) {
    if (modality() === "key") return { duration: 0 };
    // Bar and sheet crossfade as soon as the new content exists. A handoff wait after building
    // a ready component adds latency and can leave the capsule empty between the two layers.
    // A tall sheet fades without the blur (#469): a filter over a whole sheet's layer is an extra full-size pass on
    // every frame of the morph, and the first frames are the ones that must not be late.
    if (sheetSwap() && !reduce) return { duration: 170, easing: cubicOut, css: isPlain(node) ? op : blur };
    return reduce ? { duration: 120, css: op } : { duration: 180, easing: cubicOut, css: rise };
  }
  function exit(node: Element) {
    if (modality() === "key") return { duration: 0 };
    if (sheetSwap() && !reduce) return { duration: 60, css: isPlain(node) ? op : blur };
    return { duration: reduce ? 80 : 100, css: op };
  }
</script>

<div class="morph-wrap" class:away>
  {#if halo && size}
    <div
      bind:this={haloEl}
      class="morph-halo"
     
      class:ready
      aria-hidden="true"
      style="width: {size.w}px; height: {size.h}px; {dragY ? `translate: 0 ${dragY}px;` : ''}"
    ><i></i><i></i><i></i></div>
  {/if}
  <div
    bind:this={box}
    use:adaptive
    class="morph"
   
    class:ready
    class:sheet={sheetish}
    {role}
    aria-label={label}
    style="{size ? `width: ${size.w}px; height: ${size.h}px;` : ''} {dragY ? `translate: 0 ${dragY}px;` : ''}{dragging ? ' transition: none;' : ''}"
  >
    <Glass dense={role === "dialog"} />
    <div bind:this={clipEl} class="morph-clip">
      {#key key}
        <div class="layer" data-key={key} use:measure={key} in:enter out:exit>
          {@render content(key)}
        </div>
      {/key}
    </div>
  </div>
</div>

<style>
  /* Holds the capsule and its halo. No fill, filter, clip or opacity here: any of them would cut
     both off from the page behind (a backdrop root). */
  .morph-wrap {
    position: relative;
    display: flex;
    pointer-events: none;
  }
  /* The capsule: only a size that springs. No fill, filter, clip, opacity or transform of its own:
     it holds the glass, and any of those would cut the glass off from the page (a backdrop root). */
  .morph {
    position: relative;
    isolation: isolate;
    pointer-events: auto;
    border-radius: 22px;
    min-width: 44px;
    min-height: 44px;
  }
  /* The glass is the shared floating material (styles/glass.css), with no children (see the top
     of this file). It stacks in document order under .morph-clip, so not `under`. A sheet (a
     dialog: Style, Insert, Info…) is lines of words and controls, so its glass takes the one
     denser step; the tint eases there with the capsule's own spring as the bar grows into it. */
  /* The contents, cut to the capsule's round shape. `clip`, not `hidden`: a focused control that
     sticks out can never scroll it sideways. */
  .morph-clip {
    position: absolute;
    inset: 0;
    border-radius: inherit;
    overflow: clip;
  }
  /* A sheet on an iPhone: a small radius on top, a much rounder one underneath, concentric with the
     display's corners (lib/sheet-radius.ts). The clip and the glass follow it (they inherit). */
  :global(:root[data-sheet-r="device"]) .morph[role="dialog"] {
    border-radius: var(--sheet-corners);
  }
  .morph.ready,
  .morph-halo.ready {
    transition-property: width, height, translate, scale;
    transition-duration: var(--bar-morph-ms);
    transition-timing-function: var(--bar-morph);
  }
  /* A sheet's edges ride the sheet spring (the top edge moves, the bottom stays). Its corners do not
     animate (#660: a radius is a repaint of the glass on every frame); they are the sheet's from the first
     frame, and while the capsule is still short the browser scales them down to fit, so it reads as a pill. */
  .morph.sheet.ready {
    transition-property: width, height, translate, scale;
    transition-duration: var(--bar-sheet-ms);
    transition-timing-function: var(--bar-sheet);
  }
  :global(.morph .glass),
  .morph-clip,
  .morph-halo i {
    transition: opacity 120ms var(--ease-out);
  }
  :global(.morph .glass) {
    transition:
      opacity 120ms var(--ease-out),
      background-color var(--bar-morph-ms) var(--bar-morph);
  }
  @media (prefers-reduced-motion: reduce) {
    .morph.ready,
    .morph.sheet.ready,
    .morph-halo.ready {
      transition-duration: 0ms;
    }
  }
  /* Hidden in place: the capsule shrinks a little and its glass and contents fade. The fade is on
     the glass layers themselves, never on the capsule, the halo or the wrap around them: those
     would cut the glass off from the page for as long as the fade lasts. */
  .away .morph,
  .away .morph-halo {
    scale: 0.96;
    transition-duration: 120ms;
  }
  .away :global(.morph .glass),
  .away .morph-clip,
  .away .morph-halo i {
    opacity: 0;
  }
  .away .morph {
    pointer-events: none;
  }

  /* The halo: as big as the capsule plus --halo-pad all round, anchored at the same bottom edge
     and centred, so it grows with it. Three layers, each blurrier and fading in further from the
     edge: blur that is strongest right around the bar and gone by the halo's edge. */
  .morph-halo {
    --pad: var(--halo-pad, 14px);
    position: absolute;
    left: 50%;
    bottom: calc(-1 * var(--pad));
    padding: var(--pad);
    box-sizing: content-box;
    transform: translateX(-50%);
    /* Always its own layer. Left to the browser, it got one only while it overlapped something
       else, so it flipped in and out of its own layer as the bar changed mode (#287). */
    will-change: transform;
    border-radius: calc(22px + var(--pad));
    pointer-events: none;
  }
  .morph-halo i {
    --f: var(--pad);
    position: absolute;
    inset: 0;
    border-radius: inherit;
    /* Soft on all four sides: a fade across the width, kept only where a fade across the height
       also keeps it. */
    -webkit-mask-image:
      linear-gradient(to right, transparent, #000 var(--f), #000 calc(100% - var(--f)), transparent),
      linear-gradient(to bottom, transparent, #000 var(--f), #000 calc(100% - var(--f)), transparent);
    -webkit-mask-composite: source-in;
    mask-image:
      linear-gradient(to right, transparent, #000 var(--f), #000 calc(100% - var(--f)), transparent),
      linear-gradient(to bottom, transparent, #000 var(--f), #000 calc(100% - var(--f)), transparent);
    mask-composite: intersect;
  }
  .morph-halo i:nth-child(1) {
    -webkit-backdrop-filter: blur(2px);
    backdrop-filter: blur(2px);
  }
  .morph-halo i:nth-child(2) {
    inset: calc(var(--pad) * 0.3);
    -webkit-backdrop-filter: blur(5px);
    backdrop-filter: blur(5px);
  }
  .morph-halo i:nth-child(3) {
    inset: calc(var(--pad) * 0.55);
    -webkit-backdrop-filter: blur(9px);
    backdrop-filter: blur(9px);
    background: color-mix(in oklab, var(--paper) 30%, transparent);
  }
  .layer {
    position: absolute;
    width: max-content;
  }
  .layer {
    left: 0;
    bottom: 0;
  }
</style>

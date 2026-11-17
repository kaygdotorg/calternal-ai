<script lang="ts">
  // A floating panel anchored to something (a button, a point). Drawn at the end of <body>, outside every clipping and stacking context of the app,
  // placed by lib/place.ts (flipped, shifted and height-limited so it always fits the window), shown only when its material is ready (lib/ready.ts),
  // closed by Escape or a press outside, with focus returned to `trigger`. Menus, the where-it-runs picker and context menus are built on it.
  // `morph` (needs `trigger`): with a pointer or touch driving and motion not reduced, the popover covers its button and grows out of the button's own shape, and shrinks back into it
  // on close, as Apple's ⋯ menus do (lib/clip-morph.ts: the spring, the capsule radius, the rows coming into focus). The button is hidden while the popover is open and swells back at
  // 160 ms into the close. From the keyboard, or under reduced motion, the popover is placed beside the button and only fades (nothing moves). Interruptible: reversing mid-morph
  // continues from the clip the glass has now.
  import { untrack, type Snippet } from "svelte";
  import { buttonSwell, clipMorph, morphAllowed, stopMorph } from "../lib/clip-morph";
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
    morph = false,
    blind,
    refit,
    trigger,
    onclose,
    children,
    ...rest
  }: { open: boolean; anchor: Anchor | null | undefined; label: string; role?: "dialog" | "menu" | "listbox"; prefer?: PlaceOptions["prefer"]; dense?: boolean; morph?: boolean; blind?: number; refit?: unknown; trigger?: HTMLElement | null; onclose: () => void; children: Snippet } & Record<string, unknown> = $props();
  let box = $state<HTMLElement>();
  // Closing keeps the panel for one short fade (`data-leaving`, glass.css) so it does not vanish in a frame; opening is instant once the material is ready.
  let leaving = $state(false);
  let present = $state(false);
  let words = $state<HTMLElement>();
  // This opening is a morph (decided when it opens, so a keyboard press mid-way cannot change the way back).
  let morphed = $state(false);
  let morphing = $state(false);
  const glassOf = () => box?.querySelector<HTMLElement>(":scope > .panel > .glass") ?? null;
  const parts = () => { const glass = glassOf(); return box && glass && words && trigger ? { box, glass, words, from: trigger } : null; };
  $effect(() => () => trigger?.removeAttribute("data-morph-open"));
  // `refit` changes: the content changed size or width after the first placement (a picker that has added its rows), so it is placed once more. The vertical edge is anchored, so this only re-clamps sideways.
  let before: { left: number; top: number; right: number; bottom: number } | null = null;
  $effect.pre(() => { if (open && blind && anchor) { const r = anchor instanceof Element ? anchor.getBoundingClientRect() : anchor; before = { left: r.left, top: r.top, right: r.right, bottom: r.bottom }; } });
  let again: () => void = () => {};
  $effect(() => { void refit; untrack(() => again()); });
  const handBack = () => { trigger?.removeAttribute("data-morph-open"); if (trigger) buttonSwell(trigger); };
  $effect(() => {
    if (open) { present = true; leaving = false; morphing = false; return; }
    if (!present) return;
    const m = untrack(() => (morphed ? parts() : null));
    if (m && morphAllowed()) {
      // Shrinks from wherever the glass is now (full size, or part way grown) into the button.
      leaving = true;
      morphing = true;
      let live = true;
      void clipMorph(m, false, { onReturn: handBack }).then(() => { if (live) { present = false; leaving = false; morphing = false; morphed = false; } });
      return () => { live = false; };
    }
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
    // A morph covers the button (the popover starts inside the button's own box); the keyboard's and reduced motion's way sits beside it.
    const wasMorphed = untrack(() => morphed);
    morphed = morph && !!trigger && morphAllowed();
    const cover = morphed;
    // A blind placement uses the anchor's rectangle as it was read before this surface was inserted (layout still clean), so nothing is measured after the insertion.
    const go = () => place(box!, blind && before ? before : anchor, { prefer, cover, blind });
    go();
    again = go;
    let f = 0;
    if (morphed) {
      // Reveal once the material is ready (the two frames `ready` waits for); the first morph frame is painted before the browser paints. Reopening mid-close continues from the glass's progress.
      f = requestAnimationFrame(() => { f = requestAnimationFrame(() => {
        const m = parts();
        if (!m) return;
        go();
        trigger!.setAttribute("data-morph-open", "");
        void clipMorph(m, true);
      }); });
    } else if (wasMorphed) { trigger?.removeAttribute("data-morph-open"); if (box) stopMorph(box); }
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
      again = () => {};
      cancelAnimationFrame(f);
      removeEventListener("pointerdown", away, true);
      removeEventListener("keydown", esc, true);
      removeEventListener("resize", go);
    };
  });
</script>

{#if present}
  <div class="pop" use:portal use:ready bind:this={box} data-leaving={leaving && !morphing ? "" : undefined} data-morph={morphed ? "" : undefined} {role} aria-label={label} {...rest}>
    <Panel radius="card" {dense}><div class="w" bind:this={words}>{@render children()}</div></Panel>
  </div>
{/if}

<style>
  .pop { position: fixed; left: 0; top: 0; z-index: 60; overscroll-behavior: contain; scrollbar-width: none; }
  .pop :global(.panel) { min-width: 100%; }
  .w { display: block; }
  /* A morphing popover is revealed by a clip, so it does not also scale up from small when it becomes ready. */
  .pop[data-morph]:global([data-surface]:not([data-ready])) { scale: 1; }
</style>

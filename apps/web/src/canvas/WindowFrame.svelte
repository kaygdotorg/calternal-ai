<script lang="ts">
  // One window of the canvas: an opaque inner card whose colour comes from the wallpaper (surface.ts), or a frosted one (the adaptive floating material). A slim bar names the app and holds the window's actions (split to the side,
  // split below, frost, maximise, close); the input lock hangs on the top frame line at its middle, as in infraternal's canvas: invisible until the pointer is near the top edge (35 %), and when locked it stays at 90 % in the accent.
  // The right edge and the bottom edge resize it. Focus follows the press. Everything here is plain CSS and pointer events; nothing is measured while the strip pans.
  import type { Snippet } from "svelte";
  import { adaptive, Glass, Icon } from "@calternal/ui";
  import type { IconName } from "@calternal/ui";
  import { triplet, type WindowColours } from "./surface";
  import type { Win } from "./layout";
  let { win, title, icon, focused, maximised, frosted, colours, hasBelow, onfocus, onaction, onresizew, onresizeh, children }: {
    win: Win; title: string; icon: IconName; focused: boolean; maximised: boolean; frosted: boolean; colours: WindowColours; hasBelow: boolean;
    onfocus: () => void; onaction: (a: "right" | "below" | "frost" | "max" | "close" | "lock") => void; onresizew: (dx: number) => void; onresizeh: (dy: number) => void; children: Snippet;
  } = $props();
  const style = $derived(frosted || maximised ? undefined : `--win-bg: rgb(${triplet(colours.bg)}); --paper: rgb(${triplet(colours.bg)}); --ink: rgb(${triplet(colours.ink)}); --ink-rgb: ${triplet(colours.ink)}; --ink-2: rgb(${triplet(colours.ink2)}); --ink-3: rgb(${triplet(colours.ink3)});`);

  /** Drags an edge: reports the movement since the press until release. */
  function drag(e: PointerEvent, report: (d: number) => void, axis: "x" | "y") {
    e.preventDefault(); e.stopPropagation();
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
    let last = axis === "x" ? e.clientX : e.clientY;
    const move = (ev: PointerEvent) => { const p = axis === "x" ? ev.clientX : ev.clientY; report(p - last); last = p; };
    const up = () => { el.removeEventListener("pointermove", move); el.removeEventListener("pointerup", up); el.removeEventListener("pointercancel", up); };
    el.addEventListener("pointermove", move); el.addEventListener("pointerup", up); el.addEventListener("pointercancel", up);
  }
  /** The padlock appears near the top edge (the frame's 44 px hit area around its middle). */
  let near = $state(false);
  const nearTop = (e: PointerEvent) => { const r = (e.currentTarget as HTMLElement).getBoundingClientRect(); near = e.clientY - r.top < 44 && Math.abs(e.clientX - (r.left + r.width / 2)) < 90; };
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<section class="win" class:frosted class:maximised data-win={win.id} data-kind={win.kind} data-focused={focused ? "" : undefined} data-locked={win.locked ? "" : undefined} data-near={near || win.locked ? "" : undefined} {style} aria-label="{title} window" onpointerdowncapture={onfocus} onpointermove={nearTop} onpointerleave={() => (near = false)}>
  {#if frosted && !maximised}<div use:adaptive={"card"} class="mat"><Glass /></div>{/if}
  <button type="button" class="lock" aria-pressed={win.locked} aria-label={win.locked ? "Unlock input" : "Lock input"} title={win.locked ? "Unlock input" : "Lock input"} onclick={() => onaction("lock")}>
    <svg viewBox="0 0 24 24" aria-hidden="true"><path class="shackle" d="M8.5 11V8a3.5 3.5 0 0 1 7 0v3" /><rect x="5" y="11" width="14" height="9.5" rx="2.5" /></svg>
  </button>
  <header class="bar">
    <span class="ttl"><Icon inline name={icon} /><span class="nm">{title}</span></span>
    <span class="acts">
      <button type="button" aria-label="Open another beside this one" title="Split to the side" onclick={() => onaction("right")}><Icon inline name="sidebar-right" /></button>
      <button type="button" aria-label="Open another below this one" title="Split below" onclick={() => onaction("below")}><Icon inline name="down" /></button>
      <button type="button" aria-label={frosted ? "Make this window opaque" : "Make this window frosted"} aria-pressed={frosted} title="Frosted" onclick={() => onaction("frost")}><Icon inline name="spark" /></button>
      <button type="button" aria-label={maximised ? "Restore the window" : "Maximise the window"} aria-pressed={maximised} title={maximised ? "Restore" : "Maximise"} onclick={() => onaction("max")}><Icon inline name={maximised ? "close" : "rail"} /></button>
      <button type="button" aria-label="Close the window" title="Close" onclick={() => onaction("close")}><Icon inline name="close" /></button>
    </span>
  </header>
  <div class="body">{@render children()}</div>
  {#if !maximised}
    <div class="rz r" role="separator" aria-orientation="vertical" aria-label="Resize the window's width" onpointerdown={(e) => drag(e, onresizew, "x")}></div>
    {#if hasBelow}<div class="rz b" role="separator" aria-orientation="horizontal" aria-label="Resize the window's height" onpointerdown={(e) => drag(e, onresizeh, "y")}></div>{/if}
  {/if}
</section>

<style>
  /* The inner card: its own radius (Craft's document corner, measured 12 pt) and a soft shadow; focus is a hairline in the accent, not a glow. */
  .win { position: relative; isolation: isolate; height: 100%; min-height: 0; display: flex; flex-direction: column; border-radius: var(--canvas-r, 14px); background: var(--win-bg, transparent); color: var(--ink); box-shadow: 0 1px 2px rgb(0 0 0 / 0.14), 0 6px 22px rgb(0 0 0 / 0.16); transition: box-shadow var(--dur-fast) var(--ease-out); }
  .win[data-focused] { box-shadow: 0 0 0 1.5px color-mix(in oklab, var(--accent-base) 70%, transparent), 0 8px 28px rgb(0 0 0 / 0.22); }
  .win.maximised { border-radius: 0; box-shadow: none; background: transparent; }
  .mat { position: absolute; inset: 0; border-radius: inherit; z-index: -1; pointer-events: none; }
  .bar { flex: none; height: 32px; display: flex; align-items: center; justify-content: space-between; gap: var(--s-3); padding: 0 var(--s-3) 0 var(--s-5); color: var(--ink-3); font-size: var(--text-label); font-weight: var(--strong-weight); }
  .ttl { display: inline-flex; align-items: center; gap: var(--s-2); min-width: 0; }
  .nm { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .acts { display: inline-flex; gap: 2px; opacity: 0; transition: opacity var(--dur-fast) var(--ease-out); }
  .acts button { display: grid; place-items: center; width: 24px; height: 24px; border-radius: var(--r-select); color: inherit; }
  .win[data-focused] .acts, .win:focus-within .acts { opacity: 1; }
  @media (hover: hover) and (pointer: fine) { .win:hover .acts { opacity: 1; } .acts button:hover { background: var(--hover); color: var(--ink); } }
  .body { position: relative; flex: 1; min-height: 0; overflow: hidden; border-radius: 0 0 inherit inherit; }
  /* Apps draw their own header controls against the card edge: inside a window the body is its own positioning context and its own scroller. */
  .body > :global(*) { max-height: 100%; }
  .rz { position: absolute; z-index: 5; touch-action: none; }
  .rz.r { top: 12px; bottom: 12px; right: -7px; width: 14px; cursor: ew-resize; }
  .rz.b { left: 12px; right: 12px; bottom: -7px; height: 14px; cursor: ns-resize; }
  .rz::after { content: ""; position: absolute; inset: 5px; border-radius: 2px; background: var(--accent-base); opacity: 0; transition: opacity var(--dur-fast) var(--ease-out); }
  .rz.r::after { left: 5px; right: 5px; } .rz.b::after { top: 5px; bottom: 5px; }
  @media (hover: hover) and (pointer: fine) { .rz:hover::after { opacity: 0.55; } }
  /* The padlock (infraternal D177, packages/ui styles.css .ui-window-lock): centred on the top frame line, shackle above it and body below; a 44 px hit area, no chip. */
  .lock { position: absolute; z-index: 6; top: -22px; left: 50%; width: 44px; height: 44px; margin-left: -22px; display: grid; place-items: center; border-radius: var(--r-select); color: var(--ink); opacity: 0; pointer-events: none; transition: opacity var(--dur-fast) var(--ease-out), color var(--dur-fast) var(--ease-out); }
  .lock svg { width: 18px; height: 18px; fill: none; stroke: currentColor; stroke-width: 1.7; stroke-linecap: round; stroke-linejoin: round; overflow: visible; }
  .shackle { transform-origin: 8.5px 11px; transition: transform var(--dur-base) var(--ease-out); transform: translateY(-3px) rotate(-22deg); }
  .win[data-locked] .shackle { transform: none; }
  .win[data-near] .lock { opacity: 0.35; pointer-events: auto; }
  .win[data-locked] .lock { opacity: 0.9; color: var(--accent-base); pointer-events: auto; }
  .lock:focus-visible { opacity: 0.8; pointer-events: auto; }
  @media (hover: hover) and (pointer: fine) { .win .lock:hover { opacity: 0.8; } }
  /* Touch has no hover: the bar's actions and a faint padlock are always there. */
  @media (pointer: coarse) { .acts { opacity: 1; } .lock { opacity: 0.35; pointer-events: auto; } .win[data-locked] .lock { opacity: 0.9; } }
</style>

<script lang="ts">
  // One window of the canvas (docs/ui-shell-status.md, "The rules"): an inner card with its app inside. One pill at its top right holds exactly three buttons (full screen, share, the three-dot menu), and one title pill sits at its top left
  // (the Threads view draws its own, with the state and the parent's name; any other app gets this one), so a window never has two titles. Opaque: the wallpaper-derived colour (surface.ts) with a hairline edge and a soft shadow. Frosted: the old Calternal's
  // frosted card (frost.ts): one glass layer, blur 20 px and saturation 140 %, the page's paper laid over it at the least density that still reads. The input lock hangs on the top frame line at its middle (infraternal D177). The right edge and the
  // boundary below it resize between neighbours. Every control has the warm tooltip with its shortcut. Focus follows the press.
  import { setContext, type Snippet } from "svelte";
  import { Icon, Panel, tip } from "@calternal/ui";
  import type { IconName } from "@calternal/ui";
  import { triplet, type WindowColours } from "./surface";
  import type { Win } from "./layout";
  export type WinAction = "max" | "share" | "menu" | "lock";
  let { win, title, icon, ownTitle, focused, maximised, frosted, colours, paper, density, hasBelow, edge, kbd, onfocus, onaction, onresizew, onresizeh, children }: {
    win: Win; title: string; icon: IconName; /** The app draws its own title pill (Threads). */ ownTitle: boolean; focused: boolean; maximised: boolean; frosted: boolean; colours: WindowColours;
    /** The scheme's paper as `r g b` and the frost density over it (frost.ts). */ paper: string; density: number; hasBelow: boolean; /** Which viewport edge this window touches, for the shell's sidebar pills. */ edge: "l" | "r" | "lr" | "";
    kbd: Record<string, string>; onfocus: () => void; onaction: (a: WinAction, source: "pointer" | "key", el: HTMLElement) => void; onresizew: (dx: number) => void; onresizeh: (dy: number) => void; children: Snippet;
  } = $props();
  setContext("canvas-window", true);
  const style = $derived(
    frosted && !maximised ? `--fpaper: ${paper}; --fd: ${density};`
    : maximised ? undefined
    : `--win-bg: rgb(${triplet(colours.bg)}); --paper: rgb(${triplet(colours.bg)}); --ink: rgb(${triplet(colours.ink)}); --ink-rgb: ${triplet(colours.ink)}; --ink-2: rgb(${triplet(colours.ink2)}); --ink-3: rgb(${triplet(colours.ink3)});`,
  );
  const act = (a: WinAction) => (e: MouseEvent) => onaction(a, e.detail === 0 ? "key" : "pointer", e.currentTarget as HTMLElement);

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
<section class="win" class:frosted class:maximised data-win={win.id} data-kind={win.kind} data-item={win.item} data-edge={edge || undefined} data-focused={focused ? "" : undefined} data-locked={win.locked ? "" : undefined} data-near={near || win.locked ? "" : undefined} {style} aria-label="{title} window" onpointerdowncapture={onfocus} onpointermove={nearTop} onpointerleave={() => (near = false)}>
  {#if frosted && !maximised}<div class="fg" aria-hidden="true"></div>{/if}
  <button type="button" class="lock" aria-pressed={win.locked} aria-label={win.locked ? "Unlock input" : "Lock input"} use:tip={{ label: win.locked ? "Unlock input" : "Lock input", kbd: kbd.lock, side: "bottom" }} onclick={act("lock")}>
    <svg viewBox="0 0 24 24" aria-hidden="true"><path class="shackle" d="M8.5 11V8a3.5 3.5 0 0 1 7 0v3" /><rect x="5" y="11" width="14" height="9.5" rx="2.5" /></svg>
  </button>
  {#if !ownTitle}
    <div class="tl"><Panel radius="pill" class="wt"><Icon name={icon} inline /><span class="nm">{title}</span></Panel></div>
  {/if}
  <div class="tr">
    <Panel radius="pill" class="wacts">
      <button type="button" aria-label={maximised ? "Restore the window" : "Full screen"} aria-pressed={maximised} use:tip={{ label: maximised ? "Restore" : "Full screen", kbd: kbd.max, side: "bottom" }} onclick={act("max")}><Icon name={maximised ? "restore" : "expand"} /></button>
      <button type="button" aria-label="Share" use:tip={{ label: "Share", side: "bottom" }} onclick={act("share")}><Icon name="link" /></button>
      <button type="button" aria-label="More" aria-haspopup="menu" use:tip={{ label: "More", side: "bottom" }} onclick={act("menu")}><Icon name="more" /></button>
    </Panel>
  </div>
  <div class="body">{@render children()}</div>
  {#if !maximised}
    <div class="rz r" role="separator" aria-orientation="vertical" aria-label="Resize the window's width" onpointerdown={(e) => drag(e, onresizew, "x")}></div>
    {#if hasBelow}<div class="rz b" role="separator" aria-orientation="horizontal" aria-label="Resize the window's height" onpointerdown={(e) => drag(e, onresizeh, "y")}></div>{/if}
  {/if}
</section>

<style>
  /* The inner card: Craft's document corner (measured about 12 pt) and a soft shadow; the opaque one has a hairline edge so it separates from a wallpaper of nearly its own tone; focus is a hairline in the accent, not a glow. */
  /* A window is narrower than the card the wide-chat paddings were made for: the message column uses the ordinary region padding inside it. */
  .win { --safe-top: 0px; --msg-pad-l: var(--pad-region); --msg-pad-r: var(--pad-region); position: relative; isolation: isolate; height: 100%; min-height: 0; border-radius: var(--canvas-r, 14px); background: var(--win-bg, transparent); color: var(--ink); box-shadow: 0 0 0 1px rgb(var(--ink-rgb) / 0.12), 0 1px 2px rgb(0 0 0 / 0.14), 0 8px 24px rgb(0 0 0 / 0.18); transition: box-shadow var(--dur-fast) var(--ease-out); }
  .win[data-focused] { box-shadow: 0 0 0 1.5px color-mix(in oklab, var(--accent-base) 70%, transparent), 0 10px 28px rgb(0 0 0 / 0.24); }
  .win.maximised { border-radius: 0; box-shadow: none; background: transparent; }
  /* The old Calternal's frosted card (panes.css `.fg-glass`): one layer under the whole card; no neutral glass under it (k = 0), the page's paper at the density that still reads, then blur 20 px and saturate 140 %. */
  .win.frosted { background: none; box-shadow: 0 0 0 1px rgb(255 255 255 / 0.14), 0 8px 24px rgb(0 0 0 / 0.18); }
  .win.frosted[data-focused] { box-shadow: 0 0 0 1.5px color-mix(in oklab, var(--accent-base) 70%, transparent), 0 10px 28px rgb(0 0 0 / 0.24); }
  .fg { position: absolute; inset: 0; z-index: -1; border-radius: inherit; pointer-events: none; background-image: linear-gradient(color-mix(in oklab, rgb(var(--fpaper)) calc(var(--fd) * 100%), transparent) 0 0); -webkit-backdrop-filter: blur(20px) saturate(140%); backdrop-filter: blur(20px) saturate(140%); }
  /* A size container: what a window draws that must fit it (the request bar) is limited by its own height (`cqh`), not the screen's. */
  .body { position: absolute; inset: 0; overflow: hidden; border-radius: inherit; container-type: size; }
  /* The two pills float over the app's own content (its header bar is empty space of the same height): left the title, right the three buttons. Beside the shell's sidebar pills the pills move over by their width. */
  .tl, .tr { position: absolute; z-index: 5; top: var(--s-4); display: flex; }
  .tl { left: var(--s-4); }
  .tr { right: var(--s-4); }
  .win[data-edge~="l"] .tl, .win[data-edge="lr"] .tl { margin-left: var(--edge-l, 0px); }
  .win[data-edge~="r"] .tr, .win[data-edge="lr"] .tr { margin-right: var(--edge-r, 0px); }
  :global(.wt) { display: inline-flex; align-items: center; gap: var(--s-3); height: var(--pill-h); max-width: 100%; padding: 0 var(--s-5); font-size: var(--text-ui); font-weight: var(--strong-weight); }
  .nm { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  :global(.wacts) { display: inline-flex; align-items: center; height: var(--pill-h); padding: 0 var(--s-2); gap: 2px; }
  .tr button { display: inline-grid; place-items: center; width: calc(var(--pill-h) - var(--s-2)); height: calc(var(--pill-h) - var(--s-2)); border-radius: 50%; color: var(--ink-2); transition: background-color var(--dur-fast) var(--ease-out), color var(--dur-fast) var(--ease-out); }
  @media (hover: hover) and (pointer: fine) { .tr button:hover { background: var(--hover); color: var(--ink); } }
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
  /* Touch has no hover: a faint padlock is always there. */
  @media (pointer: coarse) { .lock { opacity: 0.35; pointer-events: auto; } .win[data-locked] .lock { opacity: 0.9; } }
</style>

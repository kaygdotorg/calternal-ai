<script lang="ts">
  // One window of the canvas (docs/ui-shell-status.md, "The rules"): the shared middle `Card` (ui/Card.svelte, the same component, class and rules as canvas=0's middle card, so surface, radius, material and the app's own components are literally the same code) with the window's
  // own controls on it. Nothing here restyles the card. A window is a pane card (the wallpaper's blurred copy under the card veil), a transparent card (the paper at a density over what is behind: frost.ts) or a solid one (the paper). All three are neutral: nothing is tinted with the
  // wallpaper's colour. The wallpaper picture of a pane card is the card's `moving` child, which the canvas moves against the strip so it stays still behind the card (Canvas.svelte `place`).
  // One pill at the top right holds exactly three buttons (full screen, share, the three-dot menu) where canvas=0's header keeps its menu pill, and one title pill sits at the top left (the Threads view draws its own, so a window never has two titles). The input lock hangs on the top
  // frame line at its middle (infraternal D177). The right edge and the boundary below it resize between neighbours. Every control has the warm tooltip with its shortcut. Focus follows the press. The window that is not focused is dimmed by a neutral veil and its controls recede; the
  // focused one has no ring (both changes ease on the motion tokens and can be interrupted).
  import { setContext, type Snippet } from "svelte";
  import { Card, Icon, Panel, tip } from "@calternal/ui";
  import type { IconName } from "@calternal/ui";
  import type { Look, Win } from "./layout";
  export type WinAction = "max" | "share" | "menu" | "lock";
  let { win, title, icon, ownTitle, focused, maximised, look, paper, density, hasBelow, edge, kbd, onfocus, onaction, onresizew, onresizeh, children }: {
    win: Win; title: string; icon: IconName; /** The app draws its own title pill (Threads). */ ownTitle: boolean; focused: boolean; maximised: boolean; look: Look;
    /** The scheme's paper as `r g b` and the density of a transparent window (frost.ts). */ paper: string; density: number; hasBelow: boolean; /** Which edge of the area this window touches, for the shell's sidebar pills. */ edge: "l" | "r" | "lr" | "";
    kbd: Record<string, string>; onfocus: () => void; onaction: (a: WinAction, source: "pointer" | "key", el: HTMLElement) => void; onresizew: (dx: number) => void; onresizeh: (dy: number) => void; children: Snippet;
  } = $props();
  setContext("canvas-window", true);
  const variant = $derived(maximised || look === "card" ? "pane" : look === "clear" ? "clear" : "solid");
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
<Card label="{title} window" {variant} moving class={maximised ? "mid-card win maximised" : "mid-card win"} data-win={win.id} data-kind={win.kind} data-item={win.item} data-look={maximised ? "card" : look} data-edge={edge || undefined} data-focused={focused ? "" : undefined}
  data-locked={win.locked ? "" : undefined} data-near={near || win.locked ? "" : undefined} data-max={maximised ? "" : undefined} style={look === "clear" && !maximised ? `--fpaper: ${paper}; --fd: ${density};` : undefined} onpointerdowncapture={onfocus} onpointermove={nearTop} onpointerleave={() => (near = false)}>
  <button type="button" class="lock" aria-pressed={win.locked} aria-label={win.locked ? "Unlock input" : "Lock input"} use:tip={{ label: win.locked ? "Unlock input" : "Lock input", kbd: kbd.lock, side: "bottom" }} onclick={act("lock")}>
    <svg viewBox="0 0 24 24" aria-hidden="true"><path class="shackle" d="M8.5 11V8a3.5 3.5 0 0 1 7 0v3" /><rect x="5" y="11" width="14" height="9.5" rx="2.5" /></svg>
  </button>
  {#if !ownTitle}
    <div class="tl"><Panel radius="pill" class="wt"><Icon name={icon} inline /><span class="nm">{title}</span></Panel></div>
  {/if}
  <div class="tr">
    <Panel radius="pill" class="wacts">
      <button type="button" aria-label={maximised ? "Restore the window" : "Full screen"} aria-pressed={maximised} use:tip={{ label: maximised ? "Restore" : "Full screen", kbd: kbd.max, side: "bottom" }} onclick={act("max")}><Icon name={maximised ? "restore" : "expand"} /></button>
      <button type="button" aria-label="Share" aria-haspopup="menu" use:tip={{ label: "Share", side: "bottom" }} onclick={act("share")}><Icon name="share" /></button>
      <button type="button" aria-label="More" aria-haspopup="menu" use:tip={{ label: "More", side: "bottom" }} onclick={act("menu")}><Icon name="more" /></button>
    </Panel>
  </div>
  <div class="body">{@render children()}</div>
  <div class="dim" aria-hidden="true"></div>
</Card>
{#if !maximised}
  <div class="rz r" role="separator" aria-orientation="vertical" aria-label="Resize the window's width" onpointerdown={(e) => drag(e, onresizew, "x")}></div>
  {#if hasBelow}<div class="rz b" role="separator" aria-orientation="horizontal" aria-label="Resize the window's height" onpointerdown={(e) => drag(e, onresizeh, "y")}></div>{/if}
{/if}

<style>
  /* The card itself is the shared one; these rules only place the window's own parts. The slot gives it its height. */
  :global(.card.win) { height: 100%; }
  /* Where canvas=0's card keeps the app (above the app switcher's row), so a single window lays out exactly as the card does. */
  .body { position: absolute; inset: 0; }
  /* Inactive windows: a neutral veil (black, never the wallpaper's colour) over the content, under the controls; the controls recede. Both ease on the motion tokens (instant from the keyboard: the shell's rule). */
  .dim { position: absolute; inset: 0; z-index: 4; pointer-events: none; opacity: 0; background: var(--dim-veil); border-radius: inherit; transition: opacity var(--dur-base) var(--ease-out); }
  :global(:root) { --dim-veil: rgb(0 0 0 / 0.07); }
  :global(:root[data-scheme="dark"]) { --dim-veil: rgb(0 0 0 / 0.2); }
  :global(.card.win:not([data-focused])) .dim { opacity: 1; }
  .tl, .tr { transition: opacity var(--dur-base) var(--ease-out); }
  :global(.card.win:not([data-focused])) :is(.tl, .tr) { opacity: 0.6; }
  /* Where the app's own header keeps its pills (a bar-high row under the safe area, `--pad-region` from the sides), so the window's controls line up with the title pill the Threads view draws. */
  .tl, .tr { position: absolute; z-index: 5; top: calc(var(--safe-top) + (var(--bar-h) - var(--pill-h)) / 2); display: flex; }
  .tl { left: var(--pad-region); }
  .tr { right: var(--pad-region); }
  :global(.card.win[data-edge~="l"]) .tl, :global(.card.win[data-edge="lr"]) .tl { margin-left: var(--edge-l, 0px); }
  :global(.card.win[data-edge~="r"]) .tr, :global(.card.win[data-edge="lr"]) .tr { margin-right: var(--edge-r, 0px); }
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
  /* The padlock (infraternal D177, packages/ui styles.css .ui-window-lock): centred at the top edge, the shackle above its body; a 44 px hit area, no chip. */
  .lock { position: absolute; z-index: 6; top: 2px; left: 50%; width: 44px; height: 44px; margin-left: -22px; display: grid; place-items: center; border-radius: var(--r-select); color: var(--ink); opacity: 0; pointer-events: none; transition: opacity var(--dur-fast) var(--ease-out), color var(--dur-fast) var(--ease-out); }
  .lock svg { width: 18px; height: 18px; fill: none; stroke: currentColor; stroke-width: 1.7; stroke-linecap: round; stroke-linejoin: round; overflow: visible; }
  .shackle { transform-origin: 8.5px 11px; transition: transform var(--dur-base) var(--ease-out); transform: translateY(-3px) rotate(-22deg); }
  :global(.win[data-locked]) .shackle { transform: none; }
  :global(.win[data-near]) .lock { opacity: 0.35; pointer-events: auto; }
  :global(.win[data-locked]) .lock { opacity: 0.9; color: var(--accent-base); pointer-events: auto; }
  .lock:focus-visible { opacity: 0.8; pointer-events: auto; }
  @media (hover: hover) and (pointer: fine) { :global(.win) .lock:hover { opacity: 0.8; } }
  /* Touch has no hover, and on a phone the title pill is at the top middle where the padlock hangs: the padlock shows there only while locked (the window menu has Lock input). */
  @media (pointer: coarse) { :global(.win[data-locked]) .lock { opacity: 0.9; } }
</style>

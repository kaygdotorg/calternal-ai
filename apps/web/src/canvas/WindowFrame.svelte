<script lang="ts">
  // One window of the canvas (docs/ui-shell-status.md, "The rules"): the shared middle `Card` (ui/Card.svelte, the same component, class and rules as canvas=0's middle card, so surface, radius, material and the app's own components are literally the same code) with the window's
  // own controls on it. Nothing here restyles the card. A window is a pane card (the wallpaper's blurred copy under the card veil), a transparent card (the paper at a density over what is behind: frost.ts) or a solid one (the paper). All three are neutral: nothing is tinted with the
  // wallpaper's colour. The wallpaper picture of a pane card is the card's `moving` child, which the canvas moves against the strip so it stays still behind the card (Canvas.svelte `place`).
  // One pill at the top right holds exactly three buttons (full screen, share, the three-dot menu) where canvas=0's header keeps its menu pill, and one title pill sits at the top left (the Threads view draws its own, so a window never has two titles). The input lock hangs on the top
  // frame line at its middle (infraternal D177). The right edge and the boundary below it resize between neighbours. Every control has the warm tooltip with its shortcut. Focus follows the press. The window that is not focused is dimmed by a neutral veil and its controls recede; the
  // focused one has no ring (both changes ease on the motion tokens and can be interrupted).
  import { setContext, type Snippet } from "svelte";
  import { Card, Grip, Icon, Panel, tip } from "@calternal/ui";
  import type { IconName } from "@calternal/ui";
  import type { Look, Win } from "./layout";
  export type WinAction = "max" | "share" | "menu" | "lock";
  let { win, title, nth = 1, wNow, hNow, icon, ownTitle, focused, maximised, canMax, look, paper, density, hasBelow, edge, kbd, onfocus, onaction, onresizew, onresizeh, children }: {
    win: Win; title: string; /** Which of several windows with this title this is (1 for the first): the landmark labels must differ. */ nth?: number; /** The column's width and the window's height share, in percent: the resize handles' current values. */ wNow: number; hNow: number; icon: IconName; /** The app draws its own title pill (Threads). */ ownTitle: boolean; focused: boolean; maximised: boolean; /** There is another window to be full screen among: a lone window is full already, so it has no full screen control. */ canMax: boolean; look: Look;
    /** The scheme's paper as `r g b` and the density of a transparent window (frost.ts). */ paper: string; density: number; hasBelow: boolean; /** Which edge of the area this window touches, for the shell's sidebar pills. */ edge: "l" | "r" | "lr" | "";
    kbd: Record<string, string>; onfocus: () => void; onaction: (a: WinAction, source: "pointer" | "key", el: HTMLElement) => void; onresizew: (dx: number) => void; onresizeh: (dy: number) => void; children: Snippet;
  } = $props();
  setContext("canvas-window", true);
  /** Whether this is the focused window: the composer of the focused window alone carries the page's `composer-field` id (ids are unique; shortcuts and "Write a message" reach the window in use). */
  setContext("canvas-focused", () => focused);
  const variant = $derived(maximised || look === "card" ? "pane" : look === "clear" ? "clear" : "solid");
  /** The three-dot menu is named for what is in the window, the same name the single card's header gives it ("Thread actions"). */
  const more = $derived(win.kind === "threads" ? "Thread actions" : win.kind === "notes" ? "Note actions" : "More");
  const act = (a: WinAction) => (e: MouseEvent) => onaction(a, e.detail === 0 ? "key" : "pointer", e.currentTarget as HTMLElement);
  /** The shared Grip reports pointer positions; a window wants the movement since the last one. */
  let last = 0;
  const startAt = (axis: "x" | "y") => (e: PointerEvent) => { last = axis === "x" ? e.clientX : e.clientY; };
  const moveBy = (axis: "x" | "y", report: (d: number) => void) => (e: PointerEvent) => { const p = axis === "x" ? e.clientX : e.clientY; report(p - last); last = p; };
  const keyBy = (grow: string, shrink: string, report: (d: number) => void) => (e: KeyboardEvent) => { const step = e.shiftKey ? 64 : 16; if (e.key === grow) report(step); else if (e.key === shrink) report(-step); else return; e.preventDefault(); };
  /** The padlock appears near the top edge (the frame's 44 px hit area around its middle). */
  let near = $state(false);
  const nearTop = (e: PointerEvent) => { const r = (e.currentTarget as HTMLElement).getBoundingClientRect(); near = e.clientY - r.top < 44 && Math.abs(e.clientX - (r.left + r.width / 2)) < 90; };
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<Card label="{title} window{nth > 1 ? ` ${nth}` : ''}" {variant} moving class={maximised ? "mid-card win maximised" : "mid-card win"} data-win={win.id} data-kind={win.kind} data-item={win.item} data-look={maximised ? "card" : look} data-edge={edge || undefined} data-focused={focused ? "" : undefined}
  data-locked={win.locked ? "" : undefined} data-near={near || win.locked ? "" : undefined} data-max={maximised ? "" : undefined} style={look === "clear" && !maximised ? `--fpaper: ${paper}; --fd: ${density};` : undefined} onpointerdowncapture={onfocus} onpointermove={nearTop} onpointerleave={() => (near = false)}>
  <button type="button" class="lock" aria-pressed={win.locked} aria-label={win.locked ? "Unlock input" : "Lock input"} use:tip={{ label: win.locked ? "Unlock input" : "Lock input", kbd: kbd.lock, side: "bottom" }} onclick={act("lock")}>
    <svg viewBox="0 0 24 24" aria-hidden="true"><path class="shackle" d="M8.5 11V8a3.5 3.5 0 0 1 7 0v3" /><rect x="5" y="11" width="14" height="9.5" rx="2.5" /></svg>
  </button>
  {#if !ownTitle}
    <div class="tl"><Panel radius="pill" class="wt"><Icon name={icon} inline /><span class="nm">{title}</span></Panel></div>
  {/if}
  <div class="tr">
    <Panel radius="pill" class="wacts">
      {#if canMax || maximised}<button type="button" aria-label={maximised ? "Restore the window" : "Full screen"} aria-pressed={maximised} use:tip={{ label: maximised ? "Restore" : "Full screen", kbd: kbd.max, side: "bottom" }} onclick={act("max")}><Icon name={maximised ? "restore" : "expand"} /></button>{/if}
      <button type="button" aria-label="Share" aria-haspopup="menu" use:tip={{ label: "Share", side: "bottom" }} onclick={act("share")}><Icon name="share" /></button>
      <button type="button" aria-label={more} aria-haspopup="menu" use:tip={{ label: more, side: "bottom" }} onclick={act("menu")}><Icon name="more" /></button>
    </Panel>
  </div>
  <div class="body">{@render children()}</div>
  <div class="dim" aria-hidden="true"></div>
</Card>
{#if !maximised}
  <!-- The same handle the sidebars use (ui/Grip), in the gap to the right of the window and, when another window is stacked below, in the gap under it. -->
  <Grip class="rz" label="Resize the window's width" min={20} max={100} now={wNow} style="right: calc(var(--s-7) / -2 - 4px)" onstart={startAt("x")} onmove={moveBy("x", onresizew)} onkey={keyBy("ArrowRight", "ArrowLeft", onresizew)} />
  {#if hasBelow}<Grip class="rz" orientation="horizontal" label="Resize the window's height" min={18} max={100} now={hNow} style="bottom: calc(var(--s-7) / -2 - 4px)" onstart={startAt("y")} onmove={moveBy("y", onresizeh)} onkey={keyBy("ArrowDown", "ArrowUp", onresizeh)} />{/if}
{/if}

<style>
  /* The card itself is the shared one; these rules only place the window's own parts. The slot gives it its height. */
  :global(.card.win) { height: 100%; }
  /* The app fills the window: the app switcher is not on the canvas (it is in the left sidebar), so windows use the full height. */
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
  :global(.wt) { display: inline-flex; align-items: center; gap: var(--s-3); height: var(--pill-h); max-width: 100%; padding: 0 var(--s-5); font-size: var(--text-ui); font-weight: var(--strong-weight); }
  .nm { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  :global(.wacts) { display: inline-flex; align-items: center; height: var(--pill-h); padding: 0 var(--s-2); gap: 2px; }
  .tr button { display: inline-grid; place-items: center; width: calc(var(--pill-h) - var(--s-2)); height: calc(var(--pill-h) - var(--s-2)); border-radius: 50%; color: var(--ink-2); transition: background-color var(--dur-fast) var(--ease-out), color var(--dur-fast) var(--ease-out); }
  @media (hover: hover) and (pointer: fine) { .tr button:hover { background: var(--hover); color: var(--ink); } }
  /* The padlock (infraternal D177, packages/ui styles.css .ui-window-lock): centred at the top edge, the shackle above its body; a 44 px hit area, no chip. */
  .lock { position: absolute; z-index: 6; top: 2px; left: 50%; width: 44px; height: 44px; margin-left: -22px; display: grid; place-items: center; border-radius: var(--r-select); color: var(--ink); opacity: 0; pointer-events: none; transition: opacity var(--dur-fast) var(--ease-out), color var(--dur-fast) var(--ease-out); }
  .lock svg { width: var(--glyph); height: var(--glyph); fill: none; stroke: currentColor; stroke-width: var(--glyph-stroke); stroke-linecap: round; stroke-linejoin: round; overflow: visible; }
  .shackle { transform-origin: 8.5px 11px; transition: transform var(--dur-base) var(--ease-out); transform: translateY(-3px) rotate(-22deg); }
  :global(.win[data-locked]) .shackle { transform: none; }
  :global(.win[data-near]) .lock { opacity: 0.35; pointer-events: auto; }
  :global(.win[data-locked]) .lock { opacity: 0.9; color: var(--accent-base); pointer-events: auto; }
  .lock:focus-visible { opacity: 0.8; pointer-events: auto; }
  @media (hover: hover) and (pointer: fine) { :global(.win) .lock:hover { opacity: 0.8; } }
  /* Touch has no hover, and on a phone the title pill is at the top middle where the padlock hangs: the padlock shows there only while locked (the window menu has Lock input). */
  @media (pointer: coarse) { :global(.win[data-locked]) .lock { opacity: 0.9; } }
</style>

<script lang="ts">
  // The three regions on the seamless background (docs/ui-shell.md): the sidebar sits directly on it; the middle card runs top to
  // bottom of the viewport; the right card holds the app's panels. Wide screens put them side by side. Narrow screens (< 760 px) stack
  // them: the sidebar is underneath and the middle card slides aside to reveal it (and visibly floats above it); the right card slides
  // over the middle card from the right; the app and its panels are the bottom bar.
  import { Card, Pill } from "@calternal/ui";
  import { ariaKeys, shortcuts } from "../lib/shortcuts.svelte";
  import type { AppDef } from "../apps/registry";
  import { appearance } from "../appearance/prefs.svelte";
  import { route } from "../lib/route.svelte";
  import { surfaces } from "../appearance/surfaces.svelte";
  import AppSwitcher from "./AppSwitcher.svelte";
  import Backdrop from "./Backdrop.svelte";
  import PhoneTabs from "./PhoneTabs.svelte";
  import RightSidebar from "./RightSidebar.svelte";
  import ResizeHandle from "./ResizeHandle.svelte";
  import Sidebar from "./Sidebar.svelte";
  import { shell } from "./shell.svelte";
  let { app, id }: { app: AppDef; id?: string } = $props();
  // `?switcher=card|sidebar` picks where the app switcher lives (and is remembered), so both layouts can be compared live; Appearance has the same choice.
  $effect(() => {
    const q = route.query.get("switcher");
    if ((q === "card" || q === "sidebar") && appearance.prefs.switcher !== q) appearance.change((x) => (x.switcher = q));
  });
  // Phone gestures (docs/ui-motion.md "Panels over the card"). The middle card and the side panel follow the finger one to one; on release they settle by the sheet motion tokens
  // (`--dur-sheet` 340 ms on `--ease-sheet`), toward whichever rest state the position and the release speed point at. What a gesture decides goes through `shell.over`/`shell.dismiss`,
  // so it is a history entry: Back, Forward and the platform's own back gesture do the same thing. Touch only; horizontal intent (more across than down); a row swipe, a sheet drag or a
  // text selection is not a panel gesture. Interruptible: a touch while either is still settling takes hold of it where it is (its current transform) and carries on from there.
  type Mode = "card" | "panel";
  let mode: Mode | null = null, gx = 0, gy = 0, g0 = 0, gskip = false, gtouch = false, gmoved = false;
  let samples: { t: number; x: number }[] = [];
  /** The middle card's offset from rest, and the side panel's offset from open, while a finger holds them (px); null when nothing is held. */
  let drag = $state<number | null>(null);
  let dragMode = $state<Mode | null>(null);
  let midEl = $state<HTMLElement>(), rightEl = $state<HTMLElement>(), sideEl = $state<HTMLElement>();
  const matrixX = (el: Element | undefined) => (el ? new DOMMatrixReadOnly(getComputedStyle(el).transform).m41 : 0);
  /** How far the card travels to show the list, and how wide the side panel is. */
  const travel = () => sideEl?.firstElementChild?.getBoundingClientRect().width ?? innerWidth * 0.8;
  const panelWidth = () => rightEl?.getBoundingClientRect().width ?? innerWidth * 0.9;
  const settling = (el: Element | undefined) => !!el?.getAnimations().some((a) => (a as CSSTransition).transitionProperty === "transform" && a.playState === "running");
  function gdown(e: PointerEvent) {
    if (!shell.narrow || e.pointerType !== "touch") return;
    gtouch = true; gx = e.clientX; gy = e.clientY; gmoved = false; samples = [{ t: e.timeStamp, x: e.clientX }];
    gskip = !!(e.target as Element).closest(".slide, [role=dialog], input, textarea, pre");
    mode = null;
    // Taking hold of a card or panel that is still settling: freeze it where it is.
    if (settling(midEl)) { mode = "card"; g0 = matrixX(midEl); dragMode = "card"; drag = g0; return; }
    if (settling(rightEl)) { mode = "panel"; g0 = Math.min(matrixX(rightEl), panelWidth()); dragMode = "panel"; drag = g0; return; }
    if (gskip) return;
    const edgeL = e.clientX < 24, edgeR = e.clientX > innerWidth - 24;
    const target = e.target as Element;
    if (!shell.sidebarOpen && !rightOpen && edgeL) mode = "card", g0 = 0;
    else if (shell.sidebarOpen && (target.closest(".peek") || target.closest(".side"))) mode = "card", g0 = travel(); // the strip of card that stays visible, or the list itself
    else if (!rightOpen && !shell.sidebarOpen && edgeR && hasRight) mode = "panel", g0 = panelWidth();
    else if (rightOpen && (target.closest(".scrim") || target.closest(".right"))) mode = "panel", g0 = 0;
  }
  function gmove(e: PointerEvent) {
    if (!gtouch || !mode || e.pointerType !== "touch") return;
    const dx = e.clientX - gx, dy = e.clientY - gy;
    if (drag === null) {
      if (Math.abs(dx) < 10 || Math.abs(dx) < Math.abs(dy) * 1.5) return; // not yet a horizontal gesture
      dragMode = mode;
    }
    gmoved = true;
    samples.push({ t: e.timeStamp, x: e.clientX });
    if (samples.length > 6) samples.shift();
    const max = mode === "card" ? travel() : panelWidth();
    drag = Math.max(0, Math.min(g0 + dx, max));
  }
  function gup(e: PointerEvent) {
    const m = mode;
    let d = drag;
    gtouch = false; mode = null; drag = null; dragMode = null;
    if (m === null) return;
    if (d === null && e.pointerType === "touch") {
      // A swipe that arrived as a press and a release with no moves between (a quick flick the browser coalesced): judge it by where it ended.
      const dx = e.clientX - gx, dy = e.clientY - gy;
      if (Math.abs(dx) < 56 || Math.abs(dx) < Math.abs(dy) * 1.5) return;
      d = Math.max(0, Math.min(g0 + dx, m === "card" ? travel() : panelWidth()));
      samples.push({ t: e.timeStamp, x: e.clientX });
    }
    if (d === null) return;
    // Release speed over the last ~80 ms (px per ms, positive is rightward).
    const last = samples.at(-1)!, first = samples.find((p) => last.t - p.t <= 80) ?? last;
    const v = last.t > first.t ? (last.x - first.x) / (last.t - first.t) : 0;
    const max = m === "card" ? travel() : panelWidth();
    const FLICK = 0.4;
    if (m === "card") {
      const open = v > FLICK ? true : v < -FLICK ? false : d > max / 2;
      shell.over(open ? "list" : null);
    } else {
      const open = v > FLICK ? false : v < -FLICK ? true : d < max / 2;
      if (open) shell.over("panel"); else shell.dismiss();
    }
  }
  /** A tap on the strip of card that stays visible, or on the scrim, closes what is over the card; the end of a drag that started there is not a tap. */
  const tap = () => { if (gmoved) { gmoved = false; return; } shell.dismiss(); };
  // Choosing something in the sidebar shows it: the list goes away without a history step, so Back brings the list back over what you came from.
  const choose = (e: MouseEvent) => { if (shell.narrow && (e.target as Element).closest("a[href]")) shell.closeOver(); };
  const hasRight = $derived(app.panels.length > 0);
  const rightOpen = $derived(shell.rightOpen && hasRight);
  // A closed sidebar or side panel always has a way back: a button at the card's top corner on the same side, with the shortcut in its tooltip. (A phone has its own header button and the edge swipe.)
  // (A phone shows it for Settings only: Threads and Notes have their own header button, and the Settings page has no header.)
  const showLeft = $derived((!shell.narrow && !shell.sidebarOpen) || (shell.narrow && app.kind === "settings" && !shell.sidebarOpen));
  const showRight = $derived(!shell.narrow && hasRight && !rightOpen);
  const cardSwitcher = $derived(!shell.narrow && appearance.prefs.switcher === "card");
</script>

<Backdrop />
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="shell" class:narrow={shell.narrow} onpointerdown={gdown} onpointermove={gmove} onpointerup={gup} onpointercancel={gup} data-sidebar={shell.sidebarOpen} data-right={rightOpen}>
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
  <aside class="side" bind:this={sideEl} onclick={choose} data-ink={surfaces.left ?? undefined} aria-label="{app.name} sidebar" inert={!shell.sidebarOpen}><Sidebar {app} /></aside>

  <main class="mid" bind:this={midEl} style:--edge-l={showLeft && !shell.narrow ? "calc(var(--pill-h) + var(--s-3))" : undefined} class:dragging={drag !== null} style:transform={dragMode === "card" && drag !== null ? `translateX(${drag}px)` : undefined} style:scale={dragMode === "panel" && drag !== null ? 1 - 0.04 * (1 - drag / panelWidth()) : undefined} inert={shell.narrow && (shell.sidebarOpen || rightOpen)}>
    {#if !shell.narrow && shell.sidebarOpen}<ResizeHandle side="left" />{/if}
    {#if !shell.narrow && hasRight && rightOpen}<ResizeHandle side="right" />{/if}
    {#if showLeft}<div class="edge l"><Pill icon="sidebar" label="Show the sidebar" aria-keyshortcuts={ariaKeys(shortcuts.chordFor("sidebar"))} onclick={() => shell.toggleSidebar()} /></div>{/if}
    {#if showRight}<div class="edge r"><Pill icon="sidebar-right" label="Show the side panel" aria-keyshortcuts={ariaKeys(shortcuts.chordFor("panel"))} onclick={() => shell.toggleRight()} /></div>{/if}
    <Card label={app.name} class="mid-card">
      <div class="col">
        <div class="app"><app.Main {id} /></div>
        {#if cardSwitcher}<div class="appbar" class:auto={appearance.prefs.switcherAutoHide}><AppSwitcher {app} /></div>{/if}
      </div>
    </Card>
  </main>

  {#if shell.narrow && shell.sidebarOpen}<button type="button" class="peek" aria-label="Close the sidebar" onclick={tap}></button>{/if}
  {#if shell.narrow && (rightOpen || dragMode === "panel")}<button type="button" class="scrim" aria-label="Close the side panel" style:opacity={dragMode === "panel" && drag !== null ? 1 - drag / panelWidth() : undefined} style:animation={dragMode === "panel" && drag !== null ? "none" : undefined} onclick={tap}></button>{/if}

  {#if hasRight}<aside class="right" bind:this={rightEl} class:dragging={dragMode === "panel" && drag !== null} style:transform={dragMode === "panel" && drag !== null ? `translateX(${drag}px)` : undefined} data-ink={surfaces.right ?? undefined} aria-label="{app.name} side panel" inert={!rightOpen}><RightSidebar {app} {id} /></aside>{/if}
</div>
<!-- The phone's bottom bar is the app's panels: an app with none (Settings) has no bar, and its pages use the whole height. -->
{#if shell.narrow && !shell.sidebarOpen && app.panels.length}<PhoneTabs {app} />{/if}

<style>
  /* Wide chat (the plugin's default, Settings, Appearance, Chat layout): on a desktop the thread's column is the whole card, with room on the left for a contents rail and a margin on the right. */
  @media (min-width: 760px) { :global(html[data-chat-wide]) { --msg-w: 100%; --msg-inner: none; --msg-pad-l: 3.25rem; --msg-pad-r: 1.5rem; } }
  .shell {
    --sw: var(--side-w); --rw: var(--right-w);
    position: fixed; inset: 0; z-index: 1; display: grid; grid-template-columns: var(--sw) minmax(0, 1fr) var(--rw); grid-template-rows: minmax(0, 1fr); /* one row exactly the window's height: a page taller than the window scrolls inside the card and never stretches the sidebars (their footer stays in view in every app) */
  }
  .shell[data-sidebar="false"] { --sw: 0px; }
  .shell[data-right="false"] { --rw: 0px; }
  /* Each surface carries its own secondary inks, solved from the pixels under it (appearance/veil.ts). */
  .side { min-width: 0; overflow: hidden; --ink-2: var(--l-ink2, currentcolor); --ink-3: var(--l-ink3, currentcolor); }
  .right { --ink-2: var(--r-ink2, currentcolor); --ink-3: var(--r-ink3, currentcolor); }
  .mid { --ink-2: var(--c-ink2, currentcolor); --ink-3: var(--c-ink3, currentcolor); }
  .narrow .right { --ink-2: var(--l-ink2, currentcolor); --ink-3: var(--l-ink3, currentcolor); }
  /* Opening or closing a sidebar changes the columns at once (animating them would re-lay-out the whole thread every frame); the sidebar's content eases in
     and out with transform and opacity only, which stay on the compositor. */
  .side > :global(.side), .right > :global(.rs) { transition: opacity var(--dur-sheet) var(--ease-sheet), translate var(--dur-sheet) var(--ease-sheet); }
  .shell[data-sidebar="false"] .side > :global(.side) { opacity: 0; translate: calc(var(--s-8) * -1) 0; }
  .shell[data-right="false"] .right > :global(.rs) { opacity: 0; translate: var(--s-8) 0; }
  .side > :global(.side) { width: var(--side-w); }
  /* The middle card's top and bottom edges touch the viewport; it keeps one gap on each side. */
  .mid { min-width: 0; padding: 0 var(--inset); position: relative; }
  .edge { position: absolute; z-index: 6; top: calc(var(--safe-top) + (var(--bar-h) - var(--pill-h)) / 2); }
  .edge.l { left: calc(var(--inset) + var(--s-4)); }
  .edge.r { right: calc(var(--inset) + var(--s-4)); }
  .mid :global(.mid-card) { height: 100%; overflow: clip; }
  .col { position: relative; height: 100%; display: flex; flex-direction: column; }
  .app { flex: 1; min-height: 0; position: relative; }
  .appbar { flex: none; padding: var(--s-3) var(--s-6) var(--s-4); }
  /* Hidden until needed: the bar takes no room; a thin hot zone at the card's bottom edge (and focus inside) brings it in, on the form's ease. */
  .appbar.auto { position: absolute; z-index: 3; left: 0; right: 0; bottom: 0; height: var(--s-5); padding: 0; }
  .appbar.auto > :global(.switcher) { position: absolute; left: var(--s-6); right: var(--s-6); bottom: var(--s-3); opacity: 0; translate: 0 var(--s-4); pointer-events: none; transition: opacity var(--dur-base) var(--ease-out), translate var(--dur-base) var(--ease-out); }
  .appbar.auto:hover > :global(.switcher), .appbar.auto:focus-within > :global(.switcher) { opacity: 1; translate: 0 0; pointer-events: auto; }
  .right { min-width: 0; overflow: hidden; }
  .right > :global(.rs) { width: var(--right-w); }

  /* Narrow: layers. */
  .shell.narrow { --peek: calc(var(--s-10) + var(--s-4)); display: block; --bottom-inset: calc(var(--tier-cap) + var(--cap-offset) + var(--s-4)); }
  .narrow .side { position: fixed; inset: 0; width: 100%; z-index: 1; padding-top: var(--safe-top); visibility: visible; }
  .narrow .side > :global(.side) { width: calc(100% - var(--peek)); }
  .narrow[data-sidebar="false"] .side { visibility: hidden; transition: visibility 0s var(--dur-sheet); }
  .narrow:has(.mid.dragging) .side { visibility: visible; transition: none; }
  .narrow .mid { touch-action: pan-y; }
  .narrow .mid.dragging { transition: none; }
  .narrow:has(.mid.dragging) .mid :global(.mid-card) { border-radius: var(--r-pane); }
  .narrow .mid { position: fixed; inset: 0; z-index: 2; padding: 0; transition: transform var(--dur-sheet) var(--ease-sheet), scale var(--dur-sheet) var(--ease-sheet); transform-origin: left center; }
  .narrow[data-sidebar="true"] .mid { transform: translateX(calc(100% - var(--peek))); }
  .narrow[data-right="true"] .mid { scale: 0.96; }
  .narrow .mid :global(.mid-card) { border-radius: 0; transition: border-radius var(--dur-sheet) var(--ease-sheet); }
  .narrow[data-sidebar="true"] .mid :global(.mid-card), .narrow[data-right="true"] .mid :global(.mid-card) { border-radius: var(--r-pane); }
  .peek { position: fixed; z-index: 3; top: 0; bottom: 0; right: 0; width: var(--peek); touch-action: pan-y; }
  .scrim { position: fixed; inset: 0; z-index: 3; background: var(--scrim); animation: fade var(--dur-base) var(--ease-out); touch-action: pan-y; }
  .narrow .right.dragging { transition: none; }
  .narrow .right { touch-action: pan-y; }
  /* Slid over the app, the flat sidebar needs a ground of its own: the same wallpaper copy and wash, aligned to the viewport. */
  .narrow .right { position: fixed; z-index: 4; top: 0; bottom: 0; right: 0; width: calc(100% - var(--s-9)); transform: translateX(110%); transition: transform var(--dur-sheet) var(--ease-sheet); border-radius: var(--r-pane) 0 0 var(--r-pane); box-shadow: var(--shadow-card); background: var(--veil-l, linear-gradient(rgb(var(--tint) / 0.5) 0 0)), var(--wall-side, var(--backdrop)) center / cover no-repeat fixed; }
  .narrow[data-right="true"] .right { transform: none; }
  .narrow .right > :global(.rs) { width: 100%; }
  @keyframes fade { from { opacity: 0; } }
</style>

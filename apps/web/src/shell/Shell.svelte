<script lang="ts">
  // The three regions on the seamless background (docs/ui-shell.md): the sidebar sits directly on it; the middle card runs top to
  // bottom of the viewport; the right card holds the app's panels. Wide screens put them side by side. Narrow screens (< 760 px) stack
  // them: the sidebar is underneath and the middle card slides aside to reveal it (and visibly floats above it); the right card slides
  // over the middle card from the right; the app and its panels are the bottom bar.
  import { untrack } from "svelte";
  import { Card, Pill } from "@calternal/ui";
  import { ariaKeys, shortcuts } from "../lib/shortcuts.svelte";
  import type { AppDef } from "../apps/registry";
  import { appearance } from "../appearance/prefs.svelte";
  import { route } from "../lib/route.svelte";
  import { surfaces } from "../appearance/surfaces.svelte";
  import AppSwitcher from "./AppSwitcher.svelte";
  import Backdrop from "./Backdrop.svelte";
  import Canvas from "../canvas/Canvas.svelte";
  import { canvasEnabled } from "../canvas/model.svelte";
  import RightSidebar from "./RightSidebar.svelte";
  import ResizeHandle from "./ResizeHandle.svelte";
  import Sidebar from "./Sidebar.svelte";
  import { shell } from "./shell.svelte";
  import { leavesAlone, parseMs, pickTarget, settleOpen, visible, type Side, type Target } from "./pull";
  let { app, id }: { app: AppDef; id?: string } = $props();
  // The sidebar pull (docs/ui-shell.md "Sidebar pull"; the rules are in ./pull.ts): a horizontal swipe anywhere over the middle card brings a closed sidebar into view, following the finger one to
  // one, and the opposite swipe closes it. One recogniser for every width. On a phone the card slides aside (the sidebar is underneath; the side panel slides over); on a wide screen the sidebar
  // slides in over the card's edge and the card moves with it; a trackpad does not pull (a horizontal wheel never moves a sidebar). On release the result goes through `shell.over` (a
  // history entry on a phone: Back, Forward and the platform's own back gesture do the same thing) or `shell.setOpen` (a wide screen, remembered). It settles on the sheet motion tokens
  // (`--dur-sheet` on `--ease-sheet`) toward the rest state that the position and the release speed point at (a flick of 0.4 px/ms decides, else half way). Interruptible: a touch or swipe while
  // anything still settles takes hold of it where it is. The edge swipe of the first phone version is the same gesture started at the edge.
  type Mode = "card" | "panel";
  let mode: Mode | null = null, tgt: Target | null = null, gx = 0, gy = 0, g0 = 0, gskip = false, gtouch = false, gmoved = false, gtarget: Element | null = null;
  let samples: { t: number; x: number }[] = [];
  /** A phone: the middle card's offset from rest, or the side panel's offset from open, while a finger holds them (px); null when nothing is held. */
  let drag = $state<number | null>(null);
  let dragMode = $state<Mode | null>(null);
  /** A wide screen: the sidebar being pulled, how much of it shows, and how wide it is. `layoutOpen` is whether its column is open in the layout while the pull lasts (it is not changed until release). */
  let wp = $state<{ side: Side; vis: number; w: number; layoutOpen: boolean; settling: boolean } | null>(null);
  let midEl = $state<HTMLElement>(), rightEl = $state<HTMLElement>(), sideEl = $state<HTMLElement>();
  const matrixX = (el: Element | undefined) => (el ? new DOMMatrixReadOnly(getComputedStyle(el).transform).m41 : 0);
  /** How far the card travels to show the list, and how wide the side panel is. */
  const travel = () => sideEl?.firstElementChild?.getBoundingClientRect().width ?? innerWidth * 0.8;
  const panelWidth = () => rightEl?.getBoundingClientRect().width ?? innerWidth * 0.9;
  const sideWidth = (side: Side) => (side === "left" ? sideEl : rightEl)?.firstElementChild?.getBoundingClientRect().width ?? 0;
  const settling = (el: Element | undefined) => !!el?.getAnimations().some((a) => (a as CSSTransition).transitionProperty === "transform" && a.playState === "running");
  const openness = () => ({ leftOpen: shell.sidebarOpen, rightOpen, hasRight });
  const moving = (el: Element | undefined) => !!el?.getAnimations().some((a) => a.playState === "running" && (a as Animation & { id: string }).id === "pull");
  /** The sheet motion tokens as an animation's timing. */
  const sheet = () => { const cs = getComputedStyle(document.documentElement); return { duration: parseMs(cs.getPropertyValue("--dur-sheet"), 340), easing: cs.getPropertyValue("--ease-sheet").trim() || "ease-out", id: "pull" }; };
  /** Where a swipe's first move decides what it is: the target by direction, then the geometry that goes with it. False when the swipe is not a pull. */
  function begin(dx: number, dy: number): boolean {
    if (Math.abs(dx) < 10 || Math.abs(dx) < Math.abs(dy) * 1.5) return false; // not yet a horizontal gesture
    const dir = dx > 0 ? 1 : -1;
    if (leavesAlone(gtarget, dir, false)) { gskip = true; return false; }
    const t = pickTarget(dir, openness());
    if (!t) { gskip = true; return false; }
    tgt = t;
    if (shell.narrow) {
      mode = t.side === "left" ? "card" : "panel";
      g0 = t.side === "left" ? (t.opening ? 0 : travel()) : (t.opening ? panelWidth() : 0);
      dragMode = mode;
    } else {
      const w = sideWidth(t.side);
      wp = { side: t.side, vis: t.opening ? 0 : w, w, layoutOpen: t.side === "left" ? shell.sidebarOpen : rightOpen, settling: false };
      wpFrom = wp.vis;
    }
    return true;
  }
  /** Takes hold of a wide pull that is still settling: where the card is now says how much of the sidebar shows. */
  function grabWide(side: Side): number {
    const layoutOpen = side === "left" ? shell.sidebarOpen : rightOpen;
    const w = sideWidth(side);
    const off = matrixX(midEl);
    midEl?.getAnimations().forEach((a) => a.cancel());
    (side === "left" ? sideEl : rightEl)?.firstElementChild?.getAnimations().forEach((a) => a.cancel());
    const vis = Math.max(0, Math.min(w, side === "left" ? off + (layoutOpen ? w : 0) : -off + (layoutOpen ? w : 0)));
    wp = { side, vis, w, layoutOpen, settling: false };
    return vis;
  }
  function gdown(e: PointerEvent) {
    if (e.pointerType !== "touch") return;
    gtouch = true; gx = e.clientX; gy = e.clientY; gmoved = false; samples = [{ t: e.timeStamp, x: e.clientX }];
    gtarget = e.target as Element;
    gskip = leavesAlone(gtarget, 0, !!getSelection()?.toString());
    mode = null; tgt = null;
    if (!shell.narrow) {
      // Taking hold of a wide pull that is still settling: the card is where it is, so carry on from there.
      if (moving(midEl) && !gskip) { const side: Side = (lastSide ?? "left"); tgt = { side, opening: false }; wpFrom = grabWide(side); }
      return;
    }
    // Taking hold of a card or panel that is still settling: freeze it where it is.
    if (settling(midEl)) { mode = "card"; g0 = matrixX(midEl); dragMode = "card"; drag = g0; tgt = { side: "left", opening: g0 < travel() / 2 }; return; }
    if (settling(rightEl)) { mode = "panel"; g0 = Math.min(matrixX(rightEl), panelWidth()); dragMode = "panel"; drag = g0; tgt = { side: "right", opening: g0 > panelWidth() / 2 }; return; }
  }
  let lastSide: Side | null = null, wpFrom = 0;
  function gmove(e: PointerEvent) {
    if (!gtouch || gskip || e.pointerType !== "touch") return;
    const dx = e.clientX - gx, dy = e.clientY - gy;
    if (!tgt && !begin(dx, dy)) return;
    gmoved = true;
    samples.push({ t: e.timeStamp, x: e.clientX });
    if (samples.length > 6) samples.shift();
    if (wp) { wp.vis = visible(tgt!, wpFrom, dx, wp.w); return; }
    if (mode) drag = Math.max(0, Math.min(g0 + dx, mode === "card" ? travel() : panelWidth()));
  }
  function gup(e: PointerEvent) {
    // A swipe that arrived as a press and a release with no moves between (a quick flick the browser coalesced): judge it by where it ended.
    if (gtouch && !gskip && !tgt && e.pointerType === "touch") { const dx = e.clientX - gx, dy = e.clientY - gy; if (Math.abs(dx) >= 56 && Math.abs(dx) >= Math.abs(dy) * 1.5 && begin(dx, dy)) samples.push({ t: e.timeStamp, x: e.clientX }); }
    gtouch = false;
    const t = tgt, m = mode;
    tgt = null; mode = null;
    if (!t) return;
    // Release speed over the last ~80 ms (px per ms, positive is rightward).
    const last = samples.at(-1)!, first = samples.find((p) => last.t - p.t <= 80) ?? last;
    const v = last.t > first.t ? (last.x - first.x) / (last.t - first.t) : 0;
    const dx = e.clientX - gx;
    if (wp) { finishWide(t.side === "left" ? v : -v); return; }
    if (!m) return;
    const max = m === "card" ? travel() : panelWidth();
    const d = drag ?? Math.max(0, Math.min(g0 + dx, max));
    drag = null; dragMode = null;
    if (m === "card") shell.over(settleOpen(d, max, v) ? "list" : null);
    else shell.over(settleOpen(max - d, max, -v) ? "panel" : null);
  }
  /** Ends a wide pull: the sidebar's state changes at once (the layout), and the card and the sidebar's content ease from where the finger left them to their rest, on the sheet tokens. */
  function finishWide(vAlong: number) {
    const p = wp!;
    const open = settleOpen(p.vis, p.w, vAlong);
    lastSide = p.side;
    if (open !== p.layoutOpen) shell.setOpen(p.side, open);
    settleWide(p.side, p.w, p.vis, open);
  }
  /**
   * The one animation of a sidebar going to `open` (its layout is already in the new state) from `from` px showing: the card and the sidebar's content ease from where they are to rest on the sheet tokens, `transform` and
   * `translate` only. A pull ends in it, and so do the button, the shortcut and the menu: opening and closing are the same motion in either direction, whatever started it. While it runs the sidebar stays drawn
   * (fixed at its edge) so a closing one slides away instead of vanishing; a new gesture takes hold of it where it is.
   */
  function settleWide(side: Side, w: number, from: number, open: boolean) {
    // The card's offset from where its layout now puts it, so the screen position is unchanged at this instant.
    const cardFrom = side === "left" ? from - (open ? w : 0) : -(from - (open ? w : 0));
    const timing = sheet();
    lastSide = side;
    requestAnimationFrame(() => {
      midEl?.animate([{ transform: `translateX(${cardFrom}px)` }, { transform: "translateX(0px)" }], timing);
      const inner = (side === "left" ? sideEl : rightEl)?.firstElementChild as HTMLElement | undefined;
      const sideFrom = side === "left" ? from - w : w - from;
      const sideTo = open ? 0 : side === "left" ? -w : w;
      const a = inner?.animate([{ translate: `${sideFrom}px 0`, opacity: 1 }, { translate: `${sideTo}px 0`, opacity: 1 }], timing);
      if (open || !a) wp = null;
      else a.onfinish = a.oncancel = () => { if (wp?.settling) wp = null; };
    });
    // Until the next frame the pieces stay where they were, now laid out for the new state (the screen does not move at this instant).
    wp = { side, vis: from, w, layoutOpen: open, settling: true };
  }
  // A sidebar opened or closed by anything but a pull (a button, a shortcut, a menu, the palette) goes through the same motion, from fully closed to fully open or back, or from wherever a running one had got to.
  let seenLeft = shell.sidebarOpen, seenRight = false, seenInit = false;
  $effect(() => {
    const l = shell.sidebarOpen, r = rightOpen;
    untrack(() => {
      if (!seenInit) { seenInit = true; seenLeft = l; seenRight = r; return; }
      const changedSide: Side | null = l !== seenLeft ? "left" : r !== seenRight ? "right" : null;
      seenLeft = l; seenRight = r;
      if (!changedSide || shell.narrow || wp) return; // a phone has its own CSS motion; a pull already runs this
      const open = changedSide === "left" ? l : r;
      const w = sideWidth(changedSide);
      if (!w) return;
      settleWide(changedSide, w, moving(midEl) ? grabWide(changedSide) : open ? 0 : w, open);
    });
  });
  // A trackpad's horizontal swipe does nothing here (owner decision, 2026-11-18): on a desktop the sidebars have buttons, shortcuts and resize handles. Only touch pulls (one finger).
  /** A tap on the strip of card that stays visible, or on the scrim, closes what is over the card; the end of a drag that started there is not a tap. */
  const tap = () => { if (gmoved) { gmoved = false; return; } shell.dismiss(); };
  // Choosing something in the sidebar shows it: the list goes away without a history step, so Back brings the list back over what you came from.
  const choose = (e: MouseEvent) => { if (shell.narrow && (e.target as Element).closest("a[href]")) shell.closeOver(); };
  // The panels belong to what is open: with no thread (or note) open there is nothing to switch between, so no side panel and, on a phone, no bottom bar. The way back to the list stays (the header button, the edge swipe).
  const hasRight = $derived(app.panels.length > 0 && (!shell.narrow || !!id));
  const rightOpen = $derived(shell.rightOpen && hasRight);
  // A closed sidebar or side panel always has a way back: a button at the card's top corner on the same side, with the shortcut in its tooltip. (A phone has its own header button and the edge swipe.)
  // (A phone shows it for Settings only: Threads and Notes have their own header button, and the Settings page has no header.)
  const showLeft = $derived((!shell.narrow && !shell.sidebarOpen) || (shell.narrow && (app.kind === "settings" || !id) && !shell.sidebarOpen)); // a phone card with no header of its own (Settings, or an app with nothing open) still has a way to the list
  // The side panel's button at the card's corner: on a wide screen when it is closed; on a phone (where the bottom bar is the apps) whenever there is something open to show panels for and nothing else is over the card.
  const showRight = $derived(hasRight && !rightOpen && (!shell.narrow || (!!id && !shell.sidebarOpen)));
</script>

<Backdrop />
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="shell" class:narrow={shell.narrow} style:--pull-x={shell.narrow ? (dragMode === "card" && drag !== null ? `translateX(${drag}px)` : shell.sidebarOpen ? "translateX(calc(100% - var(--peek)))" : undefined) : undefined} data-pull={wp?.side} onpointerdown={gdown} onpointermove={gmove} onpointerup={gup} onpointercancel={gup} data-sidebar={shell.sidebarOpen} data-right={rightOpen}>
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
  <aside class="side" class:pulled={wp?.side === "left"} style:--vis={wp?.side === "left" ? `${wp.vis}px` : undefined} style:--w={wp?.side === "left" ? `${wp.w}px` : undefined} bind:this={sideEl} onclick={choose} data-ink={surfaces.left ?? undefined} aria-label="{app.name} sidebar" inert={!shell.sidebarOpen}><Sidebar {app} /></aside>

  <main class="mid" bind:this={midEl} style:--edge-l={showLeft && !shell.narrow ? "calc(var(--pill-h) + var(--s-3))" : undefined} class:dragging={drag !== null} style:transform={wp ? `translateX(${wp.side === "left" ? wp.vis - (wp.layoutOpen ? wp.w : 0) : -(wp.vis - (wp.layoutOpen ? wp.w : 0))}px)` : dragMode === "card" && drag !== null ? `translateX(${drag}px)` : undefined} style:scale={dragMode === "panel" && drag !== null ? 1 - 0.04 * (1 - drag / panelWidth()) : undefined} inert={shell.narrow && (shell.sidebarOpen || rightOpen)}>
    {#if !shell.narrow && shell.sidebarOpen}<ResizeHandle side="left" />{/if}
    {#if !shell.narrow && hasRight && rightOpen}<ResizeHandle side="right" />{/if}
    {#if showLeft}<div class="edge l"><Pill icon="sidebar" label="Show the sidebar" aria-keyshortcuts={ariaKeys(shortcuts.chordFor("sidebar"))} onclick={() => shell.toggleSidebar()} /></div>{/if}
    {#if showRight}<div class="edge r"><Pill icon="sidebar-right" label="Show the side panel" aria-keyshortcuts={ariaKeys(shortcuts.chordFor("panel"))} onclick={() => shell.toggleRight()} /></div>{/if}
    <Card label={app.name} class="mid-card">
      <div class="col" class:cv={canvasEnabled}>
        <div class="app">{#if canvasEnabled}<Canvas />{:else}<app.Main {id} />{/if}</div>
        {#if !shell.narrow}<div class="appbar"><AppSwitcher {app} placement="card" /></div>{/if}
      </div>
    </Card>
  </main>

  {#if shell.narrow && shell.sidebarOpen}<button type="button" class="peek" aria-label="Close the sidebar" onclick={tap}></button>{/if}
  {#if shell.narrow && (rightOpen || dragMode === "panel")}<button type="button" class="scrim" aria-label="Close the side panel" style:opacity={dragMode === "panel" && drag !== null ? 1 - drag / panelWidth() : undefined} style:animation={dragMode === "panel" && drag !== null ? "none" : undefined} onclick={tap}></button>{/if}

  {#if hasRight}<aside class="right" class:pulled={wp?.side === "right"} style:--vis={wp?.side === "right" ? `${wp.vis}px` : undefined} style:--w={wp?.side === "right" ? `${wp.w}px` : undefined} bind:this={rightEl} class:dragging={dragMode === "panel" && drag !== null} style:transform={dragMode === "panel" && drag !== null ? `translateX(${drag}px)` : undefined} data-ink={surfaces.right ?? undefined} aria-label="{app.name} side panel" inert={!rightOpen}><RightSidebar {app} {id} /></aside>{/if}
  <!-- The phone's bottom bar is the app switcher (the same component as under the card on a wide screen); it rides with the card, so it slides away with it. The panels are the side panel's (the button at the card's corner, or a swipe). -->
  {#if shell.narrow}<AppSwitcher {app} placement="bar" />{/if}
</div>
<style>
  /* Wide chat (the plugin's default, Settings, Appearance, Chat layout): on a desktop the thread's column is the whole card, with room on the left for a contents rail and a margin on the right. */
  @media (min-width: 760px) { :global(html[data-chat-wide]) { --msg-w: 100%; --msg-inner: none; --msg-pad-l: 3.25rem; --msg-pad-r: 1.5rem; } }
  .shell {
    --sw: var(--side-w); --rw: var(--right-w);
    position: fixed; inset: 0 0 var(--kb, 0px) 0; box-sizing: border-box; padding-left: var(--safe-left); padding-right: var(--safe-right); z-index: 1; display: grid; grid-template-columns: var(--sw) minmax(0, 1fr) var(--rw); grid-template-rows: minmax(0, 1fr); /* one row exactly the window's height: a page taller than the window scrolls inside the card and never stretches the sidebars (their footer stays in view in every app) */
  }
  .shell:not(.narrow) > .side { grid-column: 1; } .shell:not(.narrow) > .mid { grid-column: 2; } .shell:not(.narrow) > .right { grid-column: 3; }
  /* A sidebar being pulled into view (or out): out of the columns, fixed at its edge at full width, its content `--vis` px into view; the card is moved by the same amount (inline transform). The rules are in ./pull.ts. */
  .shell:not(.narrow) > .side.pulled { position: fixed; z-index: 1; left: 0; top: 0; bottom: 0; width: var(--w); }
  .shell:not(.narrow) > .right.pulled { position: fixed; z-index: 1; right: 0; top: 0; bottom: 0; width: var(--w); }
  .side.pulled > :global(.side) { opacity: 1 !important; translate: calc(var(--vis) - var(--w)) 0 !important; transition: none !important; }
  .right.pulled > :global(.rs) { opacity: 1 !important; translate: calc(var(--w) - var(--vis)) 0 !important; transition: none !important; }
  .shell:not(.narrow) .mid { touch-action: pan-y; }
  /* An iPhone home-screen app is laid out shorter than its screen (shell/chrome.ts): the shell is as tall as the probe measured. The keyboard lifts it (`--kb`). */
  :global(html[data-home-screen]) .shell { bottom: auto; height: calc(var(--vh, 100%) - var(--kb, 0px)); }
  .shell[data-sidebar="false"] { --sw: 0px; }
  .shell[data-right="false"] { --rw: 0px; }
  /* Each surface carries its own secondary inks, solved from the pixels under it (appearance/veil.ts). */
  .side { min-width: 0; overflow: clip; --ink-2: var(--l-ink2, currentcolor); --ink-3: var(--l-ink3, currentcolor); }
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
  /* The card is the only thing on the background, and it casts nothing onto it: a shadow would be a second layer in the gap beside the card (the owner's seam). It is told apart by its own surface (docs/ui-design-system.md). On a phone it slides over the list, where it does cast one. */
  .shell:not(.narrow) .mid :global(.mid-card) { box-shadow: none; }
  .col { position: relative; height: 100%; display: flex; flex-direction: column; }
  .app { flex: 1; min-height: 0; position: relative; }
  .appbar { flex: none; padding: var(--s-3) var(--s-6) var(--s-4); }
  /* The canvas experiment: the canvas runs to the card's bottom edge and the capsule floats over it (no band behind it); the windows stop above it (`--canvas-bottom`). */
  .col.cv { --canvas-bottom: calc(var(--s-3) + var(--pill-h) + var(--s-4) + var(--s-5)); }
  .col.cv > .appbar { position: absolute; z-index: 4; left: 0; right: 0; bottom: 0; pointer-events: none; }
  .col.cv > .appbar > :global(*) { pointer-events: auto; }
  .right { min-width: 0; overflow: hidden; }
  .right > :global(.rs) { width: var(--right-w); }

  /* Narrow: layers. */
  .shell.narrow { --peek: calc(var(--s-10) + var(--s-4)); display: block; --bottom-inset: calc(var(--tier-cap) + var(--cap-offset) + var(--s-4)); }
  .narrow .side { position: fixed; inset: 0; width: 100%; z-index: 1; padding-top: var(--safe-top); visibility: visible; }
  .narrow .side > :global(.side) { width: calc(100% - var(--peek)); }
  .narrow[data-sidebar="false"] .side { visibility: hidden; transition: visibility 0s var(--dur-sheet); }
  .narrow:has(.mid.dragging) .side { visibility: visible; transition: none; }
  /* The sidebar and the side panel are mounted and drawn at every pull position: closed, the sidebar is only out of sight (`visibility`, after the card has slid away), never faded or moved, so a half pull shows
     the list, fully painted, with the card sliding over it (its corners rounded and its shadow on). Nothing is laid out during the pull: only the card's (or panel's) `transform` changes. */
  .narrow .side > :global(.side), .narrow .right > :global(.rs) { opacity: 1 !important; translate: none !important; transition: none !important; }
  .narrow[data-sidebar="true"] .mid :global(.mid-card), .narrow[data-right="true"] .mid :global(.mid-card), .narrow:has(.mid.dragging) .mid :global(.mid-card) { box-shadow: var(--shadow-card); }
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

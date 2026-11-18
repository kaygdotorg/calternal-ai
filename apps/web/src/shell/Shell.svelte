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
  import { leavesAlone, pickTarget, settleOpen, visible, WheelSwipe, type Side, type Target } from "./pull";
  let { app, id }: { app: AppDef; id?: string } = $props();
  // `?switcher=card|sidebar` picks where the app switcher lives (and is remembered), so both layouts can be compared live; Appearance has the same choice.
  $effect(() => {
    const q = route.query.get("switcher");
    if ((q === "card" || q === "sidebar") && appearance.prefs.switcher !== q) appearance.change((x) => (x.switcher = q));
  });
  // The sidebar pull (docs/ui-motion.md "Sidebar pull"; the rules are in ./pull.ts): a horizontal swipe anywhere over the middle card brings a closed sidebar into view, following the finger one to
  // one, and the opposite swipe closes it. One recogniser for every width. On a phone the card slides aside (the sidebar is underneath; the side panel slides over); on a wide screen the sidebar
  // slides in over the card's edge and the card moves with it, and a trackpad's horizontal two-finger swipe does the same through `wheel` events. On release the result goes through `shell.over` (a
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
  const state = () => ({ leftOpen: shell.sidebarOpen, rightOpen, hasRight });
  const moving = (el: Element | undefined) => !!el?.getAnimations().some((a) => a.playState === "running" && (a as Animation & { id: string }).id === "pull");
  /** The sheet motion tokens as an animation's timing. */
  const sheet = () => { const cs = getComputedStyle(document.documentElement); const ms = parseFloat(cs.getPropertyValue("--dur-sheet")); return { duration: Number.isFinite(ms) ? ms : 340, easing: cs.getPropertyValue("--ease-sheet").trim() || "ease-out", id: "pull" }; };
  /** Where a swipe's first move decides what it is: the target by direction, then the geometry that goes with it. False when the swipe is not a pull. */
  function begin(dx: number, dy: number): boolean {
    if (Math.abs(dx) < 10 || Math.abs(dx) < Math.abs(dy) * 1.5) return false; // not yet a horizontal gesture
    const dir = dx > 0 ? 1 : -1;
    if (leavesAlone(gtarget, dir, false)) { gskip = true; return false; }
    const t = pickTarget(dir, state());
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
    const from = p.vis;
    lastSide = p.side;
    const changed = open !== p.layoutOpen;
    if (changed) shell.setOpen(p.side, open);
    const newOpen = open;
    // The card's offset from where its layout now puts it, so the screen position is unchanged at this instant.
    const cardFrom = p.side === "left" ? from - (newOpen ? p.w : 0) : -(from - (newOpen ? p.w : 0));
    const timing = sheet();
    requestAnimationFrame(() => {
      midEl?.animate([{ transform: `translateX(${cardFrom}px)` }, { transform: "translateX(0px)" }], timing);
      const inner = (p.side === "left" ? sideEl : rightEl)?.firstElementChild as HTMLElement | undefined;
      const sideFrom = p.side === "left" ? from - p.w : p.w - from;
      const sideTo = newOpen ? 0 : p.side === "left" ? -p.w : p.w;
      const a = inner?.animate([{ translate: `${sideFrom}px 0`, opacity: 1 }, { translate: `${sideTo}px 0`, opacity: 1 }], timing);
      if (newOpen || !a) wp = null;
      else a.onfinish = a.oncancel = () => { if (wp?.settling) wp = null; };
    });
    // Until the next frame the pieces stay where the finger left them, now laid out for the new state (the screen does not move at this instant).
    wp = { ...p, vis: from, layoutOpen: open, settling: true };
  }
  // A trackpad's horizontal two-finger swipe over the app: the same pull, fed by `wheel` events (./pull.ts WheelSwipe). Only on a wide screen; the events are consumed (so the browser does not take
  // the swipe as Back) only while a pull owns them.
  const ws = new WheelSwipe();
  let wheelTimer = 0, wheelFrom = 0;
  function wheel(e: WheelEvent) {
    if (shell.narrow || e.ctrlKey) return;
    const r = ws.feed(e, (dir) => {
      const t = pickTarget(dir, state());
      if (!t || leavesAlone(e.target as Element, dir, false)) return false;
      tgt = t;
      const w = sideWidth(t.side);
      if (wp && moving(midEl)) wheelFrom = grabWide(t.side); else { wp = { side: t.side, vis: t.opening ? 0 : w, w, layoutOpen: t.side === "left" ? shell.sidebarOpen : rightOpen, settling: false }; wheelFrom = wp.vis; }
      return true;
    });
    if (r === "leave") return;
    e.preventDefault();
    if (wp) wp.vis = visible(tgt!, wheelFrom, ws.dx, wp.w);
    clearTimeout(wheelTimer);
    wheelTimer = window.setTimeout(() => { const side = wp?.side; const v = ws.velocity(); ws.end(); tgt = null; if (wp && side) finishWide(side === "left" ? v : -v); }, WheelSwipe.IDLE + 30);
  }
  /** Adds the wheel listener non-passively (a passive one cannot cancel the swipe's default). */
  const wheelAction = (node: HTMLElement) => { node.addEventListener("wheel", wheel, { passive: false }); return { destroy: () => node.removeEventListener("wheel", wheel) }; };
  /** A tap on the strip of card that stays visible, or on the scrim, closes what is over the card; the end of a drag that started there is not a tap. */
  const tap = () => { if (gmoved) { gmoved = false; return; } shell.dismiss(); };
  // Choosing something in the sidebar shows it: the list goes away without a history step, so Back brings the list back over what you came from.
  const choose = (e: MouseEvent) => { if (shell.narrow && (e.target as Element).closest("a[href]")) shell.closeOver(); };
  const hasRight = $derived(app.panels.length > 0);
  const rightOpen = $derived(shell.rightOpen && hasRight);
  // A closed sidebar or side panel always has a way back: a button at the card's top corner on the same side, with the shortcut in its tooltip. (A phone has its own header button and the edge swipe.)
  // (A phone shows it for Settings only: Threads and Notes have their own header button, and the Settings page has no header.)
  const showLeft = $derived((!shell.narrow && !shell.sidebarOpen) || (shell.narrow && (app.kind === "settings" || !id) && !shell.sidebarOpen)); // a phone card with no header of its own (Settings, or an app with nothing open) still has a way to the list
  const showRight = $derived(!shell.narrow && hasRight && !rightOpen);
  const cardSwitcher = $derived(!shell.narrow && appearance.prefs.switcher === "card");
</script>

<Backdrop />
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="shell" class:narrow={shell.narrow} use:wheelAction data-pull={wp?.side} onpointerdown={gdown} onpointermove={gmove} onpointerup={gup} onpointercancel={gup} data-sidebar={shell.sidebarOpen} data-right={rightOpen}>
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
  <aside class="side" class:pulled={wp?.side === "left"} style:--vis={wp?.side === "left" ? `${wp.vis}px` : undefined} style:--w={wp?.side === "left" ? `${wp.w}px` : undefined} bind:this={sideEl} onclick={choose} data-ink={surfaces.left ?? undefined} aria-label="{app.name} sidebar" inert={!shell.sidebarOpen}><Sidebar {app} /></aside>

  <main class="mid" bind:this={midEl} style:--edge-l={showLeft && !shell.narrow ? "calc(var(--pill-h) + var(--s-3))" : undefined} class:dragging={drag !== null} style:transform={wp ? `translateX(${wp.side === "left" ? wp.vis - (wp.layoutOpen ? wp.w : 0) : -(wp.vis - (wp.layoutOpen ? wp.w : 0))}px)` : dragMode === "card" && drag !== null ? `translateX(${drag}px)` : undefined} style:scale={dragMode === "panel" && drag !== null ? 1 - 0.04 * (1 - drag / panelWidth()) : undefined} inert={shell.narrow && (shell.sidebarOpen || rightOpen)}>
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

  {#if hasRight}<aside class="right" class:pulled={wp?.side === "right"} style:--vis={wp?.side === "right" ? `${wp.vis}px` : undefined} style:--w={wp?.side === "right" ? `${wp.w}px` : undefined} bind:this={rightEl} class:dragging={dragMode === "panel" && drag !== null} style:transform={dragMode === "panel" && drag !== null ? `translateX(${drag}px)` : undefined} data-ink={surfaces.right ?? undefined} aria-label="{app.name} side panel" inert={!rightOpen}><RightSidebar {app} {id} /></aside>{/if}
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
  .shell:not(.narrow) > .side { grid-column: 1; } .shell:not(.narrow) > .mid { grid-column: 2; } .shell:not(.narrow) > .right { grid-column: 3; }
  /* A sidebar being pulled into view (or out): out of the columns, fixed at its edge at full width, its content `--vis` px into view; the card is moved by the same amount (inline transform). The rules are in ./pull.ts. */
  .shell:not(.narrow) > .side.pulled { position: fixed; z-index: 1; left: 0; top: 0; bottom: 0; width: var(--w); }
  .shell:not(.narrow) > .right.pulled { position: fixed; z-index: 1; right: 0; top: 0; bottom: 0; width: var(--w); }
  .side.pulled > :global(.side) { opacity: 1 !important; translate: calc(var(--vis) - var(--w)) 0 !important; transition: none !important; }
  .right.pulled > :global(.rs) { opacity: 1 !important; translate: calc(var(--w) - var(--vis)) 0 !important; transition: none !important; }
  .shell:not(.narrow) .mid { touch-action: pan-y; }
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

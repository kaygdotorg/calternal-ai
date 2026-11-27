<script lang="ts">
  import { Toasts } from "@calternal/ui";
  import { arr } from "./arrange.svelte";
  // The mockup's shell: the owner's sidebar on the wallpaper (left), the canvas of windows on the floating card (right). The sidebar is the app (or the sidebar of no app) chosen in the card; the canvas holds what is open. Opening a collection
  // or an item changes the canvas and never the sidebar. On a phone the sidebar is the first screen and opening something pushes the canvas over it, with Back. The desktop can be drawn inside a Mac window (a tweak): traffic lights top left,
  // the sidebar button beside them, the wordmark below. Everything is fixture data; nothing is stored or sent except the person's arrangement (the apps' order, the sidebars' sections).
  import { onMount } from "svelte";
  import { IconButton } from "@calternal/ui";
  import Backdrop from "./vendor/Backdrop.svelte";
  import Sidebar from "./shell/Sidebar.svelte";
  import AppStrip from "./shell/AppStrip.svelte";
  import OpenMenu from "./shell/OpenMenu.svelte";
  import { watchNewWindow } from "./lib/intent";
  import Canvas from "./canvas/Canvas.svelte";
  import RightPanel from "./shell/RightPanel.svelte";
  import { Grip } from "@calternal/ui";
  import { layout, WIDTHS } from "./layout.svelte";
  import Palette from "./shell/Palette.svelte";
  import Tweaks from "./shell/Tweaks.svelte";
  import DragGhost from "./shell/DragGhost.svelte";
  import { forced, tweaks, panel } from "./tweaks.svelte";
  import { wins } from "./windows.svelte";
  import { APPS, SETTINGS } from "./data/apps";
  import { ITEMS, type Item } from "./data/collections";

  /** The sidebar shown: an app's id, a custom sidebar's id, or null (nothing chosen: the wordmark and "Recent"). */
  let app = $state<string | null>(forced.app);
  if (forced.coll) wins.navigate("collection", forced.coll); else if (forced.app) wins.navigate("app", forced.app);
  const second = new URLSearchParams(location.search).get("second"); if (second && ITEMS[second]) wins.openItem(ITEMS[second]!);
  let search = $state(forced.sheet === "search"), width = $state(innerWidth), pushed = $state(new URLSearchParams(location.search).get("pushed") === "1"), sidebarOpen = $state(true);
  const narrow = $derived(forced.phone || width < 720);
  /** The right panel is drawn only on a desktop, when the person has it open and something is open to describe. */
  /** The left sidebar is hidden (desktop only): the shell's first column closes and the sidebar slides out; the toggle moves to the first window's top bar so it stays reachable. The Mac frame keeps its own toggle. */
  const hidden = $derived(!narrow && (mac ? !sidebarOpen : !layout.leftOpen));
  // THE SWITCHER'S AUTO-HIDE (a setting): the strip is shown while the pointer is in the bottom band of the viewport or over the strip, while focus is inside it, or while the keyboard asked for it (Alt+Shift+A); it hides 260 ms after the pointer leaves, so a slip does not make it flicker. A phone has no hover: it never hides.
  let nearBottom = $state(false), stripFocus = $state(false), kbd = $state(false), hideTimer = 0;
  const dockShown = $derived(!tweaks.t.switcherHide || nearBottom || stripFocus || kbd);
  function pointer(e: PointerEvent) {
    if (narrow || e.pointerType === "touch" || !tweaks.t.switcherHide) return;
    const band = innerHeight - e.clientY <= (dockShown ? 96 : 28);
    if (band) { clearTimeout(hideTimer); nearBottom = true; } else if (nearBottom) { clearTimeout(hideTimer); hideTimer = window.setTimeout(() => (nearBottom = false), 260); }
  }
  const rightShown = $derived(!narrow && layout.rightOpen && wins.list.length > 0);
  const mac = $derived(tweaks.t.mac && !narrow);
  /** The last thing chosen in the card: an app (the top line names its sidebar) or a collection (smart: it names that collection, until an app is chosen). */
  let lastPick = $state<"app" | "collection">(forced.coll ? "collection" : "app");
  const focusedColl = $derived(wins.focused?.kind === "collection" && !wins.focused.ref.startsWith("sb:") ? arr.coll(wins.focused.ref)?.name : undefined);
  const title = $derived(tweaks.t.topLine === "smart" && lastPick === "collection" && focusedColl ? focusedColl : app ? ([...APPS, SETTINGS].find((x) => x.id === app)?.name ?? arr.custom.find((c) => c.id === app)?.name ?? "calternal") : "calternal");
  const current = $derived(app);
  onMount(() => {
    const r = () => (width = innerWidth); addEventListener("resize", r);
    const k = (e: KeyboardEvent) => { if (e.metaKey && e.ctrlKey && e.code === "KeyS") { e.preventDefault(); if (mac) sidebarOpen = !sidebarOpen; else layout.leftOpen = !layout.leftOpen; } else if (e.altKey && e.shiftKey && e.code === "KeyA") { e.preventDefault(); kbd = true; setTimeout(() => document.querySelector<HTMLElement>("[data-switcher] .it")?.focus(), 30); } else if ((e.metaKey || e.ctrlKey) && e.key === "k") { e.preventDefault(); search = true; } else if (e.altKey && e.shiftKey && e.code === "KeyT") { e.preventDefault(); panel.open = !panel.open; } }; addEventListener("keydown", k); const unwatch = watchNewWindow(pointAt, () => !narrow);
    return () => { removeEventListener("resize", r); removeEventListener("keydown", k); unwatch(); };
  });
  const openItem = (i: Item) => { if (i.opens) { wins.navigate("collection", i.opens); pushed = true; return; } wins.openItem(i); pushed = true; };
  const openColl = (cid: string) => { wins.navigate("collection", cid); lastPick = "collection"; pushed = true; };
  /** The strip's choice. Choosing the open app again chooses nothing: the top line morphs back to the wordmark. A new window is asked for by its own gestures (lib/intent.ts). */
  function onselect(id: string | null) {
    { app = id; lastPick = "app"; if (id) wins.navigate("app", id); else if (wins.focused?.kind === "app") wins.close(wins.focused.id); }
  }
  /** The sidebar follows an app opened in a new window. */
  const pointAt = (id: string) => { app = id; lastPick = "app"; };
  const home = () => { app = null; lastPick = "app"; wins.clear(); };
  const settings = () => { lastPick = "app"; app = "settings"; wins.navigate("app", "settings"); };
  const onback = () => (pushed = false);
</script>
<Toasts bottom={24} />


{#snippet shell()}
  <div class="shell" class:narrow class:mac class:closed={hidden} data-shell data-pushed={pushed} style:--sw={hidden ? undefined : `${layout.left}px`} style:--rw={rightShown ? `${layout.right}px` : '0px'}>
    <aside class="side" inert={hidden}><Sidebar {title} home={title === "calternal"} onhome={home} {app} onopen={openItem} onaction={() => { if (narrow) pushed = true; }} onapp={() => { if (app) { wins.navigate("app", app); pushed = true; } }} oncollection={openColl} selected={wins.selected} onsearch={() => (search = true)} onsettings={settings} {narrow} {mac} /></aside>
    {#if !narrow && !mac && !hidden}<Grip label="Resize the sidebar" min={WIDTHS.left.min} max={WIDTHS.left.max} now={layout.left} style="left: calc(var(--sw) - var(--s-7) / 2 - 2px)" onmove={(e) => layout.setWidth("left", e.clientX - 4)} onkey={(e) => { const d = e.shiftKey ? 64 : 16; let w = layout.left; if (e.key === "ArrowRight") w += d; else if (e.key === "ArrowLeft") w -= d; else if (e.key === "Home") w = WIDTHS.left.min; else if (e.key === "End") w = WIDTHS.left.max; else if (e.key === "Enter") { layout.setWidth("left", null); e.preventDefault(); return; } else return; e.preventDefault(); layout.setWidth("left", w); }} onreset={() => layout.setWidth("left", null)} />{/if}
    <main class="mid" data-dock={dockShown ? "show" : "hide"}>
      <Canvas {narrow} onopen={openItem} onapp={() => { if (app) { wins.navigate("app", app); pushed = true; } }} oncollection={openColl} {onback} lights={mac && !sidebarOpen} sidebarHidden={hidden && !mac} onsidebar={() => (layout.leftOpen = true)} />
      {#if !narrow}<AppStrip {current} {onselect} shown={dockShown} onfocusin={() => (stripFocus = true)} onfocusout={() => { stripFocus = false; kbd = false; }} />{/if}
    </main>
    {#if narrow && !pushed}<AppStrip {current} {onselect} narrow />{/if}
    {#if rightShown}
      <Grip label="Resize the side panel" min={WIDTHS.right.min} max={WIDTHS.right.max} now={layout.right} style="right: calc(var(--rw) - var(--s-7) / 2 - 2px)" onmove={(e) => layout.setWidth("right", innerWidth - e.clientX - 4)} onkey={(e) => { const d = e.shiftKey ? 64 : 16; let w = layout.right; if (e.key === "ArrowLeft") w += d; else if (e.key === "ArrowRight") w -= d; else if (e.key === "Home") w = WIDTHS.right.min; else if (e.key === "End") w = WIDTHS.right.max; else if (e.key === "Enter") { layout.setWidth("right", null); e.preventDefault(); return; } else return; e.preventDefault(); layout.setWidth("right", w); }} onreset={() => layout.setWidth("right", null)} />
      <aside class="right"><RightPanel win={wins.focused} onclose={() => (layout.rightOpen = false)} /></aside>
    {/if}
  </div>
{/snippet}

{#if mac}
  <div class="desk"></div>
  <div class="win" data-mac-window>
    <Backdrop />
    {@render shell()}
    <div class="lights" aria-hidden="true"><i style:background="#ff5f57"></i><i style:background="#febc2e"></i><i style:background="#28c840"></i></div>
    <div class="tb"><IconButton icon="sidebar" label={sidebarOpen ? "Hide the sidebar" : "Show the sidebar"} pressed={sidebarOpen} onclick={() => (sidebarOpen = !sidebarOpen)} /></div>
  </div>
{:else}
  <Backdrop />
  {@render shell()}
{/if}
<svelte:window onpointermove={pointer} />
<Palette open={search} onclose={() => (search = false)} />
<Tweaks />
<OpenMenu {narrow} onapp={pointAt} />
<DragGhost />

<style>
  .shell { position: fixed; inset: 0; display: grid; grid-template-columns: var(--sw, 19rem) minmax(0, 1fr) var(--rw, 0px); gap: var(--s-3); padding: var(--s-3) 0 var(--s-3) 0; box-sizing: border-box; transition: grid-template-columns var(--dur-slow) var(--ease-out); }
  .shell.mac { padding-left: 0; } .shell.closed { grid-template-columns: 0 minmax(0, 1fr) var(--rw, 0px); padding-left: var(--s-3); } .right { min-width: 0; min-height: 0; overflow: hidden; } .shell.closed .side { opacity: 0; translate: -2rem 0; }
  /* the secondary inks each surface's veil was solved for (the design system's one readability floor: ink 7:1, ink-2 5.5:1, ink-3 4.5:1 on the worst pixels under it): the sidebar and the card and the right panel each use their own */
  .side { --ink-2: var(--l-ink2, currentcolor); --ink-3: var(--l-ink3, currentcolor); } .mid { --ink-2: var(--c-ink2, currentcolor); --ink-3: var(--c-ink3, currentcolor); } .right { --ink-2: var(--r-ink2, currentcolor); --ink-3: var(--r-ink3, currentcolor); }
  .side { min-width: 0; min-height: 0; overflow: hidden; transition: opacity var(--dur-base) var(--ease-out), translate var(--dur-slow) var(--ease-out); } .mid { position: relative; min-width: 0; min-height: 0; display: block; }
  /* The middle area is the BARE CANVAS on the shared tinted background (no outer card); each window is its own card. Under it, at the bottom centre, the app switcher; the windows end `--dock-space` above the bottom while it is shown (its own height plus a gap above and the padding below) and run to the bottom when it is hidden. The windows lead the reveal and trail the hide (AppStrip.svelte states the order). */
  .mid { --dock-space: calc(var(--tb-h) + var(--s-5) + var(--s-5) - var(--s-3)); box-sizing: border-box; }
  .shell:not(.narrow) .mid { margin-block: calc(var(--s-3) * -1); }
  .shell:not(.narrow) .mid { padding-bottom: var(--dock-space); transition: padding-bottom var(--dur-morph) var(--ease-morph); }
  .shell:not(.narrow) .mid[data-dock="hide"] { padding-bottom: 0; transition-delay: 90ms; }
  /* The sidebar drag handles are the shared Grip as it comes: invisible until the pointer is in its 20 pt strip at the sidebar's edge, while dragging, or on keyboard focus (fading over --dur-base on --ease-out); hidden on a coarse pointer. */
  .narrow { display: block; padding: 0; } .narrow .side, .narrow .mid { position: absolute; inset: 0; transition: translate var(--dur-slow) var(--ease-out); } .narrow .mid { padding: var(--s-3); box-sizing: border-box; translate: 100% 0; } .narrow[data-pushed="true"] .mid { translate: 0 0; } .narrow[data-pushed="true"] .side { translate: -20% 0; opacity: 0; visibility: hidden; transition: translate var(--dur-slow) var(--ease-out), opacity var(--dur-slow) var(--ease-out), visibility 0s linear var(--dur-slow); }
  /* the Mac window variant: a desk, a window with its own backdrop (a transformed box is the containing block of the fixed layers inside it) */
  .desk { position: fixed; inset: 0; z-index: 0; background: linear-gradient(135deg, #7f8fb0, #3b4766 60%, #272d42); }
  .win { position: fixed; inset: 28px; z-index: 1; border-radius: var(--r-menu); overflow: hidden; transform: translateZ(0); box-shadow: 0 0 0 1px rgb(0 0 0 / 0.35), 0 0 0 2px rgb(255 255 255 / 0.12), 0 28px 70px rgb(0 0 0 / 0.5); }
  .lights { position: absolute; top: 14px; left: 14px; z-index: 5; display: flex; gap: 8px; } .lights i { width: 12px; height: 12px; border-radius: 50%; box-shadow: inset 0 0 0 0.5px rgb(0 0 0 / 0.25); }
  .tb { position: absolute; top: 4px; left: 74px; z-index: 5; }
</style>

<script lang="ts">
  // The three regions on the seamless background (docs/ui-shell.md): the sidebar sits directly on it; the middle card runs top to
  // bottom of the viewport; the right card holds the app's panels. Wide screens put them side by side. Narrow screens (< 760 px) stack
  // them: the sidebar is underneath and the middle card slides aside to reveal it (and visibly floats above it); the right card slides
  // over the middle card from the right; the app and its panels are the bottom bar.
  import { Card } from "@calternal/ui";
  import type { AppDef } from "../apps/registry";
  import { appearance } from "../appearance/prefs.svelte";
  import { route } from "../lib/route.svelte";
  import AppSwitcher from "./AppSwitcher.svelte";
  import Backdrop from "./Backdrop.svelte";
  import PhoneTabs from "./PhoneTabs.svelte";
  import RightSidebar from "./RightSidebar.svelte";
  import Sidebar from "./Sidebar.svelte";
  import { shell } from "./shell.svelte";
  let { app, id }: { app: AppDef; id?: string } = $props();
  // `?switcher=card|sidebar` picks where the app switcher lives (and is remembered), so both layouts can be compared live; Appearance has the same choice.
  $effect(() => {
    const q = route.query.get("switcher");
    if ((q === "card" || q === "sidebar") && appearance.prefs.switcher !== q) appearance.change((x) => (x.switcher = q));
  });
  const hasRight = $derived(app.panels.length > 0);
  const rightOpen = $derived(shell.rightOpen && hasRight);
  const cardSwitcher = $derived(!shell.narrow && appearance.prefs.switcher === "card");
</script>

<Backdrop />
<div class="shell" class:narrow={shell.narrow} data-sidebar={shell.sidebarOpen} data-right={rightOpen}>
  <aside class="side" aria-label="{app.name} sidebar" inert={!shell.sidebarOpen}><Sidebar {app} /></aside>

  <main class="mid" inert={shell.narrow && (shell.sidebarOpen || rightOpen)}>
    <Card label={app.name} class="mid-card">
      <div class="col">
        <div class="app"><app.Main {id} /></div>
        {#if cardSwitcher}<div class="appbar"><AppSwitcher {app} /></div>{/if}
      </div>
    </Card>
  </main>

  {#if shell.narrow && shell.sidebarOpen}<button type="button" class="peek" aria-label="Close the sidebar" onclick={() => shell.closeOver()}></button>{/if}
  {#if shell.narrow && rightOpen}<button type="button" class="scrim" aria-label="Close the side panel" onclick={() => shell.closeOver()}></button>{/if}

  {#if hasRight}<aside class="right" aria-label="{app.name} side panel" inert={!rightOpen}><RightSidebar {app} {id} /></aside>{/if}
</div>
{#if shell.narrow && !shell.sidebarOpen}<PhoneTabs {app} />{/if}

<style>
  .shell {
    --sw: var(--side-w); --rw: var(--right-w);
    position: fixed; inset: 0; z-index: 1; display: grid; grid-template-columns: var(--sw) minmax(0, 1fr) var(--rw);
  }
  .shell[data-sidebar="false"] { --sw: 0px; }
  .shell[data-right="false"] { --rw: 0px; }
  .side { min-width: 0; overflow: hidden; }
  /* Opening or closing a sidebar changes the columns at once (animating them would re-lay-out the whole thread every frame); the sidebar's content eases in
     and out with transform and opacity only, which stay on the compositor. */
  .side > :global(.side), .right > :global(.rs) { transition: opacity var(--dur-sheet) var(--ease-sheet), translate var(--dur-sheet) var(--ease-sheet); }
  .shell[data-sidebar="false"] .side > :global(.side) { opacity: 0; translate: calc(var(--s-8) * -1) 0; }
  .shell[data-right="false"] .right > :global(.rs) { opacity: 0; translate: var(--s-8) 0; }
  .side > :global(.side) { width: var(--side-w); }
  /* The middle card's top and bottom edges touch the viewport; it keeps one gap on each side. */
  .mid { min-width: 0; padding: 0 var(--inset); }
  .mid :global(.mid-card) { height: 100%; overflow: clip; }
  .col { height: 100%; display: flex; flex-direction: column; }
  .app { flex: 1; min-height: 0; position: relative; }
  .appbar { flex: none; padding: var(--s-3) var(--s-6) var(--s-4); }
  .right { min-width: 0; overflow: hidden; }
  .right > :global(.rs) { width: var(--right-w); }

  /* Narrow: layers. */
  .shell.narrow { --peek: calc(var(--s-10) + var(--s-4)); display: block; --bottom-inset: calc(var(--tier-cap) + var(--cap-offset) + var(--s-4) + var(--safe-bottom)); }
  .narrow .side { position: fixed; inset: 0; width: 100%; z-index: 1; padding-top: var(--safe-top); visibility: visible; }
  .narrow .side > :global(.side) { width: calc(100% - var(--peek)); }
  .narrow[data-sidebar="false"] .side { visibility: hidden; transition: visibility 0s var(--dur-sheet); }
  .narrow .mid { position: fixed; inset: 0; z-index: 2; padding: 0; transition: transform var(--dur-sheet) var(--ease-sheet), scale var(--dur-sheet) var(--ease-sheet); transform-origin: left center; }
  .narrow[data-sidebar="true"] .mid { transform: translateX(calc(100% - var(--peek))); }
  .narrow[data-right="true"] .mid { scale: 0.96; }
  .narrow .mid :global(.mid-card) { border-radius: 0; transition: border-radius var(--dur-sheet) var(--ease-sheet); }
  .narrow[data-sidebar="true"] .mid :global(.mid-card), .narrow[data-right="true"] .mid :global(.mid-card) { border-radius: var(--r-pane); }
  .peek { position: fixed; z-index: 3; top: 0; bottom: 0; right: 0; width: var(--peek); }
  .scrim { position: fixed; inset: 0; z-index: 3; background: var(--scrim); animation: fade var(--dur-base) var(--ease-out); }
  /* Slid over the app, the flat sidebar needs a ground of its own: the same wallpaper copy and wash, aligned to the viewport. */
  .narrow .right { position: fixed; z-index: 4; top: 0; bottom: 0; right: 0; width: calc(100% - var(--s-9)); transform: translateX(110%); transition: transform var(--dur-sheet) var(--ease-sheet); border-radius: var(--r-pane) 0 0 var(--r-pane); box-shadow: var(--shadow-card); background: linear-gradient(rgb(var(--tint) / var(--wash, var(--side-tint-a))) 0 0), var(--wall-side, var(--backdrop)) center / cover no-repeat fixed; }
  .narrow[data-right="true"] .right { transform: none; }
  .narrow .right > :global(.rs) { width: 100%; }
  @keyframes fade { from { opacity: 0; } }
</style>

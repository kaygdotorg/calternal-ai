<script lang="ts">
  // The three regions on the seamless background (docs/ui-shell.md): the sidebar sits directly on it; the middle card runs top to
  // bottom of the viewport; the right card holds the app's panels. Wide screens put them side by side. Narrow screens (< 760 px) stack
  // them: the sidebar is underneath and the middle card slides aside to reveal it (and visibly floats above it); the right card slides
  // over the middle card from the right; the app and its panels are the bottom bar.
  import { Card } from "@calternal/ui";
  import type { AppDef } from "../apps/registry";
  import Backdrop from "./Backdrop.svelte";
  import PhoneTabs from "./PhoneTabs.svelte";
  import RightCard from "./RightCard.svelte";
  import Sidebar from "./Sidebar.svelte";
  import { shell } from "./shell.svelte";
  let { app, id }: { app: AppDef; id?: string } = $props();
</script>

<Backdrop />
<div class="shell" class:narrow={shell.narrow} data-sidebar={shell.sidebarOpen} data-right={shell.rightOpen}>
  <aside class="side" aria-label="{app.name} sidebar" inert={!shell.sidebarOpen}><Sidebar {app} /></aside>

  <main class="mid" inert={shell.narrow && (shell.sidebarOpen || shell.rightOpen)}>
    <Card label={app.name} class="mid-card"><app.Main {id} /></Card>
  </main>

  {#if shell.narrow && shell.sidebarOpen}<button type="button" class="peek" aria-label="Close the sidebar" onclick={() => shell.closeOver()}></button>{/if}
  {#if shell.narrow && shell.rightOpen}<button type="button" class="scrim" aria-label="Close the side card" onclick={() => shell.closeOver()}></button>{/if}

  <aside class="right" aria-label="{app.name} side card" inert={!shell.rightOpen}><RightCard {app} {id} /></aside>
</div>
{#if shell.narrow && !shell.sidebarOpen}<PhoneTabs {app} />{/if}

<style>
  .shell {
    --sw: var(--side-w); --rw: var(--right-w);
    position: fixed; inset: 0; z-index: 1; display: grid; grid-template-columns: var(--sw) minmax(0, 1fr) var(--rw);
    transition: grid-template-columns var(--spring-sheet-ms) var(--spring-sheet);
  }
  .shell[data-sidebar="false"] { --sw: 0px; }
  .shell[data-right="false"] { --rw: 0px; }
  .side { min-width: 0; overflow: hidden; }
  .side > :global(.side) { width: var(--side-w); }
  /* The middle card's top and bottom edges touch the viewport; it keeps one gap on each side. */
  .mid { min-width: 0; padding: 0 var(--card-gap) 0 0; }
  .shell:not(.narrow)[data-sidebar="false"] .mid { padding-left: var(--card-gap); }
  .mid :global(.mid-card) { height: 100%; overflow: clip; }
  .right { min-width: 0; overflow: hidden; padding-right: var(--card-gap); }
  .right > :global(.right) { height: 100%; width: calc(var(--right-w) - var(--card-gap)); overflow: clip; }

  /* Narrow: layers. */
  .shell.narrow { --peek: calc(var(--s-10) + var(--s-4)); display: block; --bottom-inset: calc(max(var(--pill-h), var(--hit-min)) + 2 * var(--s-4) + var(--s-5) + var(--safe-bottom)); }
  .narrow .side { position: fixed; inset: 0; width: 100%; z-index: 1; padding-top: var(--safe-top); visibility: visible; }
  .narrow .side > :global(.side) { width: calc(100% - var(--peek)); }
  .narrow[data-sidebar="false"] .side { visibility: hidden; transition: visibility 0s var(--spring-sheet-ms); }
  .narrow .mid { position: fixed; inset: 0; z-index: 2; padding: 0; transition: transform var(--spring-sheet-ms) var(--spring-sheet), scale var(--spring-sheet-ms) var(--spring-sheet); transform-origin: left center; }
  .narrow[data-sidebar="true"] .mid { transform: translateX(calc(100% - var(--peek))); }
  .narrow[data-right="true"] .mid { scale: 0.96; }
  .narrow .mid :global(.mid-card) { border-radius: 0; transition: border-radius var(--spring-sheet-ms) var(--spring-sheet); }
  .narrow[data-sidebar="true"] .mid :global(.mid-card), .narrow[data-right="true"] .mid :global(.mid-card) { border-radius: var(--r-frame); }
  .peek { position: fixed; z-index: 3; top: 0; bottom: 0; right: 0; width: var(--peek); }
  .scrim { position: fixed; inset: 0; z-index: 3; background: var(--scrim); animation: fade var(--dur-base) var(--ease-out); }
  .narrow .right { position: fixed; z-index: 4; top: 0; bottom: 0; right: 0; width: calc(100% - var(--s-9)); padding: 0 var(--card-gap) 0 0; transform: translateX(110%); transition: transform var(--spring-sheet-ms) var(--spring-sheet); overflow: visible; }
  .narrow[data-right="true"] .right { transform: none; }
  .narrow .right > :global(.right) { width: 100%; }
  @keyframes fade { from { opacity: 0; } }
</style>

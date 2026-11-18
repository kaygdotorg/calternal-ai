<script lang="ts">
  // The right region: a flat sidebar directly on the wallpaper, like the left one (only the middle is a card). The current app's panels scroll under a
  // floating title and floating bottom tabs, fading out at both edges (no opaque band). The app's plugin decides which panels exist.
  import { Pill, Selector } from "@calternal/ui";
  import { ariaKeys, shortcuts } from "../lib/shortcuts.svelte";
  import type { AppDef } from "../apps/registry";
  import { shell } from "./shell.svelte";
  let { app, id }: { app: AppDef; id?: string } = $props();
  const panel = $derived(app.panels.find((p) => p.id === shell.panel) ?? app.panels[0]!);
</script>

<div class="rs">
  <header>
    <!-- The button leads (the left sidebar's trails), so each sits on its own outer edge. -->
    {#if shell.narrow}<Pill icon="close" label="Close the side panel" onclick={() => shell.dismiss()} />{:else}<Pill icon="sidebar-right" label="Hide the side panel" aria-keyshortcuts={ariaKeys(shortcuts.chordFor("panel"))} onclick={() => shell.toggleRight()} />{/if}
    <h2>{panel.label}</h2>
  </header>
  <div class="content edge-fade" role="tabpanel" id="panel" aria-label={panel.label}>
    {#key panel.id}<panel.component {id} {...panel.props} />{/key}
  </div>
  {#if !shell.narrow}
    <div class="tabs"><Selector label="{app.name} panels" options={app.panels.map((p) => ({ value: p.id, label: p.tab ?? p.label, iconName: p.icon }))} value={panel.id} onchange={(v) => shell.showPanel(v)} /></div>
  {/if}
</div>

<style>
  .rs { --foot-h: calc(var(--pill-h) + 2 * var(--s-2) + var(--s-4) + var(--foot-inset)); position: relative; height: 100%; min-height: 0; }
  header { position: absolute; z-index: 2; top: var(--safe-top); left: 0; right: 0; height: var(--bar-h); display: flex; align-items: center; justify-content: flex-start; gap: var(--s-4); padding: 0 var(--s-4) 0 var(--s-4); }
  h2 { font-size: var(--text-heading); font-weight: var(--strong-weight); }
  .content { position: absolute; inset: 0; overflow-y: auto; overscroll-behavior: contain; scrollbar-width: none; padding: calc(var(--safe-top) + var(--bar-h) + var(--s-4)) var(--s-4) var(--foot-h); --fade-t: calc(var(--safe-top) + var(--bar-h)); --fade-b: var(--foot-h); }
  .tabs { position: absolute; z-index: 2; left: 0; right: 0; bottom: 0; display: flex; justify-content: center; padding: 0 var(--s-4) var(--foot-inset); }
  .tabs :global(.selector) { width: 100%; }
  /* Four tabs, each with its icon and its label: a tab keeps the width of its content and the capsule shares what is left. */
  .tabs :global(.cap .seg) { flex: 1 1 auto; min-width: 0; padding-inline: var(--s-3); } /* shrinks with the sidebar (its labels end in an ellipsis): the capsule never runs out of the sidebar */
</style>

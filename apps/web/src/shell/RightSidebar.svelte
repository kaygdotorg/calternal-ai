<script lang="ts">
  // The right region: a flat sidebar directly on the wallpaper, like the left one (only the middle is a card). The current app's panels scroll under a
  // floating title and floating bottom tabs, fading out at both edges (no opaque band). The app's plugin decides which panels exist.
  import { Capsule, Pill } from "@calternal/ui";
  import type { AppDef } from "../apps/registry";
  import { shell } from "./shell.svelte";
  let { app, id }: { app: AppDef; id?: string } = $props();
  const panel = $derived(app.panels.find((p) => p.id === shell.panel) ?? app.panels[0]!);
</script>

<div class="rs">
  <header>
    <h2>{panel.label}</h2>
    {#if shell.narrow}<Pill icon="close" label="Close the side panel" onclick={() => shell.closeOver()} />{/if}
  </header>
  <div class="content edge-fade" role="tabpanel" id="panel" aria-label={panel.label}>
    {#key panel.id}<panel.component {id} {...panel.props} />{/key}
  </div>
  {#if !shell.narrow}
    <div class="tabs"><Capsule label="{app.name} panels" items={app.panels.map((p) => ({ id: p.id, label: p.tab ?? p.label, icon: p.icon }))} value={panel.id} onchange={(v) => shell.showPanel(v)} /></div>
  {/if}
</div>

<style>
  .rs { --foot-h: calc(var(--pill-h) + 2 * var(--s-2) + var(--s-4) + max(var(--s-4), var(--safe-bottom))); position: relative; height: 100%; min-height: 0; }
  header { position: absolute; z-index: 2; top: 0; left: 0; right: 0; height: var(--bar-h); display: flex; align-items: center; justify-content: space-between; padding: 0 var(--s-6); }
  h2 { font-size: var(--text-heading); font-weight: var(--strong-weight); }
  .content { position: absolute; inset: 0; overflow-y: auto; overscroll-behavior: contain; scrollbar-width: none; padding: calc(var(--bar-h) + var(--s-4)) var(--s-4) var(--foot-h); --fade-t: var(--bar-h); --fade-b: var(--foot-h); }
  .tabs { position: absolute; z-index: 2; left: 0; right: 0; bottom: 0; display: flex; justify-content: center; padding: 0 var(--s-4) max(var(--s-4), var(--safe-bottom)); }
  .tabs :global(.cap) { width: 100%; }
  /* Four tabs, each with its icon and its label: a tab keeps the width of its content and the capsule shares what is left. */
  .tabs :global(.cap .seg) { flex: 1 0 auto; padding-inline: var(--s-4); }
</style>

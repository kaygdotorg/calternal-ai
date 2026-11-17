<script lang="ts">
  // The right region: a flat sidebar directly on the wallpaper, like the left one (only the middle is a card). The current app's panels, with
  // the tabs at the bottom of the sidebar on a desktop (on a phone they are the bottom bar). The app's plugin decides which panels exist.
  import { Capsule, IconButton } from "@calternal/ui";
  import type { AppDef } from "../apps/registry";
  import { shell } from "./shell.svelte";
  let { app, id }: { app: AppDef; id?: string } = $props();
  const panel = $derived(app.panels.find((p) => p.id === shell.panel) ?? app.panels[0]!);
</script>

<div class="rs">
  <header>
    <h2>{panel.label}</h2>
    {#if shell.narrow}<IconButton icon="close" label="Close the side panel" onclick={() => shell.closeOver()} />{/if}
  </header>
  <div class="content" role="tabpanel" id="panel" aria-label={panel.label}>
    {#key panel.id}<panel.component {id} {...panel.props} />{/key}
  </div>
  {#if !shell.narrow}
    <div class="tabs"><Capsule label="{app.name} panels" morph={app.panels.length > 3} items={app.panels.map((p) => ({ id: p.id, label: p.label, icon: p.icon }))} value={panel.id} onchange={(v) => shell.showPanel(v)} /></div>
  {/if}
</div>

<style>
  .rs { height: 100%; display: flex; flex-direction: column; min-height: 0; padding: 0 var(--s-4); }
  header { flex: none; display: flex; align-items: center; justify-content: space-between; min-height: var(--bar-h); padding: 0 var(--s-3); }
  h2 { font-size: var(--text-heading); font-weight: var(--strong-weight); }
  .content { flex: 1; min-height: 0; overflow-y: auto; }
  .tabs { flex: none; display: flex; justify-content: center; padding: var(--s-4) 0 max(var(--s-4), var(--safe-bottom)); }
  .tabs :global(.cap) { width: 100%; }
</style>

<script lang="ts">
  // The right card: the current app's panels, with the tabs at the bottom of the card on a desktop (on a phone they are the bottom bar).
  import { Capsule, Card, IconButton } from "@calternal/ui";
  import type { AppDef } from "../apps/registry";
  import { shell } from "./shell.svelte";
  let { app, id }: { app: AppDef; id?: string } = $props();
  const panel = $derived(app.panels.find((p) => p.id === shell.panel) ?? app.panels[0]!);
</script>

<Card label="{app.name} side panel" class="right">
  <div class="in">
    <header>
      <h2>{panel.label}</h2>
      {#if shell.narrow}<IconButton icon="close" label="Close the side card" onclick={() => shell.closeOver()} />{/if}
    </header>
    <div class="content" role="tabpanel" id="panel" aria-label={panel.label}>
      {#key panel.id}<panel.component {id} {...panel.props} />{/key}
    </div>
    {#if !shell.narrow}
      <div class="tabs"><Capsule label="{app.name} panels" items={app.panels.map((p) => ({ id: p.id, label: p.label, icon: p.icon }))} value={panel.id} onchange={(v) => shell.showPanel(v)} /></div>
    {/if}
  </div>
</Card>

<style>
  .in { position: absolute; inset: 0; display: flex; flex-direction: column; border-radius: inherit; overflow: hidden; }
  header { flex: none; display: flex; align-items: center; justify-content: space-between; padding: var(--s-4) var(--s-5) var(--s-2) var(--s-6); min-height: calc(max(var(--pill-h), var(--hit-min)) + 2 * var(--s-4)); }
  h2 { font-size: var(--text-heading); font-weight: var(--strong-weight); }
  .content { flex: 1; min-height: 0; overflow-y: auto; padding: 0 var(--s-4) var(--s-4); }
  .tabs { flex: none; display: flex; justify-content: center; padding: var(--s-4) var(--s-5) var(--s-5); }
  .tabs :global(.cap) { width: 100%; }
</style>

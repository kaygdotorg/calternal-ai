<script lang="ts">
  // The phone's bottom bar: the app and its panels as one capsule. The selected tab widens to show its name and the others shrink to
  // their icon (the mockup's tab morph). Choosing a panel slides the right card over the app; choosing the app slides it away.
  import { Capsule } from "@calternal/ui";
  import type { AppDef } from "../apps/registry";
  import { shell } from "./shell.svelte";
  let { app }: { app: AppDef } = $props();
  const items = $derived([{ id: "main", label: app.name, icon: app.icon }, ...app.panels.map((p) => ({ id: p.id, label: p.label, icon: p.icon }))]);
  const value = $derived(shell.rightOpen ? (app.panels.find((p) => p.id === shell.panel) ?? app.panels[0]!).id : "main");
</script>

<div class="phone-tabs"><Capsule label="{app.name} sections" morph {items} {value} onchange={(v) => (v === "main" ? shell.closeOver() : shell.showPanel(v))} /></div>

<style>
  .phone-tabs { position: fixed; z-index: 5; left: var(--s-6); right: var(--s-6); bottom: max(var(--s-5), var(--safe-bottom)); display: flex; justify-content: center; }
  .phone-tabs :global(.cap) { width: 100%; max-width: 26rem; }
</style>

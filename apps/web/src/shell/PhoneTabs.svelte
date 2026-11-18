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

<nav class="phone-tabs" aria-label="{app.name} sections"><Capsule label="{app.name} sections" morph tall {items} {value} onchange={(v) => (v === "main" ? shell.dismiss() : shell.showPanel(v))} /></nav>

<style>
  .phone-tabs { transform: var(--pull-x, none); transition: transform var(--dur-sheet) var(--ease-sheet); position: fixed; z-index: 5; left: var(--s-6); right: var(--s-6); bottom: calc(var(--cap-offset) + var(--kb, 0px)); /* native: the bottom pill sits 28 above the screen edge, inside the home-indicator area */ display: flex; justify-content: center; }
  .phone-tabs :global(.cap) { width: 100%; max-width: 26rem; }
:global(.shell:has(.mid.dragging)) .phone-tabs { transition: none; }
</style>

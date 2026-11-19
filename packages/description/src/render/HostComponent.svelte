<script lang="ts">
  // A host component (item editor, transcript, composer, request card, sealed frame) is configured by the description and drawn by the host, never by plugin code. The page that embeds the renderer supplies each one in
  // `components` (the real editor, the transcript); a component it does not supply is drawn as a labelled, empty slot, so a description that uses one still draws and the missing piece is visible, not invented.
  import type { Component } from "../model";
  import { useApp } from "../invoke/context";
  let { component }: { component: Component } = $props();
  const app = useApp();
  const Host = $derived(app?.components?.[component.type]);
</script>

{#if Host}<Host config={component.config} id={component.id} />
{:else}<div class="slot" data-host-component={component.type} data-host-component-id={component.id} role="group" aria-label={component.type}>{component.type}</div>{/if}

<style>
  .slot { display: grid; place-items: center; min-height: 72px; flex: 1; border: 1px dashed color-mix(in oklab, var(--ink) 25%, transparent); border-radius: var(--r-select); color: var(--ink-3); font-size: var(--text-label); }
</style>

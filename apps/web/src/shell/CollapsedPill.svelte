<script lang="ts">
  // The left sidebar is collapsed: its way back and the apps in ONE two-part pill at the card's top-left corner, `[sidebar icon | current app icon]`. The first part shows the sidebar again (with its shortcut); the second opens a
  // dropdown of every app with its name and the current one checked (a `Menu`, morphing out of the pill), so the apps stay reachable without a second rail. A phone has the plain sidebar button instead (its sidebar is the list the
  // card slides over). Kept as one small component so the owner's look can change here alone.
  import { Icon, Menu, Panel, tip, type MenuItem } from "@calternal/ui";
  import type { AppDef } from "../apps/registry";
  import { navigate } from "../lib/route.svelte";
  import { ariaKeys, shortcuts } from "../lib/shortcuts.svelte";
  import { shell } from "./shell.svelte";
  import { switcherApps } from "./switcherApps.svelte";
  let { app }: { app: AppDef } = $props();
  let open = $state(false), trigger = $state<HTMLButtonElement>();
  const apps = $derived(switcherApps());
  const items = $derived<MenuItem[]>(apps.map((a) => ({ id: a.kind, label: a.name, icon: a.icon, checked: a.kind === app.kind })));
</script>

<Panel radius="pill" class="cp" role="group" aria-label="Sidebar and apps">
  <button type="button" class="part" aria-label="Show the sidebar" aria-keyshortcuts={ariaKeys(shortcuts.chordFor("sidebar"))} use:tip={{ label: "Show the sidebar" }} onclick={() => shell.toggleSidebar()}><Icon name="sidebar" /></button>
  <button type="button" class="part" bind:this={trigger} aria-label="Apps: {app.name}" aria-haspopup="menu" aria-expanded={open} use:tip={{ label: `Apps: ${app.name}` }} onclick={() => (open = !open)}><Icon name={app.icon} /></button>
</Panel>
<Menu {open} {items} label="Apps" {trigger} morph onclose={() => (open = false)} onpick={(id) => navigate(`/${id}`)} />

<style>
  :global(.cp) { display: inline-flex; height: var(--pill-h); }
  .part { width: var(--pill-h); height: var(--pill-h); display: grid; place-items: center; border-radius: var(--r-pill); color: var(--ink-2); transition: scale var(--dur-fast) var(--ease-out), color var(--dur-fast) var(--ease-out); }
  .part:active { scale: var(--press-scale); }
  @media (hover: hover) and (pointer: fine) { .part:hover { color: var(--ink); } }
</style>

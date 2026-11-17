<script lang="ts">
  // The left region. Not a card: it sits on the tinted background. Header (the app's icon and name; a label, not a control), the
  // attention card, the app's own body, and the footer: account picture, app switcher, search. Switching apps always happens here.
  import { Icon, IconButton, Menu, Pill, type MenuItem } from "@calternal/ui";
  import { APPS, type AppDef } from "../apps/registry";
  import { navigate } from "../lib/route.svelte";
  import AccountSheet from "./AccountSheet.svelte";
  import Attention from "./Attention.svelte";
  import SearchSheet from "./SearchSheet.svelte";
  import { shell } from "./shell.svelte";

  let { app }: { app: AppDef } = $props();
  let menu = $state(false);
  let account = $state(false);
  let search = $state(false);
  let trigger = $state<HTMLButtonElement>();
  const items = $derived<MenuItem[]>(APPS.map((a) => ({ id: a.kind, label: a.name, icon: a.icon, checked: a.kind === app.kind })));
</script>

<div class="side">
  <header>
    <span class="tile"><Icon name={app.icon} /></span>
    <p class="name">{app.name}</p>
    {#if !shell.narrow}<IconButton icon="sidebar" label="Hide the sidebar" onclick={() => shell.toggleSidebar()} />{/if}
  </header>

  <Attention items={app.attention?.() ?? []} />

  <div class="body"><app.Sidebar /></div>

  <footer>
    <button type="button" class="avatar" aria-label="Account: Kay" onclick={() => (account = true)}><span aria-hidden="true">K</span></button>
    <span class="switch">
      <Pill label={app.name} icon="apps" showLabel glass={false} aria-haspopup="menu" aria-expanded={menu} bind:el={trigger} onclick={() => (menu = !menu)} />
      <Menu open={menu} {items} label="Switch app" {trigger} onclose={() => (menu = false)} onpick={(k) => navigate(`/${k}`)} />
    </span>
    <IconButton icon="search" label="Search" onclick={() => (search = true)} />
  </footer>
</div>

<AccountSheet open={account} onclose={() => (account = false)} />
<SearchSheet open={search} onclose={() => (search = false)} />

<style>
  .side { height: 100%; display: flex; flex-direction: column; gap: var(--s-4); padding: var(--s-4) var(--s-4) 0; min-height: 0; }
  header { flex: none; display: flex; align-items: center; gap: var(--s-4); padding: var(--s-2) var(--s-2) var(--s-2) var(--s-3); min-height: calc(max(var(--pill-h), var(--hit-min))); }
  .tile { display: grid; place-items: center; width: calc(var(--pill-h) - var(--s-2)); height: calc(var(--pill-h) - var(--s-2)); border-radius: var(--r-control); background: var(--accent-soft); color: var(--accent); }
  .name { flex: 1; font-family: var(--font-rounded); font-size: var(--text-heading); font-weight: var(--strong-weight); letter-spacing: -0.01em; }
  .body { flex: 1; min-height: 0; overflow-y: auto; margin: 0 calc(var(--s-1) * -1); padding: 0 var(--s-1); }
  footer { flex: none; display: flex; align-items: center; gap: var(--s-3); padding: var(--s-4) 0 max(var(--s-4), var(--safe-bottom)); }
  .avatar { flex: none; width: max(var(--pill-h), var(--hit-min)); height: max(var(--pill-h), var(--hit-min)); border-radius: 50%; display: grid; place-items: center; background: color-mix(in oklab, var(--accent-base) 22%, rgb(var(--tint) / 0.6)); color: var(--accent); font-family: var(--font-rounded); font-weight: var(--strong-weight); transition: scale var(--dur-press) var(--ease-out); }
  .avatar:active { scale: var(--press-scale); }
  .switch { position: relative; flex: 1; min-width: 0; display: flex; justify-content: center; }
</style>

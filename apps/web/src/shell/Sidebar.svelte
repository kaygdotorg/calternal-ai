<script lang="ts">
  // The left region. Not a card: it sits on the wallpaper. A wordmark at the top (the person's own name for the product, in a cursive face, both set in
  // Settings; no icon) with the sidebar button at the other end; the attention card and the app's own body scroll as one column that fades out under
  // the wordmark row and the footer (no opaque band); the footer holds the account picture, the app switcher and search. Switching apps happens here.
  import { IconButton, Menu, Pill, type MenuItem } from "@calternal/ui";
  import { appearance } from "../appearance/prefs.svelte";
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
    <p class="mark">{appearance.prefs.brand}</p>
    {#if !shell.narrow}<IconButton icon="sidebar" label="Hide the sidebar" onclick={() => shell.toggleSidebar()} />{/if}
  </header>

  <div class="scroll edge-fade">
    <Attention items={app.attention?.() ?? []} />
    <div class="body"><app.Sidebar /></div>
  </div>

  <footer>
    <button type="button" class="avatar" aria-label="Account: Kay" onclick={() => (account = true)}><span aria-hidden="true">K</span></button>
    <span class="switch">
      <Pill label={app.name} icon={app.icon} showLabel glass={false} aria-haspopup="menu" aria-expanded={menu} bind:el={trigger} onclick={() => (menu = !menu)} />
      <Menu open={menu} {items} label="Switch app" {trigger} onclose={() => (menu = false)} onpick={(k) => navigate(`/${k}`)} />
    </span>
    <IconButton icon="search" label="Search" onclick={() => (search = true)} />
  </footer>
</div>

<AccountSheet open={account} onclose={() => (account = false)} />
<SearchSheet open={search} onclose={() => (search = false)} />

<style>
  /* The bars float over one scrolling column; its fades are as tall as the bars. */
  .side { --foot-h: calc(var(--pill-h) + var(--s-4) + max(var(--s-4), var(--safe-bottom))); position: relative; height: 100%; min-height: 0; }
  header { position: absolute; z-index: 2; top: 0; left: 0; right: 0; height: var(--bar-h); display: flex; align-items: center; justify-content: space-between; padding: 0 var(--s-4) 0 var(--s-6); }
  .mark { font-family: var(--font-brand); font-size: var(--text-title); font-weight: var(--strong-weight); line-height: 1; letter-spacing: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .scroll { position: absolute; inset: 0; overflow-y: auto; overscroll-behavior: contain; scrollbar-width: none; display: flex; flex-direction: column; gap: var(--s-4); padding: var(--bar-h) var(--s-4) var(--foot-h); --fade-t: var(--bar-h); --fade-b: var(--foot-h); }
  .body { flex: none; }
  footer { position: absolute; z-index: 2; left: 0; right: 0; bottom: 0; height: var(--foot-h); display: flex; align-items: flex-start; gap: var(--s-3); padding: var(--s-2) var(--s-4) 0; }
  .avatar { flex: none; width: var(--pill-h); height: var(--pill-h); border-radius: 50%; display: grid; place-items: center; background: color-mix(in oklab, var(--accent-base) 22%, rgb(var(--tint) / 0.6)); color: var(--accent); font-weight: var(--strong-weight); transition: scale var(--dur-fast) var(--ease-out); }
  .avatar:active { scale: var(--press-scale); }
  .switch { flex: 1; min-width: 0; display: flex; justify-content: center; }
</style>

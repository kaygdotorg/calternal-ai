<script lang="ts">
  // The left region. Not a card: it sits on the wallpaper. A wordmark at the top (the person's own name for the product, in a cursive face, both set in
  // Settings; no icon) with the sidebar button at the other end; the attention card and the app's own body scroll as one column that fades out under
  // the wordmark row and the footer (no opaque band); the footer holds the account picture, the app capsule (icon-only, three at a time) and search. Switching apps happens here.
  import { Avatar, Menu, Pill, type MenuItem } from "@calternal/ui";
  import { appearance } from "../appearance/prefs.svelte";
  import type { AppDef } from "../apps/registry";
  import { navigate } from "../lib/route.svelte";
  import { ariaKeys, shortcuts } from "../lib/shortcuts.svelte";
  import Attention from "./Attention.svelte";
  import Palette from "./Palette.svelte";
  import { pull } from "../lib/pull";
  import AppSwitcher from "./AppSwitcher.svelte";
  import { spaces } from "./spaces.svelte";
  import { self, signOutNow } from "./self.svelte";
  import { shell } from "./shell.svelte";

  let { app }: { app: AppDef } = $props();
  // One scroller serves every app: a new app starts at the top (an offset left by another app's list would put this one's first row under the wordmark).
  let scroller = $state<HTMLElement>();
  $effect(() => { void app.kind; if (scroller) scroller.scrollTop = 0; });
  // The account picture opens the account menu on every screen: the spaces (the current one checked; a search field above five of them, which makes it a sheet on a phone) and Settings. There are no space dots in the footer.
  let menu = $state(false);
  let trigger = $state<HTMLButtonElement>();
  const search = $derived(spaces.list.length > 5);
  $effect(() => { self.load(); });
  // Who is signed in comes first (it opens Settings, Account), then the spaces, Settings, and Sign out last: a person sees which account this is and can leave it in two taps (the picture, then the row).
  const items = $derived<MenuItem[]>([
    { id: "account", label: self.name, sub: self.email ?? `@${self.username}`, icon: "user" as const },
    ...spaces.list.map((s) => ({ id: `space:${s.id}`, label: s.name, checked: s.id === spaces.current.id })),
    { id: "settings", label: "Settings", icon: "settings" as const },
    { id: "signout", label: "Sign out", icon: "signout" as const, danger: true },
  ]);
  function pick(id: string) {
    if (id === "settings") navigate("/settings/appearance");
    else if (id === "account") navigate("/settings/account");
    else if (id === "signout") signOutNow();
    else { spaces.switchTo(id.slice(6)); navigate("/threads"); }
  }
</script>

<div class="side">
  <header>
    <p class="mark">{appearance.prefs.brand}</p>
    {#if !shell.narrow}<Pill icon="sidebar" label="Hide the sidebar" aria-keyshortcuts={ariaKeys(shortcuts.chordFor("sidebar"))} onclick={() => shell.toggleSidebar()} />{/if}
  </header>

  <div class="scroll edge-fade" bind:this={scroller} use:pull={{ refresh: async () => { await app.refresh?.(); } }}>
    <Attention items={app.attention?.() ?? []} />
    {#key spaces.current.id}<div class="body"><app.Sidebar /></div>{/key}
  </div>

  <footer>
    <button type="button" class="avatar" bind:this={trigger} aria-label="Account, spaces and settings: {self.name}" aria-haspopup="menu" aria-expanded={menu} onclick={() => (menu = !menu)}><Avatar name={self.name} size="md" /></button>
    <Menu open={menu} {items} {search} label="Account" {trigger} onclose={() => (menu = false)} onpick={pick} />
    <span class="switch"><AppSwitcher current={app.kind} /></span>
    <Pill icon="search" label="Search" aria-keyshortcuts={ariaKeys(shortcuts.chordFor("search"))} onclick={() => (shell.searchOpen = true)} />
  </footer>
</div>

<Palette open={shell.searchOpen} onclose={() => (shell.searchOpen = false)} />

<style>
  /* The bars float over one scrolling column; its fades are as tall as the bars. */
  .side { --foot-h: calc(var(--pill-h) + var(--s-4) + var(--foot-inset)); position: relative; height: 100%; min-height: 0; }
  header { position: absolute; z-index: 2; top: 0; left: 0; right: 0; height: var(--bar-h); display: flex; align-items: center; justify-content: space-between; padding: 0 var(--s-4) 0 var(--s-6); }
  .mark { font-family: var(--font-brand); font-size: var(--text-title); font-weight: var(--strong-weight); line-height: 1; letter-spacing: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .scroll { position: absolute; inset: 0; overflow-y: auto; overscroll-behavior: contain; scrollbar-width: none; display: flex; flex-direction: column; gap: var(--s-4); padding: var(--bar-h) var(--s-4) var(--foot-h); --fade-t: var(--bar-h); --fade-b: var(--foot-h); }
  .body { flex: none; }
  footer { position: absolute; z-index: 2; left: 0; right: 0; bottom: 0; height: var(--foot-h); display: flex; align-items: flex-start; gap: var(--s-3); padding: var(--s-2) var(--s-4) 0; }
  .avatar { flex: none; border-radius: 50%; display: grid; place-items: center; transition: scale var(--dur-fast) var(--ease-out); }
  .avatar:active { scale: var(--press-scale); }
  .switch { flex: 1; min-width: 0; display: flex; justify-content: center; }
  .body { animation: space-in var(--dur-base) var(--ease-out); }
  @keyframes space-in { from { opacity: 0; } }
</style>

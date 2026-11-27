<script lang="ts">
  // The left sidebar of the owner's sketch, flat on the background, top to bottom:
  // the top line (wordmark or app name + optional menu & hide-sidebar glass circle),
  // the scrollable body slot, and the foot: three matching glass pills (profile Av,
  // centred space pill with chevron, round search pill).
  // Shared verbatim from apps/mockup/src/shell/Sidebar.svelte.
  import type { Snippet } from "svelte";
  import { Icon, IconButton, Menu, Panel, type MenuItem } from "@calternal/ui";
  import Av from "./Av.svelte";
  import { layout } from "../layout.svelte";

  let {
    title,
    home,
    onhome,
    narrow = false,
    mac = false,
    cursive = false,
    accountName,
    accountItems = [],
    onAccountPick,
    spaceName,
    spaceItems = [],
    onSpacePick,
    spaceSearch = false,
    onsearch,
    titleSlot,
    headerActions,
    body,
    extra,
  }: {
    title: string;
    home: boolean;
    onhome: () => void;
    narrow?: boolean;
    mac?: boolean;
    cursive?: boolean;
    accountName: string;
    accountItems?: MenuItem[];
    onAccountPick?: (id: string) => void;
    spaceName: string;
    spaceItems?: MenuItem[];
    onSpacePick?: (id: string) => void;
    spaceSearch?: boolean;
    onsearch?: () => void;
    titleSlot?: Snippet;
    headerActions?: Snippet;
    body: Snippet;
    extra?: Snippet;
  } = $props();

  let menuOpen = $state(false);
  let spaceOpen = $state(false);
  let trig = $state<HTMLElement | null>(null);
  let strig = $state<HTMLElement | null>(null);

  const pick = (id: string) => { menuOpen = false; onAccountPick?.(id); };
  const pickSpace = (id: string) => { spaceOpen = false; onSpacePick?.(id); };
</script>

<nav class="sb" class:narrow class:mac aria-label="Sidebar" data-sidebar>
  <header class="wm" data-top-line>
    <h1 style:font-family={cursive ? "var(--wm-font)" : "var(--font-ui)"} class:ui={!cursive} class:long={title.length > 9}>
      <button type="button" class="wmb" disabled={home} onclick={onhome} aria-label={home ? "calternal" : `${title}: back to the start`}>
        {#if titleSlot}{@render titleSlot()}{:else}<span>{title}</span>{/if}
      </button>
    </h1>
    <span class="wt">
      {#if headerActions}{@render headerActions()}{/if}
      {#if !narrow && !mac}
        <Panel radius="pill" class="roundP tbp">
          <IconButton icon="sidebar" label="Hide the sidebar" aria-keyshortcuts="Control+Meta+S" onclick={() => (layout.leftOpen = false)} />
        </Panel>
      {/if}
    </span>
  </header>

  {@render body()}

  <footer class="foot" data-foot>
    <!-- three matching pills, one height and one material (the toolbar capsule): profile picture, centred space pill, search -->
    <Panel radius="pill" class="fp round tbp">
      <button type="button" class="pf" bind:this={trig} aria-haspopup="menu" aria-expanded={menuOpen} aria-label="Account" onclick={() => (menuOpen = true)} data-profile>
        <Av name={accountName} size="calc(var(--fp-h) - var(--s-4))" />
      </button>
    </Panel>
    <Panel radius="pill" class="fp wide">
      <button type="button" class="sp" bind:this={strig} aria-haspopup="menu" aria-expanded={spaceOpen} onclick={() => (spaceOpen = true)} data-space>
        <strong>{spaceName}</strong>
        <Icon name="chevron-down" inline />
      </button>
    </Panel>
    <Panel radius="pill" class="fp round roundP tbp">
      <IconButton icon="search" label="Search" onclick={() => onsearch?.()} />
    </Panel>
    {#if extra}{@render extra()}{/if}
    <Menu open={menuOpen} items={accountItems} label="Account" onpick={pick} onclose={() => (menuOpen = false)} trigger={trig} />
    <Menu open={spaceOpen} items={spaceItems} search={spaceSearch} label="Spaces" onpick={pickSpace} onclose={() => (spaceOpen = false)} trigger={strig} />
  </footer>
</nav>

<style>
  .sb { position: relative; display: flex; flex-direction: column; height: 100%; min-height: 0; padding: 0 var(--s-3); gap: var(--s-3); box-sizing: border-box; }
  .wm { flex: none; display: flex; align-items: center; justify-content: space-between; gap: var(--s-3); padding: var(--s-5) var(--s-5) var(--s-1); } .wm h1 { min-width: 0; } .wt { flex: none; display: inline-flex; gap: var(--s-3); } .sb.mac .wm { padding-top: 44px; }
  .wm h1 { margin: 0; font-size: 32px; line-height: 1.15; font-weight: 600; letter-spacing: 0.01em; font-synthesis: none; } .wm h1.ui { font-weight: var(--strong-weight); letter-spacing: 0; } .wm h1.long { font-size: 26px; letter-spacing: 0; }
  .wmb { display: block; max-width: 100%; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; text-align: start; color: inherit; font: inherit; letter-spacing: inherit; padding-inline: var(--s-2); margin-inline: calc(var(--s-2) * -1); cursor: default; } .wmb:not(:disabled) { cursor: pointer; }
  .foot { container-type: inline-size; flex: none; display: flex; align-items: center; justify-content: flex-start; gap: var(--s-4); padding: 0 var(--s-3) var(--s-3) var(--s-3); --fp-h: var(--tb-h); }
  .foot :global(.fp) { flex: none; height: var(--fp-h); }
  .foot :global(.fp.round) { width: var(--fp-h) !important; height: var(--fp-h) !important; display: grid; place-items: center; --ib: var(--fp-h); }
  .foot :global(.fp.wide) { flex: 1; min-width: 0; display: flex; }
  .pf { width: 100%; height: 100%; display: grid; place-items: center; border-radius: 50%; transition: scale var(--dur-fast) var(--ease-out), background-color var(--dur-fast) var(--ease-out); }
  .sp { flex: 1; min-width: 0; display: flex; align-items: center; justify-content: center; gap: var(--s-3); padding: 0 var(--s-5); border-radius: var(--r-pill); font-size: var(--text-ui); line-height: 1.25; transition: scale var(--dur-fast) var(--ease-out), background-color var(--dur-fast) var(--ease-out); } .sp strong { min-width: 0; font-weight: var(--strong-weight); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } .sp :global(svg) { flex: none; color: var(--ink-3); }
  .pf:active, .sp:active { scale: var(--press-scale); }
  @media (hover: hover) and (pointer: fine) { .pf:hover, .sp:hover { background: var(--hover); } }
  .pf:focus-visible, .sp:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  .narrow .foot { position: absolute; left: var(--s-3); right: var(--s-3); bottom: calc(var(--s-3) + env(safe-area-inset-bottom, 0px)); padding: 0; z-index: 2; pointer-events: none; --fp-h: var(--tb-search-h); }
  .narrow .foot > :global(*) { pointer-events: auto; }
  :global(.roundP) { flex: none; display: grid; place-items: center; }
</style>

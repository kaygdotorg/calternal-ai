<script lang="ts">
  // A Settings page: the title, one line of purpose, then groups of rows in a centred column (at most 560 px, like Notes). `/settings/<section>/<setting-id>`
  // opens the page with that row highlighted for a moment and scrolled into view. Admin pages end with a Danger area.
  import { tick } from "svelte";
  import { chromeTrack } from "@calternal/ui";
  import { route, segments } from "../../lib/route.svelte";
  import { serverMode } from "../../server/client";
  import SettingGroup from "./SettingGroup.svelte";
  import SettingRow from "./SettingRow.svelte";
  import { pageById, visibleSections } from "./registry";
  let { id }: { id?: string } = $props();
  const page = $derived(pageById(id));
  const target = $derived(segments()[0] === "settings" ? segments()[2] : undefined);
  // Every page is one flat scrolling page at every width: sections in order under their headings (the sticky header pill says which one you are in). How wide it is follows the layout preference the chat uses (Appearance, Layout): Wide uses the card's width, Centred a readable column.
  let scrollEl = $state<HTMLElement>(), width = $state(0);
  // Fixtures only: `?scroll=N` opens the page already scrolled N px, so a phone Simulator (which cannot be dragged by script) can show the sticky header pill.
  $effect(() => { const n = Number(new URLSearchParams(location.search).get("scroll")); if (scrollEl && width && n > 0 && !serverMode()) scrollEl.scrollTop = n; });
  let lit = $state<string | undefined>();
  $effect(() => {
    const t = target;
    if (!t) { lit = undefined; return; }
    void tick().then(() => {
      lit = t;
      document.querySelector(`[data-setting="${CSS.escape(t)}"]`)?.scrollIntoView({ block: "center" });
    });
    const off = setTimeout(() => (lit = undefined), 2200);
    return () => clearTimeout(off);
  });
</script>

<div class="scroller edge-fade" bind:this={scrollEl} use:chromeTrack style:--fade-t="calc(var(--safe-top) + var(--bar-h))" style:--fade-b="var(--s-8)" bind:clientWidth={width}>
<div class="page">
  {#if page}
    <h1 data-chrome-title>{page.name}</h1>
    <p class="blurb">{page.blurb}</p>
    {#each page.groups as g}
      <SettingGroup title={g.title} danger={g.danger}>
        {#each g.settings as s (s.id)}<SettingRow {s} highlighted={lit === s.id} />{/each}
      </SettingGroup>
    {/each}
  {:else}
    <h1>Settings</h1>
    <p class="blurb">Choose a section on the left.</p>
    {#each visibleSections() as s}<p class="soft">{s.name}: {s.pages.map((p) => p.name).join(", ")}</p>{/each}
  {/if}
</div>
</div>

<style>
  /* A settings page taller than the window scrolls inside the card; the shell never grows (its sidebars and their footer stay in view). */
  .scroller { container: settings-page / inline-size; position: absolute; inset: 0; overflow-y: auto; overscroll-behavior: contain; scrollbar-width: none; }
  .page { box-sizing: border-box; min-width: 0; overflow-x: clip; width: 100%; max-width: 35rem; margin: 0 auto; padding: calc(var(--safe-top) + var(--bar-h) + var(--s-6)) var(--s-6) var(--s-10); }
  /* Wide (the default on desktop; the layout preference, Appearance, Layout) takes the card's width, with descriptions kept to a readable line; Centred keeps the column above. */
  :global(html[data-chat-wide]) .page { max-width: none; }
  h1 { font-size: var(--text-title); font-weight: var(--strong-weight); }
  .blurb { margin-top: var(--s-3); color: var(--ink-2); }
  .soft { margin-top: var(--s-4); color: var(--ink-3); font-size: var(--text-label); }
</style>

<script lang="ts">
  // A Settings page: the title, one line of purpose, then groups of rows in a centred column (at most 560 px, like Notes). `/settings/<section>/<setting-id>`
  // opens the page with that row highlighted for a moment and scrolled into view. Admin pages end with a Danger area.
  import { tick } from "svelte";
  import { SplitView, chromeTrack } from "@calternal/ui";
  import { route, segments } from "../../lib/route.svelte";
  import { serverMode } from "../../server/client";
  import SettingGroup from "./SettingGroup.svelte";
  import SettingRow from "./SettingRow.svelte";
  import { pageById, visibleSections } from "./registry";
  let { id }: { id?: string } = $props();
  const page = $derived(pageById(id));
  const target = $derived(segments()[0] === "settings" ? segments()[2] : undefined);
  // A page that opted into sections is two panes only where the card is wide (the list of sections, the selected one beside it, the first selected until the person picks); narrow, it is one flat page: every section in order under its heading, full width, no drill-in and no Back. The address's setting id selects its section.
  let sel = $state<number | null>(null), width = $state(0), scrollEl = $state<HTMLElement>();
  // Fixtures only: `?scroll=N` opens the page already scrolled N px, so a phone Simulator (which cannot be dragged by script) can show the sticky header pill.
  $effect(() => { const n = Number(new URLSearchParams(location.search).get("scroll")); if (scrollEl && width && n > 0 && !serverMode()) scrollEl.scrollTop = n; });
  const wide = $derived(width >= 640);
  $effect(() => { void page?.id; sel = null; });
  $effect(() => {
    if (!page?.split) return;
    const t = target;
    const i = t ? page.groups.findIndex((g) => g.settings.some((x) => x.id === t)) : -1;
    if (i >= 0) sel = i; else if (sel === null) sel = 0;
  });
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
{#if width === 0}
  <!-- the card is not measured yet: nothing, so the page never flashes the wrong arrangement -->
{:else if page?.split && wide}
  <div class="page split">
    <h1 data-chrome-title>{page.name}</h1>
    <p class="blurb">{page.blurb}</p>
    <div class="sp">
      <SplitView listLabel="{page.name} sections" detailLabel={sel !== null ? page.groups[sel]?.title : page.name}>
        {#snippet list()}
          <ul class="sections">
            {#each page.groups as g, i (g.title)}
              <li><button type="button" class="sec" aria-current={sel === i ? "true" : undefined} onclick={() => (sel = i)}>{g.title}</button></li>
            {/each}
          </ul>
        {/snippet}
        {#snippet detail()}
          {#if sel !== null && page.groups[sel]}
            {@const g = page.groups[sel]!}
            <SettingGroup title={g.title} danger={g.danger}>
              {#each g.settings as s (s.id)}<SettingRow {s} highlighted={lit === s.id} />{/each}
            </SettingGroup>
          {/if}
        {/snippet}
      </SplitView>
    </div>
  </div>
{:else}
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
{/if}
</div>

<style>
  /* A settings page taller than the window scrolls inside the card; the shell never grows (its sidebars and their footer stay in view). */
  .scroller { position: absolute; inset: 0; overflow-y: auto; overscroll-behavior: contain; scrollbar-width: none; }
  .page { box-sizing: border-box; min-width: 0; overflow-x: clip; width: 100%; max-width: 35rem; margin: 0 auto; padding: calc(var(--safe-top) + var(--bar-h) + var(--s-6)) var(--s-6) var(--s-10); }
  h1 { font-size: var(--text-title); font-weight: var(--strong-weight); }
  .page.split { max-width: 60rem; }
  .sp { margin-top: var(--s-6); min-height: 30rem; }
  .blurb { margin-top: var(--s-3); color: var(--ink-2); }
  .sections { display: grid; gap: var(--s-1); }
  .sec { width: 100%; min-height: var(--row-h-settings); padding: 0 var(--s-5); border-radius: var(--r-select); text-align: start; font-size: var(--text-ui); color: var(--ink-2); }
  .sec[aria-current="true"] { background: var(--accent-soft); color: var(--accent); font-weight: var(--strong-weight); }
  @media (hover: hover) and (pointer: fine) { .sec:not([aria-current="true"]):hover { background: var(--hover); color: var(--ink); } }
  .soft { margin-top: var(--s-4); color: var(--ink-3); font-size: var(--text-label); }
</style>

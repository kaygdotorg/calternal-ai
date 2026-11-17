<script lang="ts">
  // A Settings page: the title, one line of purpose, then groups of rows in a centred column (at most 560 px, like Notes). `/settings/<section>/<setting-id>`
  // opens the page with that row highlighted for a moment and scrolled into view. Admin pages end with a Danger area.
  import { tick } from "svelte";
  import { navigate, route, segments } from "../../lib/route.svelte";
  import SettingsSheet from "../../appearance/SettingsSheet.svelte";
  import { shell } from "../../shell/shell.svelte";
  import SettingGroup from "./SettingGroup.svelte";
  import SettingRow from "./SettingRow.svelte";
  import { pageById, SECTIONS } from "./registry";
  let { id }: { id?: string } = $props();
  const page = $derived(pageById(id));
  const target = $derived(segments()[0] === "settings" ? segments()[2] : undefined);
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

<div class="scroller edge-fade" style:--fade-t="calc(var(--safe-top) + var(--bar-h))" style:--fade-b="var(--s-8)">
{#if !shell.narrow && route.query.get("sheet") === "1"}
  <!-- A desktop has no sheet for settings: wallpaper and fonts are a page of the Settings app (a phone keeps its bottom sheet, mounted by App). -->
  <SettingsSheet inline open onclose={() => navigate("/settings/appearance")} />
{:else}
<div class="page">
  {#if page}
    <h1>{page.name}</h1>
    <p class="blurb">{page.blurb}</p>
    {#each page.groups as g}
      <SettingGroup title={g.title} danger={g.danger}>
        {#each g.settings as s (s.id)}<SettingRow {s} highlighted={lit === s.id} />{/each}
      </SettingGroup>
    {/each}
  {:else}
    <h1>Settings</h1>
    <p class="blurb">Choose a section on the left.</p>
    {#each SECTIONS as s}<p class="soft">{s.name}: {s.pages.map((p) => p.name).join(", ")}</p>{/each}
  {/if}
</div>
{/if}
</div>

<style>
  /* A settings page taller than the window scrolls inside the card; the shell never grows (its sidebars and their footer stay in view). */
  .scroller { position: absolute; inset: 0; overflow-y: auto; overscroll-behavior: contain; scrollbar-width: none; }
  .page { box-sizing: border-box; min-width: 0; overflow-x: clip; width: 100%; max-width: 35rem; margin: 0 auto; padding: calc(var(--safe-top) + var(--bar-h) + var(--s-6)) var(--s-6) var(--s-10); }
  h1 { font-size: var(--text-title); font-weight: var(--strong-weight); }
  .blurb { margin-top: var(--s-3); color: var(--ink-2); }
  .soft { margin-top: var(--s-4); color: var(--ink-3); font-size: var(--text-label); }
</style>

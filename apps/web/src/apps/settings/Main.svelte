<script lang="ts">
  // A Settings page: the title, one line of purpose, then groups of rows in a centred column (at most 560 px, like Notes). `/settings/<section>/<setting-id>`
  // opens the page with that row highlighted for a moment and scrolled into view. Admin pages end with a Danger area.
  import { tick } from "svelte";
  import { segments } from "../../lib/route.svelte";
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

<style>
  .page { box-sizing: border-box; min-width: 0; overflow-x: clip; width: 100%; max-width: 35rem; margin: 0 auto; padding: calc(var(--safe-top) + var(--bar-h) + var(--s-6)) var(--s-6) var(--s-10); }
  h1 { font-size: var(--text-title); font-weight: var(--strong-weight); }
  .blurb { margin-top: var(--s-3); color: var(--ink-2); }
  .soft { margin-top: var(--s-4); color: var(--ink-3); font-size: var(--text-label); }
</style>

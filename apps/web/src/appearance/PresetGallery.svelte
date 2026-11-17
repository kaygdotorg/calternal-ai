<script lang="ts">
  // The preset gallery: one tile per preset (backdrop and palette together). Used by the appearance settings and by the first-run sheet.
  import { PRESETS } from "./presets";
  import { appearance as A } from "./prefs.svelte";
  let { onpick }: { onpick?: (id: string) => void } = $props();
  const p = $derived(A.prefs);
  $effect(() => { void A.ensureThumbs(); });
</script>

<div class="presets" role="radiogroup" aria-label="Theme presets">
  {#each PRESETS as pr (pr.id)}
    {@const th = pr.theme ? pr.theme[A.drawn] : null}
    {@const wp = pr.wallpaper === "none" ? null : A.wallpapers.find((w) => w.id === (pr.wallpaper === "none" ? "" : pr.wallpaper)) ?? (pr.wallpaper === "Nkwrpa_v0WA" ? A.wallpapers.find((w) => w.id === "haze") : undefined)}
    <button type="button" role="radio" class="preset" aria-checked={p.presetId === pr.id} aria-label={pr.name} onclick={() => { A.applyPreset(pr.id); onpick?.(pr.id); }}>
      <span class="mini" style:--bg={th ? th.paper : "var(--paper)"} style:--ink={th ? th.ink : "var(--ink)"} style:--acc={th ? th.accent : "var(--accent-base)"} style:background-image={wp ? `url(${A.thumbOf(wp)})` : "none"} aria-hidden="true">
        <i class="ms"></i><i class="mc"><b></b><b></b></i><i class="mr"></i>
      </span>
      <span class="pn">{pr.name}</span>
    </button>
  {/each}
</div>

<style>
  .presets { display: grid; grid-template-columns: repeat(auto-fill, minmax(6.5rem, 1fr)); gap: var(--s-4); }
  .preset { display: grid; gap: var(--s-2); text-align: start; border-radius: var(--r-card); }
  .preset[aria-checked="true"] .mini { outline: 2px solid var(--accent); outline-offset: 2px; }
  .mini { aspect-ratio: 4 / 3; border-radius: var(--r-card); background: var(--bg) center / cover; display: grid; grid-template-columns: 22% 1fr 22%; gap: 6%; padding: 6%; overflow: hidden; box-shadow: inset 0 0 0 1px rgb(var(--ink-rgb) / 0.08); }
  .mini i { display: block; border-radius: var(--r-select); }
  .mc { background: var(--bg); box-shadow: 0 1px 4px rgb(0 0 0 / 0.18); padding: 12% 10%; display: grid; align-content: start; gap: 12%; }
  .mc b { height: 3px; border-radius: 2px; background: var(--ink); opacity: 0.6; }
  .mc b:first-child { width: 55%; background: var(--acc); opacity: 1; }
  .pn { font-size: var(--text-label); font-weight: var(--strong-weight); padding: 0 var(--s-1); }
</style>

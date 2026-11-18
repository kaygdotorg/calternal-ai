<script lang="ts">
  // The preset gallery: one tile per preset (backdrop and palette together). Used by the appearance settings and by the Look step of the onboarding.
  import tokens from "../../../../packages/ui/tokens/tokens.json" with { type: "json" };
  import { PRESETS } from "./presets";
  import { appearance as A } from "./prefs.svelte";
  let { onpick }: { onpick?: (id: string) => void } = $props();
  const p = $derived(A.prefs);
  $effect(() => { void A.ensureThumbs(); });
  /** The shell's own paper, ink and accent per scheme, for presets that keep its tokens. */
  const own = { light: { paper: tokens.light.paper, ink: tokens.light.ink, accent: tokens.base["accent-base"] }, dark: { paper: tokens.dark.paper, ink: tokens.dark.ink, accent: tokens.base["accent-base"] } };
</script>

<div class="presets" role="radiogroup" aria-label="Theme presets">
  {#each PRESETS as pr (pr.id)}
    {@const th = pr.theme ? pr.theme[A.drawn] : own[A.drawn]}
    {@const wp = pr.wallpaper === "none" ? null : A.wallpapers.find((w) => w.id === (pr.wallpaper === "none" ? "" : pr.wallpaper)) ?? (pr.wallpaper === "Nkwrpa_v0WA" ? A.wallpapers.find((w) => w.id === "haze") : undefined)}
    <button type="button" role="radio" class="preset" aria-checked={p.presetId === pr.id} aria-label={pr.name} onclick={() => { A.applyPreset(pr.id); onpick?.(pr.id); }}>
      <span class="mini" style:--paper={th.paper} style:--ink={th.ink} style:--acc={th.accent} style:background-image={wp ? `url(${A.thumbOf(wp)})` : undefined} aria-hidden="true">
        <i class="ms"></i><i class="mc"><em class="ctl"></em><b></b><b></b></i>
      </span>
      <span class="pn">{pr.name}</span>
    </button>
  {/each}
</div>

<style>
  .presets { display: grid; grid-template-columns: repeat(auto-fill, minmax(6.5rem, 1fr)); gap: var(--s-4); }
  .preset { display: grid; gap: var(--s-2); text-align: start; border-radius: var(--r-card); }
  .preset[aria-checked="true"] .mini { outline: 2px solid var(--accent); outline-offset: 2px; }
  /* A tile is a tiny mock of the app in that preset, in the scheme that is on now: its wallpaper (or a ground a step from its paper), its card, an accent-coloured control and two lines in its text colours. */
  .mini { --ground: color-mix(in oklab, var(--paper) 90%, var(--ink)); aspect-ratio: 4 / 3; border-radius: var(--r-card); background-color: var(--ground); background-size: cover; background-position: center; display: grid; grid-template-columns: 24% 1fr; gap: 6%; padding: 7%; overflow: hidden; box-shadow: inset 0 0 0 1px rgb(var(--ink-rgb) / 0.08); }
  .mini i { display: block; border-radius: var(--r-select); }
  .ms { background: color-mix(in oklab, var(--paper) 55%, transparent); }
  .mc { background: var(--paper); box-shadow: 0 1px 4px rgb(0 0 0 / 0.18); padding: 12% 10%; display: grid; align-content: start; gap: 11%; }
  .mc .ctl { display: block; width: 62%; height: 7px; border-radius: 4px; background: var(--acc); }
  .mc b { height: 3px; border-radius: 2px; background: var(--ink); }
  .mc b:last-child { width: 70%; opacity: 0.45; }
  .pn { font-size: var(--text-label); font-weight: var(--strong-weight); padding: 0 var(--s-1); }
</style>

<script lang="ts">
  // The preset gallery: one tile per preset (backdrop and palette together). Used by the appearance settings and by the Look step of the onboarding.
  import tokens from "../../../../packages/ui/tokens/tokens.json" with { type: "json" };
  import { PRESETS } from "./presets";
  import { appearance as A } from "./prefs.svelte";
  let { onpick }: { onpick?: (id: string) => void } = $props();
  const p = $derived(A.prefs);
  $effect(() => { void A.ensureThumbs(); });
  /** The shell's own paper and ink per scheme, for presets that keep its tokens (a tile shows the preset's colours, not the current scheme's). */
  const own = { light: { paper: tokens.light.paper, ink: tokens.light.ink }, dark: { paper: tokens.dark.paper, ink: tokens.dark.ink }, accent: tokens.base["accent-base"] };
  /** A tile is drawn with the preset's light colours on one diagonal half and its dark colours on the other, whatever scheme is on now, so presets can be told apart in either. */
  const split = (l: string, d: string) => `linear-gradient(135deg, ${l} 50%, ${d} 50%)`;
</script>

<div class="presets" role="radiogroup" aria-label="Theme presets">
  {#each PRESETS as pr (pr.id)}
    {@const lt = pr.theme ? pr.theme.light : own.light}
    {@const dk = pr.theme ? pr.theme.dark : own.dark}
    {@const acc = pr.theme ? pr.theme[A.drawn].accent : own.accent}
    {@const wp = pr.wallpaper === "none" ? null : A.wallpapers.find((w) => w.id === (pr.wallpaper === "none" ? "" : pr.wallpaper)) ?? (pr.wallpaper === "Nkwrpa_v0WA" ? A.wallpapers.find((w) => w.id === "haze") : undefined)}
    <button type="button" role="radio" class="preset" aria-checked={p.presetId === pr.id} aria-label={pr.name} onclick={() => { A.applyPreset(pr.id); onpick?.(pr.id); }}>
      <span class="mini" style:--bg={split(lt.paper, dk.paper)} style:--ink={split(lt.ink, dk.ink)} style:--acc={acc} style:background-image={wp ? `url(${A.thumbOf(wp)})` : undefined} aria-hidden="true">
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
  .mini { aspect-ratio: 4 / 3; border-radius: var(--r-card); background: var(--bg); background-size: cover; background-position: center; display: grid; grid-template-columns: 22% 1fr 22%; gap: 6%; padding: 6%; overflow: hidden; box-shadow: inset 0 0 0 1px rgb(var(--ink-rgb) / 0.08); }
  .mini i { display: block; border-radius: var(--r-select); }
  .mc { background: var(--bg); box-shadow: 0 1px 4px rgb(0 0 0 / 0.18); padding: 12% 10%; display: grid; align-content: start; gap: 12%; }
  .mc b { height: 3px; border-radius: 2px; background: var(--ink); opacity: 0.8; }
  .mc b:first-child { width: 55%; background: var(--acc); opacity: 1; }
  .pn { font-size: var(--text-label); font-weight: var(--strong-weight); padding: 0 var(--s-1); }
</style>

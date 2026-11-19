<script lang="ts">
  // Page: the theme row first ("Same as app" and the themes), then what the page is made of: Frost (one switch: the page at the usual see-through density with a 20 px blur, or solid paper), Grain (only
  // meaningful on frost, so dimmed while Frost is off), Frame, Width, then the page's colour. Every row changes ONLY this note's page. Ported from the existing app's components/style/PageTab.svelte and
  // ThemeRow.svelte / Swatches.svelte (origin/dev); the Cover, Separator and the "Wide page" tickbox have no counterpart yet (docs/notes-style.md). Width is the three-way choice the key has.
  import { Selector, Switch } from "@calternal/ui";
  import { currentScheme, onScheme } from "@calternal/ui";
  import { onMount } from "svelte";
  import { PRESETS } from "../../../appearance/presets";
  import { THEMES } from "./apply";
  import { PAPERS, paperOf } from "./page";
  import { noteStyle } from "./store.svelte";
  const page = $derived(noteStyle.page);
  const off = $derived(noteStyle.readOnly);
  const frosted = $derived(page.opacity !== null && page.opacity < 0.99);
  const WIDTHS = [{ value: "narrow", label: "Narrow" }, { value: "wide", label: "Wide" }, { value: "full", label: "Full" }];
  let scheme = $state(currentScheme());
  onMount(() => onScheme((s) => (scheme = s)));
  /** Three colours of a theme's palette for its tile: paper, ink and accent, as the card will show them. */
  const dots = (id: string) => { const t = PRESETS.find((p) => p.id === id)?.theme?.[scheme]; return t ? [t.paper, t.ink, t.accent] : []; };
  const set = (patch: Record<string, string | null>) => noteStyle.set(patch);
</script>

<section aria-labelledby="st-theme">
  <h3 class="section-label" id="st-theme">Theme</h3>
  <div class="themes" role="radiogroup" aria-label="Theme">
    <button type="button" role="radio" aria-checked={page.theme === ""} disabled={off} class="tile" onclick={() => set({ "calternal-theme": null })}><span class="dots"><i class="same"></i></span><span class="nm">Same as app</span></button>
    {#each THEMES as t (t.id)}
      <button type="button" role="radio" aria-checked={page.theme === t.id} disabled={off} class="tile" onclick={() => set({ "calternal-theme": t.id })}>
        <span class="dots">{#each dots(t.id) as c}<i style:background={c}></i>{/each}</span><span class="nm">{t.name}</span>
      </button>
    {/each}
  </div>
</section>
<section aria-labelledby="st-page">
  <h3 class="section-label" id="st-page">Page</h3>
  <Switch label="Frost" checked={frosted} disabled={off} onchange={(on) => set(on ? { "calternal-page-opacity": "80%", "calternal-page-blur": "20px" } : { "calternal-page-opacity": null, "calternal-page-blur": null, "calternal-grain": null })} />
  <!-- Grain is a texture ON the frosted page: with Frost off the row is dimmed and cannot be pressed. -->
  <Switch label="Grain" checked={page.grain} disabled={off || !frosted} onchange={(on) => set({ "calternal-grain": on ? "on" : null })} />
  <Switch label="Frame" checked={page.frame} disabled={off} onchange={(on) => set({ "calternal-frame": on ? null : "off" })} />
  <!-- On a phone the page is always the card's width, so a width would do nothing there. -->
  {#if !noteStyle.phone}
    <div class="field"><span class="cap">Width</span><Selector label="Width" options={WIDTHS} value={page.width} onchange={(w) => set({ "calternal-width": w === "narrow" ? null : w })} /></div>
  {/if}
</section>
<section aria-labelledby="st-paper">
  <h3 class="section-label" id="st-paper">Page colour</h3>
  <div class="swatches" role="group" aria-label="Page colour">
    {#each PAPERS as p (p.id)}
      <button type="button" class="sw" aria-label={p.label} title={p.label} aria-pressed={page.colour === p.id} disabled={off} style:--fill={paperOf(p.id) ?? "var(--paper)"} onclick={() => set({ "calternal-page-colour": p.id === "default" ? null : p.id })}></button>
    {/each}
  </div>
</section>

<style>
  section { display: grid; grid-template-columns: minmax(0, 1fr); gap: var(--s-3); }
  section + section { margin-top: var(--s-5); }
  .themes { display: grid; grid-template-columns: repeat(auto-fill, minmax(5.5rem, 1fr)); gap: var(--s-2); }
  .tile { display: grid; gap: var(--s-2); justify-items: start; padding: var(--s-3) var(--s-4); border-radius: var(--r-select); background: rgb(var(--ink-rgb) / 0.05); text-align: start; transition: scale var(--dur-fast) var(--ease-out); }
  .tile:active:not(:disabled) { scale: var(--press-scale); }
  .tile[aria-checked="true"] { background: var(--accent-soft); box-shadow: 0 0 0 1.5px var(--accent-base); }
  .tile:disabled { opacity: 0.5; }
  .dots { display: flex; gap: var(--s-1); }
  .dots i { width: var(--s-5); height: var(--s-5); border-radius: 50%; box-shadow: inset 0 0 0 1px rgb(var(--ink-rgb) / 0.2); }
  .dots i.same { background: conic-gradient(var(--paper) 0 50%, var(--ink) 0); }
  .nm { font-size: var(--text-label); color: var(--ink-2); }
  .field { display: grid; gap: var(--s-2); margin-top: var(--s-2); }
  .cap { font-size: var(--text-ui); }
  .swatches { display: flex; flex-wrap: wrap; gap: var(--s-3); }
  .sw { width: var(--s-8); height: var(--s-8); border-radius: 50%; background: var(--fill); box-shadow: inset 0 0 0 1px rgb(var(--ink-rgb) / 0.25); transition: scale var(--dur-fast) var(--ease-out); }
  .sw:active:not(:disabled) { scale: var(--press-scale); }
  .sw[aria-pressed="true"] { box-shadow: inset 0 0 0 1px rgb(var(--ink-rgb) / 0.25), 0 0 0 2px var(--paper), 0 0 0 4px var(--accent-base); }
  .sw:disabled { opacity: 0.5; }
</style>

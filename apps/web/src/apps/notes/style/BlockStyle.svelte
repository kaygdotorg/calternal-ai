<script lang="ts">
  // Style · this block: text style, kind and colour of the block the caret is in. Ported from the existing app's components/style/BlockStyle.svelte (origin/dev): Title, Subtitle and Heading, Strong and
  // Body, the five kinds as icon pills, and the colour row. The caret is watched only while this tab is on screen (the editor adds its listener for the length of the watch), so typing pays nothing for it.
  // The old colour swatches were not wired to anything either (no handler in the old component); they are drawn and disabled until a colour has a markdown form (docs/notes-style.md).
  import { Icon, tip } from "@calternal/ui";
  import type { IconName } from "@calternal/ui";
  import { noteStyle } from "./store.svelte";
  type T = "h1" | "h2" | "h3" | "p" | "li" | "ol" | "todo" | "quote" | "code";
  $effect(() => { void noteStyle.epoch; return noteStyle.watchCaret(); });
  const current = $derived(noteStyle.caret.type);
  const strong = $derived(noteStyle.caret.bold);
  const off = $derived(noteStyle.readOnly);
  const TITLES: [T, string][] = [["h1", "Title"], ["h2", "Subtitle"], ["h3", "Heading"]];
  const KINDS: [T, string, IconName][] = [["li", "List", "list"], ["ol", "Numbered list", "list"], ["todo", "To-do", "check"], ["quote", "Quote", "quote"], ["code", "Code", "code"]];
  const COLOURS: [string, string][] = [["Default", "var(--ink)"], ["Clay", "#b4533a"], ["Marigold", "#b97a12"], ["Moss", "#5f7d3f"], ["Sea", "#2f7a86"], ["Dusk", "#5b5fa8"], ["Plum", "#9a4a86"]];
</script>

<section>
  <h3 class="section-label">Text style</h3>
  <div class="seg">
    {#each TITLES as [t, label] (t)}<button type="button" aria-pressed={current === t} disabled={off} onclick={() => noteStyle.convert(t)}>{label}</button>{/each}
  </div>
  <div class="seg">
    <button type="button" aria-pressed={strong} aria-disabled={current === "h1" || undefined} disabled={off} onclick={() => current !== "h1" && noteStyle.toggleBold()}><strong>Strong</strong></button>
    <button type="button" aria-pressed={current === "p" && !strong} disabled={off} onclick={() => noteStyle.convert("p")}>Body</button>
  </div>
</section>
<section>
  <h3 class="section-label">Block</h3>
  <div class="kinds">
    {#each KINDS as [t, label, icon] (t)}
      <button type="button" class="kind" aria-label={label} aria-pressed={current === t} disabled={off} use:tip={{ label }} onclick={() => noteStyle.convert(current === t ? "p" : t)}><Icon name={icon} /></button>
    {/each}
  </div>
</section>
<section>
  <h3 class="section-label">Colour</h3>
  <div class="swatches" role="group" aria-label="Text colour">
    {#each COLOURS as [name, c] (name)}<button type="button" class="sw" style:--c={c} aria-label={name} title={name} disabled></button>{/each}
  </div>
</section>

<style>
  section { display: grid; gap: var(--s-3); }
  section + section { margin-top: var(--s-5); }
  .seg { display: flex; gap: var(--s-2); flex-wrap: wrap; }
  .seg button, .kind { min-height: var(--pill-h); padding: 0 var(--s-5); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.07); font-size: var(--text-ui); color: var(--ink); transition: scale var(--dur-fast) var(--ease-out); }
  .seg button:active:not(:disabled), .kind:active:not(:disabled) { scale: var(--press-scale); }
  .seg button[aria-pressed="true"], .kind[aria-pressed="true"] { background: var(--accent-soft); color: var(--accent); font-weight: var(--strong-weight); }
  .seg button[aria-disabled="true"] { opacity: 0.45; }
  .seg button:disabled, .kind:disabled { opacity: 0.5; }
  .kinds { display: flex; gap: var(--s-2); flex-wrap: wrap; }
  .kind { display: inline-grid; place-items: center; padding: 0; width: calc(var(--pill-h) * 1.15); }
  .swatches { display: flex; gap: var(--s-3); }
  .sw { width: var(--s-8); height: var(--s-8); border-radius: 50%; background: var(--c); box-shadow: inset 0 0 0 1px rgb(var(--ink-rgb) / 0.15); opacity: 0.45; }
</style>

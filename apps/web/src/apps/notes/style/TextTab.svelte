<script lang="ts">
  // Text: the font (each shown in itself), the note's text size (only this note's words) and the tree lines. Ported from the existing app's components/style/TextTab.svelte (origin/dev): the same three
  // sections in the same order; fonts go through our roles (system is the editor's chat font, mono the code role, rounded the rounded face) and "Serif" has no role here (docs/notes-style.md).
  import { Icon, IconButton, Selector } from "@calternal/ui";
  import { noteStyle } from "./store.svelte";
  const style = $derived(noteStyle.page);
  const FONTS = [
    ["system", "Aa", "System", "var(--font-chat)"],
    ["rounded", "Rr", "Rounded", "var(--font-rounded)"],
    ["mono", "00", "Mono", "var(--font-mono)"],
  ] as const;
  const SIZES = [{ value: "small", label: "Small" }, { value: "default", label: "Default" }, { value: "large", label: "Large" }];
  const LINES = [{ value: "dotted", label: "Dotted" }, { value: "connected", label: "Connected" }];
  const key = (k: string, v: string | null) => noteStyle.set({ [k]: v });
</script>

<section aria-labelledby="st-text">
  <h3 class="section-label" id="st-text">Font</h3>
  <Selector label="Font" class="fonts" options={FONTS.map(([id, , label]) => ({ value: id, label }))} value={style.font} onchange={(f) => key("calternal-font", f === "system" ? null : f)}>
    {#snippet optionContent(o)}
      {@const f = FONTS.find(([id]) => id === o.value)!}
      <span class="glyph" style:font-family={f[3]} style:font-variation-settings={f[0] === "rounded" ? '"ROND" 100' : undefined}>{f[1]}</span>
      <span class="name">{f[2]}</span>
    {/snippet}
  </Selector>
</section>
<section aria-labelledby="st-size">
  <h3 class="section-label" id="st-size">Text size</h3>
  <Selector label="Text size" options={SIZES} value={style.size} onchange={(z) => key("calternal-text-size", z === "default" ? null : z)} />
</section>
<section aria-labelledby="st-lines">
  <h3 class="section-label head" id="st-lines">
    Tree lines
    <IconButton icon="restore" label="Reset tree lines" onclick={() => key("calternal-lines", null)} />
  </h3>
  <Selector label="Tree lines" options={LINES} value={style.lines} onchange={(l) => key("calternal-lines", l)} />
</section>

<style>
  section { display: grid; grid-template-columns: minmax(0, 1fr); gap: var(--s-3); }
  section + section { margin-top: var(--s-5); }
  section :global(.fonts) { --selector-option-direction: column; --selector-option-gap: var(--s-1); }
  .head { display: flex; align-items: center; justify-content: space-between; }
  .glyph { font-size: var(--text-heading); font-weight: var(--strong-weight); }
  .name { font-size: var(--text-label); color: var(--ink-3); }
</style>

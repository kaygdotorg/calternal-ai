<script lang="ts">
  // One setting as a row: the label on the left with its help text under it, one control on the right edge. The control is chosen by the setting's data
  // (registry.ts), by the rules of docs/ui-craft-and-settings-study.md 11.2. Changes apply when made: there is no Save.
  import { Button, Icon, InlineEditor, Menu, Pill, Selector, StepSlider, Switch } from "@calternal/ui";
  import PresetGallery from "../../appearance/PresetGallery.svelte";
  import Slider from "../../appearance/Slider.svelte";
  import type { Setting } from "./registry";
  let { s, highlighted = false }: { s: Setting; highlighted?: boolean } = $props();
  const c = $derived(s.control);
  const value = $derived(s.get?.());
  let menu = $state(false);
  let trigger = $state<HTMLButtonElement>();
  let draft = $state("");
  $effect(() => { if (c.type === "text" || c.type === "number") draft = String(value ?? ""); });
</script>

{#if c.type === "custom"}
  <div class="row col" class:hl={highlighted} data-setting={s.id}>{#await c.load() then m}{@const C = m.default}<C />{/await}</div>
{:else if c.type === "switch"}
  <div class="row" class:hl={highlighted} data-setting={s.id}><Switch label={s.label} hint={s.help} checked={!!value} onchange={(v) => s.set?.(v)} /></div>
{:else if c.type === "presets"}
  <div class="row col" class:hl={highlighted} data-setting={s.id}>
    <div class="l"><span class="t">{s.label}</span>{#if s.help}<span class="h">{s.help}</span>{/if}</div>
    <PresetGallery />
  </div>
{:else if c.type === "slider"}
  <div class="row col" class:hl={highlighted} data-setting={s.id}>
    <Slider label={s.label} unit={c.unit ?? ""} min={c.min} max={c.max} value={Number(value)} onchange={(v) => s.set?.(v)} />
    {#if s.help}<span class="h">{s.help}</span>{/if}
  </div>
{:else if c.type === "steps"}
  <div class="row col" class:hl={highlighted} data-setting={s.id}>
    <StepSlider label={s.label} steps={c.steps} value={Number(value)} onchange={(i) => s.set?.(i)} hint={s.help} />
  </div>
{:else if c.type === "checkbox"}
  <div class="row" class:hl={highlighted} data-setting={s.id}><Switch label={s.label} hint={s.help} checked={!!value} onchange={(v) => s.set?.(v)} /></div>
{:else}
  <div class="row" class:hl={highlighted} data-setting={s.id}>
    <span class="l"><span class="t">{s.label}</span>{#if s.help}<span class="h">{s.help}</span>{/if}</span>
    <span class="ctl">
      {#if c.type === "scheme"}
        <Selector label={s.label} fill={false} options={[{ value: "system", label: "Auto" }, { value: "light", label: "Light" }, { value: "dark", label: "Dark" }]} value={String(value)} onchange={(v) => s.set?.(v)} />
      {:else if c.type === "segmented"}
        <Selector label={s.label} fill={false} options={c.options.map((o) => ({ value: o.id, label: o.label }))} value={String(value)} onchange={(v) => s.set?.(v)} />
      {:else if c.type === "popup"}
        <Pill label={s.label} showLabel glass={false} aria-haspopup="menu" aria-expanded={menu} bind:el={trigger} onclick={() => (menu = !menu)}><span>{c.options.find((o) => o.id === value)?.label ?? String(value)}</span><Icon inline name="chevron" /></Pill>
        <Menu open={menu} items={c.options.map((o) => ({ id: o.id, label: o.label, checked: o.id === value }))} label={s.label} {trigger} onclose={() => (menu = false)} onpick={(id) => s.set?.(id)} />
      {:else if c.type === "text"}
        <InlineEditor class="field" label={s.label} placeholder={c.placeholder ?? ""} bind:value={draft} onsubmit={() => s.set?.(draft)} onblur={() => draft !== String(value ?? "") && s.set?.(draft)} />
      {:else if c.type === "number"}
        <span class="num"><input class="field" type="number" aria-label={s.label} min={c.min} max={c.max} bind:value={draft} onchange={() => s.set?.(Math.min(c.max, Math.max(c.min, Number(draft) || c.min)))} />{#if c.unit}<span class="h">{c.unit}</span>{/if}</span>
      {:else if c.type === "button"}
        <Button text={c.text} variant={c.danger ? "secondary" : "secondary"} onclick={c.run} />
      {:else if c.type === "drill"}
        <button type="button" class="drill" onclick={c.run} aria-label="{s.label}: {c.summary()}"><span class="h">{c.summary()}</span><Icon inline name="chevron" /></button>
      {:else if c.type === "info"}
        <span class="h">{c.text()}</span>
      {/if}
    </span>
  </div>
{/if}

<style>
  .row > :global(.sw) { width: 100%; }
  .row { min-width: 0; display: flex; align-items: center; justify-content: space-between; gap: var(--s-6); min-height: calc(var(--row-h-settings)); padding: var(--s-3) var(--s-6); position: relative; transition: background-color var(--dur-slow) var(--ease-out); }
  .row.col { display: grid; gap: var(--s-3); justify-content: stretch; padding-block: var(--s-5); }
  .row.col:has(> :global(ul)) { padding: 0; }
  .row.hl { background: var(--accent-soft); }
  .l { display: grid; min-width: 0; gap: var(--s-1); }
  .t { font-size: var(--text-ui); font-weight: var(--strong-weight); }
  .h { font-size: var(--text-label); color: var(--ink-3); max-width: 38rem; }
  .ctl { flex: none; display: inline-flex; align-items: center; justify-content: flex-end; min-width: 10rem; }
  /* A phone: the control goes under the words and takes the row's width, so nothing runs past the card (found in real Safari). */
  @media (max-width: 600px) {
    .row { flex-direction: column; align-items: stretch; gap: var(--s-3); }
    .ctl { min-width: 0; justify-content: flex-start; flex-wrap: wrap; }
    .ctl :global(.cap) { width: 100%; max-width: 100%; }
    .ctl :global(.cap .seg) { flex: 1 1 auto; min-width: 0; padding-inline: var(--s-3); font-size: var(--text-label); }
    :global(.row .field) { width: 100%; text-align: start; }
    .h { max-width: none; }
  }
  .num { display: inline-flex; align-items: center; gap: var(--s-3); }
  :global(.row .field) { min-height: var(--pill-h); padding: 0 var(--s-5); border: 0; border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.07); color: var(--ink); text-align: end; width: 10rem; }
  .drill { display: inline-flex; align-items: center; gap: var(--s-2); min-height: var(--pill-h); }
  /* A hairline between rows, inset from the label's edge. */
  :global(.sg > .row + .row)::before, :global(.sg > .row + .row)::before { content: ""; position: absolute; top: 0; left: var(--s-6); right: 0; height: 1px; background: var(--line); }
</style>

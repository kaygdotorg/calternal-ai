<script lang="ts">
  import { arr } from "../arrange.svelte";
  // The mockup-only tweak panel: the shell design's open points as switches, so the owner can flip an assumption while looking. It is not part of any app and nothing in it is a product feature.
  import { IconButton, Panel, Selector, Switch } from "@calternal/ui";
  import { tweaks, forced, panel } from "../tweaks.svelte";
  import { notesUi } from "../apps/notes/ui.svelte";
  panel.open ||= forced.tweaks;
  const open = $derived(panel.open);
  const t = $derived(tweaks.t);
</script>

<div class="tw" data-tweaks>
  {#if open}
    <Panel radius="card" class="twp"><div class="in">
      <header class="hd"><h2>Mockup tweaks</h2><IconButton icon="close" label="Close the tweak panel" onclick={() => (panel.open = false)} /></header>
      <p class="d">Only here to look at alternatives. Not part of the app.</p>
      <div class="r"><span>Wordmark</span><Selector options={[{ value: "dancing", label: "Dancing" }, { value: "caveat", label: "Caveat" }, { value: "satisfy", label: "Satisfy" }]} value={t.wordmark} label="Wordmark cursive" onchange={(v: string) => tweaks.set("wordmark", v as never)} /></div>
      <div class="r"><span>App name (what the wordmark morphs into)</span><Selector options={[{ value: "ui", label: "Interface face" }, { value: "cursive", label: "Cursive too" }]} value={t.appName} label="App name face" onchange={(v: string) => tweaks.set("appName", v as never)} /></div>
      <div class="r"><span>Scheme</span><Selector options={[{ value: "system", label: "Auto" }, { value: "light", label: "Light" }, { value: "dark", label: "Dark" }]} value={t.scheme} label="Scheme" onchange={(v: string) => tweaks.set("scheme", v as never)} /></div>
      <Switch label="Mac window" checked={t.mac} onchange={(v: boolean) => tweaks.set("mac", v)} />
      <div class="r"><span>Big list density</span><Selector options={[{ value: "compact", label: "One line" }, { value: "comfortable", label: "Two lines" }]} value={t.density} label="Density" class="equal" onchange={(v: string) => tweaks.set("density", v as never)} /></div>
      <div class="r"><span>Top line names</span><Selector options={[{ value: "smart", label: "Smart" }, { value: "sidebar", label: "Sidebar" }]} value={t.topLine} label="Top line" class="equal" onchange={(v: string) => tweaks.set("topLine", v as never)} /></div>
      <Switch label="Notes: reading text size" checked={notesUi.reading} onchange={(v: boolean) => notesUi.set(v)} />
      <p class="d">Taste options: the first is today's, the second is the review's recommendation.</p>
      <div class="r"><span>Item name</span><Selector options={[{ value: "all", label: "Everywhere" }, { value: "once", label: "Pill and heading" }]} value={t.nameOnce ? "once" : "all"} label="Item name" class="equal" onchange={(v: string) => tweaks.set("nameOnce", v === "once")} /></div>
      <div class="r"><span>Selected row tint</span><Selector options={[{ value: "accent", label: "Accent" }, { value: "neutral", label: "Neutral" }]} value={t.selTint} label="Selected tint" class="equal" onchange={(v: string) => tweaks.set("selTint", v as never)} /></div>
      <div class="r"><span>Gallery captions</span><Selector options={[{ value: "left", label: "Left" }, { value: "center", label: "Centred" }]} value={t.caps} label="Captions" class="equal" onchange={(v: string) => tweaks.set("caps", v as never)} /></div>
      <Switch label="Auto-hide the app switcher" checked={t.switcherHide} onchange={(v: boolean) => tweaks.set("switcherHide", v)} />
      <div class="r"><span>Window background</span><Selector options={[{ value: "frosted", label: "Frosted" }, { value: "opaque", label: "Opaque" }]} value={t.winBg} label="Window background" class="equal" onchange={(v: string) => tweaks.set("winBg", v as never)} /></div>
      <div class="r"><span>Width of a lone window</span><Selector options={[{ value: "640", label: "640" }, { value: "800", label: "800" }, { value: "960", label: "960" }, { value: "fill", label: "Fill" }]} value={t.loneW} label="Lone window width" class="equal" onchange={(v: string) => tweaks.set("loneW", v as never)} /></div>
      <div class="r"><span>Your role in the space</span><Selector options={[{ value: "Owner", label: "Owner" }, { value: "Manager", label: "Manager" }, { value: "Editor", label: "Editor" }, { value: "Reader", label: "Reader" }]} value={t.role} label="Your role" class="equal" onchange={(v: string) => tweaks.set("role", v as never)} /></div>
      <div class="r"><span>Collection marks</span><Selector options={[{ value: "dots", label: "Colour dots" }, { value: "icons", label: "Icons" }]} value={t.collMarks} label="Collection marks" class="equal" onchange={(v: string) => tweaks.set("collMarks", v as never)} /></div>
      <div class="r"><span>Profile popover</span><Selector options={[{ value: "plain", label: "As is" }, { value: "dense", label: "Dense, searchable" }]} value={t.profile} label="Profile popover" class="equal" onchange={(v: string) => tweaks.set("profile", v as never)} /></div>
      <div class="r"><span>Apps order</span><button type="button" class="b" onclick={() => tweaks.resetOrder()}>Back to frecency, unpin all</button></div>
      <div class="r"><span>Sidebars</span><button type="button" class="b" onclick={() => arr.reset()}>Reset every sidebar and the custom one</button></div>
    </div></Panel>
  {/if}
</div>

<style>
  :global(.twp) { background: color-mix(in srgb, var(--paper, #fff) 94%, transparent); }
  .tw { position: fixed; right: var(--s-4); bottom: var(--s-4); z-index: 40; } .hd { display: flex; align-items: center; justify-content: space-between; }
  :global(.twp) { width: min(22rem, calc(100vw - 2 * var(--s-4))); } .in { display: grid; gap: var(--s-4); padding: var(--s-5); } h2 { margin: 0; font-size: var(--text-heading); font-weight: var(--strong-weight); } .d { margin: 0; color: var(--ink-3); font-size: var(--text-label); }
  .r { display: grid; gap: var(--s-2); } .r > span { color: var(--ink-3); font-size: var(--text-label); } .b { justify-self: start; padding: var(--s-2) var(--s-4); border-radius: var(--r-select); background: var(--hover); }
  @media (max-width: 719px) { .tw { left: var(--s-4); bottom: var(--s-4); } }
</style>

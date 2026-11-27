<script lang="ts">
  // Settings, Appearance (the part that exists in the mockup): the window's surface, and the app switcher. Each row is the setting's name, its one-line help, and its control; the same values are in the mockup's tweak panel. SIZES from native-ui-study.md 2.3 (Settings page): rows 44 pt high, groups 10 pt apart, group cards 14 pt radius, 1 pt dividers, cards about 460 pt wide.
  import { Selector } from "@calternal/ui";
  import { tweaks } from "../tweaks.svelte";
  const t = $derived(tweaks.t);
</script>

<div class="st" data-settings>
  <h2>Appearance</h2>
  <section aria-labelledby="g1"><h3 id="g1">Windows</h3>
    <div class="grp">
      <div class="row"><div class="tx"><b>Window background</b><small>Opaque is solid. Frosted lets the background show through, blurred.</small></div><div class="ctl"><Selector fill={false} options={[{ value: "opaque", label: "Opaque" }, { value: "frosted", label: "Frosted" }]} value={t.winBg} label="Window background" onchange={(v: string) => tweaks.set("winBg", v as never)} /></div></div>
      <div class="row"><div class="tx"><b>Width of a lone window</b><small>A window that is alone on the canvas is centred at this width.</small></div><div class="ctl"><Selector fill={false} options={[{ value: "640", label: "640" }, { value: "800", label: "800" }, { value: "960", label: "960" }, { value: "fill", label: "Fill" }]} value={t.loneW} label="Lone window width" onchange={(v: string) => tweaks.set("loneW", v as never)} /></div></div>
    </div>
  </section>
  <section aria-labelledby="g2"><h3 id="g2">App switcher</h3>
    <div class="grp">
      <div class="row"><div class="tx"><b>Hide when not in use</b><small>Moves out of sight; the pointer at the bottom of the window, or Alt+Shift+A, brings it back.</small></div><div class="ctl"><Selector fill={false} options={[{ value: "off", label: "Always" }, { value: "on", label: "Hide" }]} value={t.switcherHide ? "on" : "off"} label="Hide the app switcher until needed" onchange={(v: string) => tweaks.set("switcherHide", v === "on")} /></div></div>
      <div class="row"><div class="tx"><b>Shows</b><small>Automatic is text on a computer and icons on a phone.</small></div><div class="ctl"><Selector fill={false} options={[{ value: "auto", label: "Auto" }, { value: "text", label: "Text" }, { value: "icons", label: "Icons" }, { value: "both", label: "Both" }]} value={t.switcherLabels} label="Switcher items" onchange={(v: string) => tweaks.set("switcherLabels", v as never)} /></div></div>
      <div class="row"><div class="tx"><b>Apps at once</b><small>The strip scrolls for the rest.</small></div><div class="ctl"><Selector fill={false} options={[{ value: "3", label: "3" }, { value: "4", label: "4" }, { value: "5", label: "5" }]} value={String(t.switcherCount)} label="Apps at once" onchange={(v: string) => tweaks.set("switcherCount", Number(v) as never)} /></div></div>
    </div>
  </section>
</div>

<style>
  .st { display: grid; gap: var(--s-6); align-content: start; padding: var(--s-7) var(--s-7) var(--s-9); max-width: 34rem; } h2 { margin: 0; font-size: var(--text-title); font-weight: var(--strong-weight); } h3 { margin: 0 0 var(--gap-head-items); font-size: var(--text-label); font-weight: var(--strong-weight); color: var(--ink-2); }
  .grp { border-radius: var(--r-menu); background: rgb(var(--ink-rgb) / 0.05); overflow: hidden; } .row { display: flex; align-items: center; gap: var(--s-6); min-height: var(--row-h-settings); padding: var(--s-4) var(--s-6); } .row + .row { box-shadow: inset 0 1px 0 rgb(var(--ink-rgb) / 0.08); }
  .ctl { flex: none; } .tx { flex: 1; min-width: 0; display: grid; gap: 1px; } .tx b { font-weight: var(--ui-weight); font-size: var(--text-ui); } .tx small { color: var(--ink-2); font-size: var(--text-label); line-height: 1.3; }
</style>

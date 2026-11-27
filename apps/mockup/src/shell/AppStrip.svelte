<script lang="ts">
  import { AppStrip as SharedAppStrip } from "@calternal/canvas-shell";
  import { APPS } from "../data/apps";
  import { arr } from "../arrange.svelte";
  import { tweaks } from "../tweaks.svelte";
  import { strip } from "../strip.svelte";

  let { current, onselect, narrow = false, shown = true, onfocusin, onfocusout }: {
    current: string | null; onselect: (id: string | null) => void; narrow?: boolean; shown?: boolean; onfocusin?: () => void; onfocusout?: () => void;
  } = $props();

  const items = $derived(
    strip.ids().map((id) => {
      const a = APPS.find((x) => x.id === id);
      return a ? { id, name: a.name, icon: a.icon } : { id, name: arr.custom.find((c) => c.id === id)?.name ?? id, icon: "sidebar" };
    }),
  );
</script>

<SharedAppStrip
  {items}
  {current}
  {onselect}
  onplace={(id, index) => strip.place(id, index)}
  onmove={(id, by) => strip.move(id, by)}
  onadd={(name) => arr.addSidebar(name)}
  {narrow}
  {shown}
  labelsMode={tweaks.t.switcherLabels}
  maxCount={tweaks.t.switcherCount}
  {onfocusin}
  {onfocusout}
/>

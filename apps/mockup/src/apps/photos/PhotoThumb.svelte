<script lang="ts">
  // A photo's thumbnail: its generated scene, cropped to the box it is given (the box sets the shape), with a duration chip for a video and a small mark for a favourite. Decoration, hidden from screen readers.
  import AppIcon from "../../shell/AppIcon.svelte";
  import Scene from "./Scene.svelte";
  import { META } from "./data";
  import { ps } from "./state.svelte";
  let { id, marks = true }: { id: string; marks?: boolean } = $props();
  const m = $derived(META[id]!);
  const dur = $derived(m.video ? `${Math.floor(m.video / 60)}:${String(m.video % 60).padStart(2, "0")}` : "");
</script>

<span class="pt" aria-hidden="true">
  <Scene spec={m.spec} />
  {#if marks && m.video}<span class="chip"><AppIcon name="play" inline />{dur}</span>{/if}
  {#if marks && ps.isFav(id)}<span class="fv"><AppIcon name="spark" inline /></span>{/if}
</span>

<style>
  .pt { position: relative; display: block; width: 100%; height: 100%; overflow: hidden; }
  .chip, .fv { position: absolute; display: inline-flex; align-items: center; gap: var(--s-1); color: #fff; background: rgb(0 0 0 / 0.5); font-size: var(--text-section); font-variant-numeric: tabular-nums; line-height: 1; }
  .chip { inset-inline-end: var(--s-2); inset-block-end: var(--s-2); padding: var(--s-1) var(--s-3) var(--s-1) var(--s-2); border-radius: var(--r-pill); } .fv { inset-inline-start: var(--s-2); inset-block-end: var(--s-2); padding: var(--s-2); border-radius: 50%; }
</style>

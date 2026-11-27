<script lang="ts">
  // One attachment. A picture is a tile (a stand-in tint, as the fixtures have no photos); anything else is a one-line chip with its kind's glyph, name and size. `onremove` adds the remove button (compose).
  import AppIcon from "../../shell/AppIcon.svelte";
  import { IconButton } from "@calternal/ui";
  import type { Att } from "./data";
  let { att, onremove, flat = false }: { att: Att; onremove?: () => void; /** Always the one-line chip (the right panel lists, it does not show). */ flat?: boolean } = $props();
  const GLYPH = { pdf: "file", image: "image", calendar: "calendar", sheet: "table", zip: "archive", doc: "notes" } as const;
  const hue = $derived([...att.name].reduce((h, c) => (h * 31 + c.charCodeAt(0)) % 360, 11));
</script>

{#if att.type === "image" && !flat}
  <span class="tile" title={`${att.name}, ${att.size}`}><span class="pic" style:--hue={hue}><AppIcon name="image" /></span><span class="cap"><span class="n">{att.name}</span><span class="z">{att.size}</span></span>{#if onremove}<span class="x"><IconButton small icon="close" label={`Remove ${att.name}`} onclick={onremove} /></span>{/if}</span>
{:else}
  <span class="chip" title={`${att.name}, ${att.size}`}><span class="g"><AppIcon name={GLYPH[att.type]} inline /></span><span class="n">{att.name}</span><span class="z">{att.size}</span>{#if onremove}<IconButton small icon="close" label={`Remove ${att.name}`} onclick={onremove} />{/if}</span>
{/if}

<style>
  .chip { display: inline-flex; align-items: center; gap: var(--s-3); max-width: 100%; box-sizing: border-box; min-height: 32px; padding: 0 var(--s-4) 0 var(--s-4); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.07); font-size: var(--text-label); } .g { display: grid; color: var(--ink-2); }
  .n { flex: 0 1 auto; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } .z { flex: none; color: var(--ink-3); font-variant-numeric: tabular-nums; }
  .tile { position: relative; display: inline-grid; gap: var(--s-2); width: 7.5rem; font-size: var(--text-label); } .pic { display: grid; place-items: center; aspect-ratio: 4 / 3; border-radius: var(--r-menu); background: oklch(0.78 0.08 var(--hue) / 0.55); color: var(--ink-2); }
  .cap { display: grid; min-width: 0; } .cap .z { font-size: var(--text-section); } .x { position: absolute; top: var(--s-2); right: var(--s-2); border-radius: 50%; background: rgb(var(--tint) / 0.85); }
</style>

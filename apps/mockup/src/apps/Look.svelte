<script lang="ts">
  // How a file and a photo look wherever a collection draws them (the seam's `looks`, for kinds `file` and `photo`): a row (a 24 px tile, the name, when), a strip tile (a 44 px picture and a caption), a card (a picture, a name), a cell (a picture in a 4:3 box and a caption).
  // The picture is the thing itself, never a stock icon: a file's own thumbnail (`files/FileThumb`), a photo's scene, an album's cover, a library list's icon. Sizes and states match `shell/Item.svelte` (row height, press, hover, selected), so a mixed collection reads as one list.
  import AppIcon from "../shell/AppIcon.svelte";
  import Scene from "./photos/Scene.svelte";
  import FileThumb from "./files/FileThumb.svelte";
  import { META as PM, ALBUMS, albumPhotos } from "./photos/data";
  import { META as FM } from "./files/data";
  import { typeOf } from "./files/types";
  import type { LookProps } from "./registry";
  let { item, mode, big = false, two = false, selected = false, onopen }: LookProps = $props();
  /** An item that only a collection made up (no file or photo record behind it) still gets its own thumbnail: a file a type tile from its name, a photo a scene seeded by its id, so no tile is ever empty. */
  const seed = (s: string) => [...s].reduce((a, c) => (a * 31 + c.charCodeAt(0)) >>> 0, 7);
  const ensure = (i: typeof item) => { if (i.kind === "file" && !FM[i.id] && !PM[i.id] && !i.folder && !i.icon) FM[i.id] = { id: i.id, name: i.title, parent: null, folder: false, bytes: 120000, modified: i.date ?? "2026-11-20", time: "09:00", type: typeOf(i.title), by: "Ada", versions: 1 }; };
  const fm = $derived((ensure(item), FM[item.id])), pm = $derived(PM[item.id]);
  const album = $derived(ALBUMS.find((a) => a.id === item.id));
  const spec = $derived(pm?.spec ?? (album ? albumPhotos(album)[0]?.spec : item.kind === "photo" && !item.icon ? { kind: (["hills", "coast", "city", "dunes", "blobs", "table"] as const)[seed(item.id) % 6]!, hue: item.hue ?? seed(item.id) % 360, seed: seed(item.id), time: "day" as const } : undefined));
  const icon = $derived(item.icon && !fm ? item.icon : undefined);
  const sub = $derived(fm ? (fm.folder ? item.sub : `${fm.type.label} · ${item.sub}`) : (item.sub ?? ""));
  const open = () => onopen?.(item);
</script>

{#snippet pic(size: "icon" | "face")}
  {#if fm}<FileThumb id={item.id} {size} />
  {:else if icon}<span class="ico"><AppIcon name={icon} /></span>
  {:else if spec}<span class="sc"><Scene {spec} /></span>{/if}
{/snippet}

{#if mode === "row"}
  <span class="irow" class:big class:two={big && two} class:sel={selected} data-kind={item.kind} data-item={item.id}>
    <button type="button" class="ib" aria-current={selected ? "true" : undefined} onclick={open}>
      <span class="lead">{@render pic("icon")}</span>
      <span class="txt"><span class="t" title={item.title}>{item.title}</span>{#if sub && big && !two}<span class="s1" title={sub}>{sub}</span>{/if}{#if big && two && sub}<span class="s">{sub}</span>{/if}</span>
      <span class="tr">{#if item.when}<span class="tm">{item.when}</span>{/if}{#if item.count}<span class="tm">{item.count.toLocaleString("en")}</span>{/if}</span>
    </button>
  </span>
{:else if mode === "face"}
  <span class="fill" data-kind={item.kind}>{@render pic("face")}</span>
{:else if mode === "strip"}
  <button type="button" class="tile" class:sel={selected} aria-label={item.title} aria-current={selected ? "true" : undefined} data-kind={item.kind} data-item={item.id} onclick={open}>
    <span class="sq">{@render pic("face")}</span>{#if album || icon || fm?.folder}<span class="cap">{item.title}</span>{/if}
  </button>
{:else if mode === "card"}
  <button type="button" class="icard" data-kind={item.kind} data-item={item.id} onclick={open}>
    <span class="th">{@render pic("face")}</span><strong>{item.title}</strong><span class="s">{sub}</span>
  </button>
{:else}
  <button type="button" class="cell" data-kind={item.kind} data-item={item.id} onclick={open}>
    <span class="face">{@render pic("face")}</span><span class="cp"><strong>{item.title}</strong><span class="s">{sub}</span></span>
  </button>
{/if}

<style>
  button { text-align: start; color: var(--ink); }
  .irow { position: relative; display: flex; align-items: center; width: 100%; border-radius: var(--r-select); transition: background-color var(--dur-fast) var(--ease-out); }
  .ib { flex: 1; min-width: 0; display: flex; align-items: center; gap: var(--s-4); min-height: var(--row-h); padding: 0 var(--s-5); border-radius: var(--r-select); font-size: var(--text-ui); line-height: 1.25; transition: scale var(--dur-fast) var(--ease-out); } .irow.two .ib { min-height: var(--row-h-settings); } .ib:active { scale: var(--press-scale); }
  @media (hover: hover) and (pointer: fine) { .irow:not(.sel):hover { background: var(--hover); } } .irow.sel { background: var(--accent-soft); }
  .lead { flex: none; display: grid; place-items: center; width: 22px; height: 22px; } .lead :global(.sc), .lead .ico { width: 22px; height: 22px; border-radius: var(--r-select); } .ico { display: grid; place-items: center; color: var(--ink-2); } .irow.sel .ico { color: var(--accent); } .lead .sc { display: block; overflow: hidden; }
  .txt { flex: 1; min-width: 0; display: flex; align-items: baseline; gap: var(--s-3); } .irow.two .txt { display: grid; gap: 0; } .s1 { flex: 1 1 0; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--ink-3); font-size: var(--text-label); } .t, .s { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } .t { font-weight: var(--ui-weight); } .s { color: var(--ink-3); font-size: var(--text-label); font-weight: var(--ui-weight); }
  .tr { flex: none; display: inline-flex; align-items: center; gap: var(--s-3); } .tm { color: var(--ink-3); font-size: var(--text-label); font-variant-numeric: tabular-nums; }
  .tile { flex: none; display: grid; justify-items: center; align-content: start; gap: var(--s-2); width: 56px; border-radius: var(--r-menu); transition: scale var(--dur-fast) var(--ease-out); } .tile:active { scale: var(--press-scale); }
  .sq { display: block; width: 44px; height: 44px; border-radius: var(--r-menu); overflow: hidden; } .tile.sel .sq { box-shadow: 0 0 0 2px var(--accent); } .sq .ico { width: 100%; height: 100%; background: rgb(var(--ink-rgb) / 0.08); }
  .cap { max-width: 56px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: var(--text-section); line-height: 1.2; color: var(--ink-3); text-align: center; }
  .icard { display: grid; gap: var(--s-3); width: 100%; padding: var(--s-4); border-radius: var(--r-menu); background: rgb(var(--ink-rgb) / 0.06); box-shadow: 0 1px 2px rgb(0 0 0 / 0.08); font-size: var(--text-ui); line-height: 1.3; transition: scale var(--dur-fast) var(--ease-out); } .icard:active { scale: var(--press-scale); } .icard strong, .cp strong { font-weight: var(--strong-weight); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } .icard .s { color: var(--ink-3); font-size: var(--text-label); }
  .th { display: block; aspect-ratio: 4 / 3; overflow: hidden; border-radius: var(--r-select); } .th .ico, .face .ico { width: 100%; height: 100%; background: rgb(var(--ink-rgb) / 0.06); }
  .cell { display: grid; gap: var(--s-3); width: 100%; align-content: start; transition: scale var(--dur-fast) var(--ease-out); } .cell:active { scale: var(--press-scale); } .face { display: block; aspect-ratio: 4 / 3; overflow: hidden; border-radius: var(--r-menu); } .cp { display: grid; min-width: 0; }
  /* a picture fills its frame exactly: the frame is positioned and clips, the picture lies in it from edge to edge (a percentage height inside a grid cell of unknown height is no height at all) */
  .th, .face, .fill { position: relative; overflow: hidden; } .th > :global(*), .face > :global(*), .fill > :global(*) { position: absolute; inset: 0; width: 100%; height: 100%; }
  .fill { display: block; position: absolute; inset: 0; width: 100%; height: 100%; } .fill .ico, .fill .sc { width: 100%; height: 100%; }
  .sc { display: block; width: 100%; height: 100%; } .ico { display: grid; place-items: center; }
</style>

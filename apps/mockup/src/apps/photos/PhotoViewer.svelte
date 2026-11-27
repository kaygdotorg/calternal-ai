<script lang="ts">
  // One photo, large: the picture on the page's quiet backdrop, the arrows beside it, and a strip of thumbnails under it (the list it was opened from, the open one ringed). Details live in the side panel, not here.
  // Keys: Left and Right step, Escape goes back. Stepping crossfades the picture over `--dur-base` on `--ease-out` (a second step mid-way restarts from where it is: the old picture is simply replaced); the strip follows the open thumbnail
  // (instantly under reduced motion). The arrows show on hover or focus on a desktop, always on touch. Used in the Photos window (where `win.sel` is the photo) and as the page of a photo opened in a window of its own.
  import { IconButton } from "@calternal/ui";
  import Scene from "./Scene.svelte";
  import AppIcon from "../../shell/AppIcon.svelte";
  import { PHOTOS, albumOf, albumPhotos, dayLabel, listOf, metaOf } from "./data";
  import { ps } from "./state.svelte";
  import { ITEMS, type Item } from "../../data/collections";
  import type { Win } from "../../windows.svelte";
  let { item, win, onback }: { item: Item; win?: Win; onback?: () => void } = $props();
  let local = $state<string | null>(null);
  const cur = $derived(win ? item.id : (local ?? item.id));
  const ctx = $derived(win ? (ps.from[win.id] ?? "pl-all") : "pl-all");
  const list = $derived(albumOf(ctx) ? albumPhotos(albumOf(ctx)!) : PHOTOS.filter((listOf(ctx) ?? listOf("pl-all")!).pick));
  const i = $derived(Math.max(0, list.findIndex((p) => p.id === cur)));
  const m = $derived(metaOf(cur)!);
  const name = $derived(ITEMS[cur]!.title);
  const where = $derived(albumOf(ctx)?.title ?? listOf(ctx)?.title ?? "Library");
  const go = (to: number) => { const p = list[Math.max(0, Math.min(list.length - 1, to))]; if (!p || p.id === cur) return; if (win) win.sel = p.id; else local = p.id; };
  function key(e: KeyboardEvent) {
    if ((e.target as HTMLElement | null)?.closest("input, textarea")) return;
    if (e.key === "ArrowLeft") { e.preventDefault(); go(i - 1); } else if (e.key === "ArrowRight") { e.preventDefault(); go(i + 1); } else if (e.key === "Escape" && onback) onback();
  }
  let strip = $state<HTMLElement>();
  $effect(() => {
    void cur; const el = strip?.querySelector<HTMLElement>("[aria-current='true']"); if (!el || !strip) return;
    const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
    strip.scrollTo({ left: el.offsetLeft - strip.clientWidth / 2 + el.clientWidth / 2, behavior: reduce ? "auto" : "smooth" });
  });
</script>

<svelte:window onkeydown={key} />
<div class="pv" data-photo-viewer={cur}>
  <header class="bar">
    {#if onback}<button type="button" class="back" onclick={onback}><AppIcon name="back" inline /><span>{where}</span></button>{/if}
    <div class="id"><strong>{name}</strong><span>{dayLabel(m.date) === "Today" || dayLabel(m.date) === "Yesterday" ? `${dayLabel(m.date)}, ${m.time}` : `${dayLabel(m.date)} · ${m.time}`}</span></div>
    <span class="sp"></span>
    <IconButton icon="spark" label={ps.isFav(cur) ? "Remove from favourites" : "Add to favourites"} pressed={ps.isFav(cur)} onclick={() => ps.toggle(cur)} />
    <IconButton icon="share" label="Share" onclick={() => {}} />
  </header>
  <div class="stage" style:--ar={m.w / m.h}>
    {#key cur}<div class="pic" role="img" aria-label={`${name}, ${m.place}`}><Scene spec={m.spec} />{#if m.video}<span class="play"><AppIcon name="play" /></span>{/if}</div>{/key}
    {#if i > 0}<span class="nav l"><IconButton icon="back" label="Previous photo" onclick={() => go(i - 1)} /></span>{/if}
    {#if i < list.length - 1}<span class="nav r"><IconButton icon="chevron" label="Next photo" onclick={() => go(i + 1)} /></span>{/if}
  </div>
  <p class="cnt" aria-live="polite">{i + 1} of {list.length}</p>
  <div class="strip" bind:this={strip} role="listbox" aria-label="Photos in this list">
    {#each list as p (p.id)}<button type="button" class="th" role="option" aria-selected={p.id === cur} aria-current={p.id === cur ? "true" : undefined} aria-label={ITEMS[p.id]!.title} onclick={() => go(list.indexOf(p))}><Scene spec={p.spec} /></button>{/each}
  </div>
</div>

<style>
  .pv { flex: 1; min-height: 0; display: flex; flex-direction: column; gap: var(--s-4); }
  .bar { flex: none; display: flex; align-items: center; gap: var(--s-4); } .sp { flex: 1; }
  .back { display: inline-flex; align-items: center; gap: var(--s-2); min-height: 32px; padding: 0 var(--s-4) 0 var(--s-3); border-radius: var(--r-pill); color: var(--accent); font-size: var(--text-ui); transition: background-color var(--dur-fast) var(--ease-out), scale var(--dur-fast) var(--ease-out); } .back:active { scale: var(--press-scale); } @media (hover: hover) and (pointer: fine) { .back:hover { background: var(--accent-soft); } }
  .id { display: grid; min-width: 0; line-height: 1.25; } .id strong { font-size: var(--text-ui); font-weight: var(--strong-weight); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } .id span { color: var(--ink-3); font-size: var(--text-label); }
  .stage { position: relative; flex: 1; min-height: 8rem; container-type: size; display: grid; place-items: center; border-radius: var(--r-menu); background: rgb(var(--ink-rgb) / 0.05); }
  .pic { position: relative; width: min(100cqw, calc(100cqh * var(--ar))); aspect-ratio: var(--ar); overflow: hidden; border-radius: var(--r-select); box-shadow: 0 1px 2px rgb(0 0 0 / 0.12), 0 8px 28px rgb(0 0 0 / 0.14); animation: pin var(--dur-base) var(--ease-out); } @keyframes pin { from { opacity: 0; } }
  .play { position: absolute; inset: 0; display: grid; place-items: center; color: #fff; } .play :global(svg) { width: 3rem; height: 3rem; padding: var(--s-5); border-radius: 50%; background: rgb(0 0 0 / 0.5); box-sizing: border-box; }
  .nav :global(button) { color: #fff; } .nav { position: absolute; top: 50%; translate: 0 -50%; border-radius: 50%; background: rgb(0 0 0 / 0.45); color: #fff; } .nav.l { inset-inline-start: var(--s-4); } .nav.r { inset-inline-end: var(--s-4); }
  @media (hover: hover) and (pointer: fine) { .nav { opacity: 0; transition: opacity var(--dur-fast) var(--ease-out); } .stage:hover .nav, .nav:focus-within { opacity: 1; } }
  .nav.r :global(svg) { } .cnt { margin: 0; text-align: center; color: var(--ink-3); font-size: var(--text-label); font-variant-numeric: tabular-nums; }
  .strip { flex: none; display: flex; gap: var(--s-2); overflow-x: auto; padding: var(--s-2) var(--s-1) var(--s-3); scrollbar-width: none; }
  .back { white-space: nowrap; flex: none; } .pv { container-type: inline-size; }
  @container (max-width: 26rem) { .back span { display: none; } .back { padding: 0 var(--s-3); } .id span { display: none; } }
  .th { flex: none; width: 52px; height: 52px; padding: 0; overflow: hidden; border-radius: var(--r-select); opacity: 0.7; outline: 2px solid transparent; outline-offset: 1px; transition: opacity var(--dur-fast) var(--ease-out), outline-color var(--dur-fast) var(--ease-out), scale var(--dur-fast) var(--ease-out); } .th:active { scale: var(--press-scale); }
  .th[aria-current="true"] { opacity: 1; outline-color: var(--accent); } @media (hover: hover) and (pointer: fine) { .th:hover { opacity: 1; } }
</style>

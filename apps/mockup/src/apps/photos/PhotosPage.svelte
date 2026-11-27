<script lang="ts">
  // The Photos window's page. What the window's `sel` names decides what it shows: nothing or a library list is the timeline (days in a dense grid, newest first, or months); an album is its own page (cover, title, dates, the grid); a photo is the viewer.
  // The page is its own scroller. Cells are square and 2 px apart (the dense grid the owner asked for); a cell is one button, its name is the photo's name and day. Opening a photo remembers the list it came from, so the viewer's strip and Back use it.
  // Narrow windows (a phone, or a split) use a container query, not the viewport: the grid keeps at least three across.
  import { Selector, Button } from "@calternal/ui";
  import AppIcon from "../../shell/AppIcon.svelte";
  import PhotoThumb from "./PhotoThumb.svelte";
  import PhotoViewer from "./PhotoViewer.svelte";
  import { ALBUMS, PHOTOS, albumOf, albumPhotos, byDay, dayLabel, listOf, metaOf, shortDate, type PhotoMeta } from "./data";
  import { ps } from "./state.svelte";
  import { ITEMS, type Item } from "../../data/collections";
  import type { PageProps } from "../registry";
  let { win, onopen }: PageProps = $props();
  const sel = $derived(win.sel);
  const photo = $derived(sel && metaOf(sel) ? ITEMS[sel]! : undefined);
  const album = $derived(sel ? albumOf(sel) : undefined);
  const lst = $derived(sel ? listOf(sel) : undefined);
  const ctx = $derived(album?.id ?? lst?.id ?? "pl-all");
  const photos = $derived<PhotoMeta[]>(album ? albumPhotos(album) : PHOTOS.filter((lst ?? listOf("pl-all")!).pick));
  let zoom = $state<"days" | "months">("days"), pw = $state(0);
  // Whole-pixel cells and a 2 px gutter, so no seam is ever half a pixel: the column count follows the page width, the cell is what is left divided evenly, rounded down.
  const GAP = 2, dense = $derived((cols: number) => Math.floor((pw - GAP * (cols - 1)) / cols));
  const cols = $derived(Math.max(zoom === "months" && !album ? 4 : 3, Math.floor((pw + GAP) / ((zoom === "months" && !album ? 76 : 120) + GAP))));
  const title = $derived(album?.title ?? lst?.title ?? "Library");
  const range = $derived(photos.length ? `${shortDate(photos[photos.length - 1]!.date)} to ${shortDate(photos[0]!.date)}` : "");
  const groups = $derived.by(() => {
    if (zoom === "days") return byDay(photos).map((g) => ({ key: g.date, label: dayLabel(g.date), place: [...new Set(g.photos.map((p) => p.city))].slice(0, 2).join(" · "), photos: g.photos }));
    const out: { key: string; label: string; place: string; photos: PhotoMeta[] }[] = [];
    for (const p of photos) { const k = p.date.slice(0, 7), g = out[out.length - 1]; if (g && g.key === k) g.photos.push(p); else out.push({ key: k, label: new Date(`${k}-15T12:00:00Z`).toLocaleDateString("en-GB", { month: "long", year: "numeric", timeZone: "UTC" }), place: "", photos: [p] }); }
    return out;
  });
  const open = (m: PhotoMeta) => { ps.from[win.id] = ctx; onopen(ITEMS[m.id] as Item); };
  const back = () => { win.sel = ps.from[win.id] && ps.from[win.id] !== "pl-all" ? ps.from[win.id]! : null; };
  const goto = (id: string) => { win.sel = id === "pl-all" ? null : id; };
</script>

{#if photo}
  <PhotoViewer item={photo} {win} onback={back} />
{:else}
  <div class="pg" data-photos-page={album ? "album" : "timeline"} bind:clientWidth={pw} style:--cols={cols} style:--cell="{dense(cols)}px">
    <header class="top" class:alb={!!album}>
      {#if album}
        <div class="cover"><PhotoThumb id={photos[0]!.id} marks={false} /></div>
        <div class="tt"><p class="k">Album</p><h1>{album.title}</h1><p class="m">{photos.length} photos · {range}</p></div>
        <div class="act"><Button text="Share" icon="share" variant="secondary" /></div>
      {:else}
        <div class="tt"><h1>{title}</h1><p class="m">{photos.length} {photos.length === 1 ? "item" : "items"}{range ? ` · ${range}` : ""}</p></div>
        <div class="act"><Selector options={[{ value: "days", label: "Days" }, { value: "months", label: "Months" }]} value={zoom} label="Group by" size="sm" fill={false} onchange={(v: string) => (zoom = v as typeof zoom)} /></div>
      {/if}
    </header>
    {#if album}<div class="chips" role="group" aria-label="Albums">
      <button type="button" class="chip" onclick={() => goto("pl-all")}>Library</button>
      {#each ALBUMS as a (a.id)}<button type="button" class="chip" class:on={a.id === album.id} aria-current={a.id === album.id ? "true" : undefined} onclick={() => goto(a.id)}>{a.title}</button>{/each}
    </div>{/if}
    {#if !photos.length}
      <div class="empty"><AppIcon name="image" /><h2>No photos here yet</h2><p>Photos you favourite will appear in this list.</p></div>
    {:else}
      {#each groups as g (g.key)}
        <section class="day" data-day={g.key} aria-label={g.label}>
          <h2>{g.label}{#if g.place}<span>{g.place}</span>{/if}<em>{g.photos.length}</em></h2>
          <div class="grid">
            {#each g.photos as m (m.id)}<button type="button" class="cell" aria-label={`${ITEMS[m.id]!.title}, ${dayLabel(m.date)}, ${m.time}`} data-photo={m.id} onclick={() => open(m)}><PhotoThumb id={m.id} /></button>{/each}
          </div>
        </section>
      {/each}
    {/if}
  </div>
{/if}

<style>
  .pg { flex: 1; min-height: 0; overflow-y: auto; container-type: inline-size; padding-bottom: var(--s-8); scrollbar-width: thin; }
  .top { display: flex; align-items: flex-end; justify-content: space-between; gap: var(--s-5); padding: var(--s-2) 0 var(--s-5); } .top.alb { align-items: center; padding-bottom: var(--s-5); } .tt { flex: 1; min-width: 0; display: grid; gap: var(--s-1); }
  h1 { margin: 0; font-size: var(--text-title); line-height: 1.2; font-weight: var(--strong-weight); } .m, .k { margin: 0; color: var(--ink-3); font-size: var(--text-label); line-height: 1.3; font-variant-numeric: tabular-nums; } .k { text-transform: uppercase; letter-spacing: 0.05em; font-size: var(--text-section); }
  .cover { flex: none; width: 72px; aspect-ratio: 1; overflow: hidden; border-radius: var(--r-menu); box-shadow: 0 1px 2px rgb(0 0 0 / 0.12), 0 4px 14px rgb(0 0 0 / 0.08); }
  .chips { display: flex; gap: var(--s-3); overflow-x: auto; padding-bottom: var(--s-5); scrollbar-width: none; } .chip { flex: none; padding: var(--s-2) var(--s-5); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.07); color: var(--ink-2); font-size: var(--text-label); line-height: 1.5; transition: background-color var(--dur-fast) var(--ease-out), scale var(--dur-fast) var(--ease-out); } .chip:active { scale: var(--press-scale); }
  @media (hover: hover) and (pointer: fine) { .chip:not(.on):hover { background: var(--hover); } } .chip.on { background: var(--accent-soft); color: var(--accent); font-weight: var(--strong-weight); }
  .day { padding-bottom: var(--gap-before-head); } h2 { display: flex; align-items: baseline; gap: var(--s-4); margin: 0; padding: 0 0 var(--gap-head-items); font-size: var(--text-ui); line-height: 1.25; font-weight: var(--strong-weight); } h2 span { color: var(--ink-3); font-weight: var(--ui-weight); font-size: var(--text-label); } h2 em { margin-inline-start: auto; color: var(--ink-3); font-style: normal; font-weight: var(--ui-weight); font-size: var(--text-label); font-variant-numeric: tabular-nums; }
  .grid { display: grid; grid-template-columns: repeat(var(--cols), var(--cell)); gap: var(--s-1); }
  .cell { display: block; aspect-ratio: 1; padding: 0; overflow: hidden; border-radius: var(--r-select); transition: scale var(--dur-fast) var(--ease-out); } .cell:active { scale: var(--press-scale); }
  @container (max-width: 30rem) { .cover { width: 56px; } .alb .act { display: none; } }
  .empty { display: grid; justify-items: center; gap: var(--s-3); padding: var(--s-10) var(--s-6); color: var(--ink-3); text-align: center; } .empty h2 { padding: 0; color: var(--ink); } .empty p { margin: 0; font-size: var(--text-ui); }
</style>

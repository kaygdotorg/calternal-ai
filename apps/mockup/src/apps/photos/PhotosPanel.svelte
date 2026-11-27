<script lang="ts">
  // The Photos side panel (the seam's `panel`): for a photo its date, place (a small generated map), camera, people and albums; for an album its dates and who it is shared with; for the library its size and backup. Details and Assistant tabs as the shared panel. Fixture only.
  import { Switch } from "@calternal/ui";
  import AppIcon from "../../shell/AppIcon.svelte";
  import Av from "../../shell/Av.svelte";
  import PanelFrame from "../../shell/PanelFrame.svelte";
  import "../files/panel.css";
  import { ALBUMS, PHOTOS, albumOf, albumPhotos, bytesText, listOf, longDate, metaOf, shortDate } from "./data";
  import type { Win } from "../../windows.svelte";
  import type { Item } from "../../data/collections";
  let { win, item }: { win: Win | undefined; item?: Item } = $props();
  const m = $derived(item ? metaOf(item.id) : undefined);
  const album = $derived(item ? albumOf(item.id) : undefined);
  const list = $derived(item ? listOf(item.id) : undefined);
  const albumList = $derived(album ? albumPhotos(album) : []);
  const range = $derived(albumList.length ? `${shortDate(albumList[albumList.length - 1]!.date)} to ${shortDate(albumList[0]!.date)}` : "");
  const total = PHOTOS.reduce((s, p) => s + p.bytes, 0);
  const people = ["Ada", "Ben", "Lena", "Hana", "Ivo"].map((n) => [n, PHOTOS.filter((p) => p.people.includes(n)).length] as const);
  let keep = $state(false);
  const title = $derived(m ? item!.title : (album?.title ?? list?.title ?? "Photos"));
  const sub = $derived(m ? longDate(m.date, m.time) : album ? `${albumList.length} photos · ${range}` : `${(list ? PHOTOS.filter(list.pick) : PHOTOS).length} items`);
</script>

<PanelFrame {title} {sub}>
  {#if m}
    <section class="pn-sec"><h3>Details</h3><dl>
      <dt>Taken</dt><dd>{longDate(m.date, m.time)}</dd><dt>Camera</dt><dd>{m.camera}</dd><dt>Lens</dt><dd title={m.lens}>{m.lens}</dd><dt>Exposure</dt><dd title={m.exposure}>{m.exposure}</dd>
      <dt>Size</dt><dd>{bytesText(m.bytes)}</dd><dt>Dimensions</dt><dd>{m.w} × {m.h}</dd>{#if m.video}<dt>Length</dt><dd>0:{String(m.video).padStart(2, "0")}</dd>{/if}
    </dl></section>
    <section class="pn-sec"><h3>Place</h3>
      <div class="map" role="img" aria-label={`Map of ${m.place}, ${m.city}`}><svg viewBox="0 0 160 80" preserveAspectRatio="xMidYMid slice"><rect width="160" height="80" class="land" /><path d="M0 52 C30 40 50 66 90 50 S140 40 160 46 V80 H0Z" class="water" /><path d="M20 0 L52 80 M70 0 L96 80 M0 22 L160 30" class="road" /><circle cx="84" cy="36" r="5" class="pin" /></svg></div>
      <dl class="loc"><dt>{m.place}</dt><dd>{m.city}</dd></dl>
      <button type="button" class="row-btn"><span class="ic"><AppIcon name="external" inline /></span>Open in Maps</button>
    </section>
    {#if m.people.length}<section class="pn-sec"><h3>People</h3><ul>{#each m.people as p (p)}<li><Av name={p} size="24px" /><span class="pn">{p}</span></li>{/each}</ul></section>{/if}
    {#if m.albums.length}<section class="pn-sec"><h3>Albums</h3><div class="pn-chips">{#each m.albums as t (t)}<span class="chip">{ALBUMS.find((a) => a.tag === t)?.title ?? t}</span>{/each}</div></section>{/if}
    <section class="pn-sec"><h3>On this device</h3><Switch label="Keep downloaded" bind:checked={keep} /><p class="note kn">Stays available without a connection until you turn it off.</p></section>
  {:else if album}
    <section class="pn-sec"><h3>Album</h3><dl><dt>Photos</dt><dd>{albumList.length}</dd><dt>Dates</dt><dd>{range}</dd><dt>Size</dt><dd>{bytesText(albumList.reduce((s, p) => s + p.bytes, 0))}</dd></dl>{#if album.note}<p class="note nt">{album.note}</p>{/if}</section>
    <section class="pn-sec"><h3>Shared with</h3>{#if album.sharedWith.length}<ul>{#each album.sharedWith as p, i (p)}<li><Av name={p} size="24px" /><span class="pn">{p}</span><span class="rl">{i === 0 ? "Can add" : "Can view"}</span></li>{/each}</ul>{:else}<p class="note">Only you.</p>{/if}
      <button type="button" class="row-btn"><span class="ic"><AppIcon name="share" inline /></span>Share this album</button></section>
  {:else}
    <section class="pn-sec"><h3>Library</h3><dl><dt>Photos</dt><dd>{PHOTOS.filter((p) => !p.video).length.toLocaleString("en")}</dd><dt>Videos</dt><dd>{PHOTOS.filter((p) => p.video).length}</dd><dt>Size</dt><dd>{bytesText(total)}</dd><dt>Backup</dt><dd>Up to date</dd></dl></section>
    <section class="pn-sec"><h3>People</h3><ul>{#each people as [n, c] (n)}<li><Av name={n} size="24px" /><span class="pn">{n}</span><span class="rl">{c}</span></li>{/each}</ul></section>
    <section class="pn-sec"><h3>Storage</h3><div class="pn-bar" role="img" aria-label="Photos use 12 percent of the shared quota"><i style="width:12%;background:var(--accent-base)"></i></div><p class="note">Photos use {bytesText(total)} of the 1 TB shared with every app.</p></section>
  {/if}
</PanelFrame>

<style>
  .map { overflow: hidden; border-radius: var(--r-menu); aspect-ratio: 2 / 1; margin-bottom: var(--s-4); background: rgb(var(--ink-rgb) / 0.06); } .map svg { display: block; width: 100%; height: 100%; }
  .land { fill: oklch(0.93 0.03 120); } .water { fill: oklch(0.82 0.06 230); } .road { fill: none; stroke: oklch(0.99 0 0); stroke-width: 3; stroke-linecap: round; } .pin { fill: var(--accent-base); stroke: #fff; stroke-width: 2; }
  :global(html[data-scheme="dark"]) .land { fill: oklch(0.32 0.03 120); } :global(html[data-scheme="dark"]) .water { fill: oklch(0.38 0.06 230); } :global(html[data-scheme="dark"]) .road { stroke: oklch(0.48 0.01 250); }
  .loc { margin-bottom: var(--s-3); } .nt { margin-top: var(--s-4); }
</style>

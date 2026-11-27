<script lang="ts">
  // The Files side panel (the seam's `panel`): for a file or folder its picture, details, who can open it (people, and the link with its expiry), versions and whether it is kept downloaded; with nothing selected the space used (one quota shared with every app) and what you share.
  // The space bar is one rail split by app, the legend under it; the numbers are fixture. Details and Assistant tabs as the shared panel. Fixture only.
  import { Switch } from "@calternal/ui";
  import AppIcon from "../../shell/AppIcon.svelte";
  import Av from "../../shell/Av.svelte";
  import PanelFrame from "../../shell/PanelFrame.svelte";
  import FileThumb from "./FileThumb.svelte";
  import "./panel.css";
  import { META, fullWhen, kids, metaOf, sizeOfFolder, sizeText, trail } from "./data";
  import { bytesText } from "../photos/data";
  import type { Win } from "../../windows.svelte";
  import type { Item } from "../../data/collections";
  let { win, item }: { win: Win | undefined; item?: Item } = $props();
  const m = $derived(item ? metaOf(item.id) : (win?.sel ? metaOf(win.sel) : undefined));
  const cur = $derived(m ?? metaOf("fo-home")!);
  let keep = $state(false);
  $effect.pre(() => { keep = !!cur.keep; void cur.id; });
  const where = $derived(trail(cur.id).slice(1, -1).map((t) => t.name).join(" / ") || "Home");
  const people = $derived(cur.share?.with ?? []);
  const USED: { k: string; gb: number; c: number }[] = [{ k: "Photos", gb: 301, c: 330 }, { k: "Files", gb: 28, c: 235 }, { k: "Mail", gb: 31, c: 205 }, { k: "Versions", gb: 22, c: 55 }, { k: "Trash", gb: 6, c: 25 }, { k: "Notes", gb: 0.4, c: 150 }];
  const used = USED.reduce((s, u) => s + u.gb, 0), QUOTA = 1000;
  const mine = Object.values(META).filter((x) => x.share && !x.owner);
  const size = $derived(cur.folder ? (kids(cur.id).length ? bytesText(sizeOfFolder(cur.id)) : "Empty") : bytesText(cur.bytes));
  const exp = (d: string) => new Date(`${d}T12:00:00Z`).toLocaleDateString("en-GB", { day: "numeric", month: "long", timeZone: "UTC" });
</script>

<PanelFrame title={m ? cur.name : "Files"} sub={m ? `${cur.type.label} · ${cur.folder ? sizeText(cur) : size}` : `${used.toFixed(0)} GB of 1 TB used`}>
  {#if m}
    <section class="pn-sec"><div class="pre"><FileThumb id={cur.id} /></div></section>
    <section class="pn-sec"><h3>Details</h3><dl>
      <dt>Kind</dt><dd>{cur.type.label}</dd><dt>Size</dt><dd>{size}</dd>{#if cur.folder}<dt>Contains</dt><dd>{kids(cur.id).length} items</dd>{/if}<dt>Modified</dt><dd title={fullWhen(cur)}>{fullWhen(cur)}</dd><dt>By</dt><dd>{cur.owner ?? cur.by}</dd><dt>Where</dt><dd title={where}>{where}</dd>{#if cur.pages}<dt>Pages</dt><dd>{cur.pages}</dd>{/if}{#if !cur.folder}<dt>Versions</dt><dd>{cur.versions}</dd>{/if}
    </dl></section>
    <section class="pn-sec"><h3>Sharing</h3>
      {#if people.length}<ul>{#each people as p, i (p)}<li><Av name={p} size="24px" /><span class="pn">{p}</span><span class="rl">{cur.owner ? (p === cur.owner ? "Owner" : "Can view") : i === 0 ? "Can edit" : "Can view"}</span></li>{/each}</ul>{:else}<p class="note">Only you can open this.</p>{/if}
      {#if cur.share?.link}<p class="lnk"><AppIcon name="link" inline /><span>Anyone with the link can {cur.share.link.can}{cur.share.link.password ? ", with a password" : ""}{cur.share.link.expires ? `. Expires ${exp(cur.share.link.expires)}` : ""}</span></p>{/if}
      <button type="button" class="row-btn"><span class="ic"><AppIcon name="link" inline /></span>Copy link</button>
      <button type="button" class="row-btn"><span class="ic"><AppIcon name="share" inline /></span>Share…</button>
    </section>
    <section class="pn-sec"><h3>On this device</h3><Switch label="Keep downloaded" bind:checked={keep} /><p class="note kn">Stays available without a connection until you turn it off.</p></section>
  {:else}
    <section class="pn-sec"><h3>Storage</h3>
      <div class="pn-bar" role="img" aria-label={`${used.toFixed(0)} GB of 1 TB used`}>{#each USED as u (u.k)}<i style:width="{(u.gb / QUOTA) * 100}%" style:background="oklch(0.68 0.12 {u.c})"></i>{/each}</div>
      <ul class="pn-key">{#each USED as u (u.k)}<li><span class="sw" style:background="oklch(0.68 0.12 {u.c})"></span><span class="pn">{u.k}</span><span class="rl">{u.gb < 1 ? "400 MB" : `${u.gb} GB`}</span></li>{/each}</ul>
      <p class="note">One quota of 1 TB, shared by every app. Versions and the trash count; nothing is removed to make room unless you choose.</p></section>
    <section class="pn-sec"><h3>You share</h3><ul>{#each mine.slice(0, 6) as s (s.id)}<li><span class="ft"><AppIcon name={s.folder ? "folder" : "file"} inline /></span><span class="pn">{s.name}</span><span class="rl">{s.share!.with.length} {s.share!.with.length === 1 ? "person" : "people"}</span></li>{/each}</ul></section>
  {/if}
</PanelFrame>

<style>
  .pre { aspect-ratio: 4 / 3; overflow: hidden; border-radius: var(--r-menu); } .lnk { display: flex; gap: var(--s-4); margin: var(--s-4) 0; padding: var(--s-4) var(--s-5); border-radius: var(--r-menu); background: rgb(var(--ink-rgb) / 0.06); font-size: var(--text-label); line-height: 1.4; color: var(--ink-2); } .lnk :global(svg) { flex: none; margin-top: 1px; }
  .ft { display: grid; color: var(--ink-2); } section :global(.row-btn) + :global(.row-btn) { margin-top: 0; }
</style>

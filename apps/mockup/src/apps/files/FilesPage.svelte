<script lang="ts">
  // The Files window's page. What the window's `sel` names decides what it shows: nothing is Home (the curated root, with what was recent under it), a folder is its contents, a file is its preview. Folders are one tree you walk in the middle with a breadcrumb; there is no second pane for it.
  // The list has the columns the Files grill decided (Name, Modified, Size, Kind), sortable by clicking a head (folders always first); a narrow window drops Kind, then Size, then (at phone width) keeps Name and Modified. Rows are one line. The gallery draws each file as a picture of itself.
  // The container queries below follow the window's width, not the screen's. A drag over the page shows where it would upload; the fixture does nothing with the drop.
  import { Selector, Button } from "@calternal/ui";
  import AppIcon from "../../shell/AppIcon.svelte";
  import FileThumb from "./FileThumb.svelte";
  import FilePreview from "./FilePreview.svelte";
  import { CHILDREN, META, kids, metaOf, sizeOfFolder, sizeText, trail, whenText, type FileMeta } from "./data";
  import { bytesText } from "../photos/data";
  import { ITEM } from "./data";
  import type { PageProps } from "../registry";
  let { win, onopen }: PageProps = $props();
  const HOME = "fo-home";
  const cur = $derived(metaOf(win.sel ?? HOME) ?? metaOf(HOME)!);
  type Key = "name" | "modified" | "size" | "kind";
  let view = $state<"list" | "gallery">("list"), key = $state<Key>("name"), asc = $state(true), over = $state(false);
  const sorted = $derived.by(() => {
    const dir = asc ? 1 : -1, f = (m: FileMeta) => m;
    return kids(cur.id).map(f).sort((a, b) => {
      if (a.folder !== b.folder) return a.folder ? -1 : 1;
      const c = key === "name" ? a.name.localeCompare(b.name, "en", { numeric: true }) : key === "modified" ? (a.modified + a.time).localeCompare(b.modified + b.time) : key === "size" ? (a.folder ? kids(a.id).length - kids(b.id).length : a.bytes - b.bytes) : a.type.label.localeCompare(b.type.label);
      return c * dir || a.name.localeCompare(b.name);
    });
  });
  const recent = $derived(cur.id === HOME ? Object.values(META).filter((m) => !m.folder && !m.owner).sort((a, b) => (b.modified + b.time).localeCompare(a.modified + a.time)).slice(0, 5) : []);
  const total = $derived(sizeOfFolder(cur.id));
  const sortBy = (k: Key) => { if (key === k) asc = !asc; else { key = k; asc = true; } };
  const go = (m: FileMeta) => (m.id === HOME ? (win.sel = null) : (win.sel = m.id));
  const open = (m: FileMeta) => onopen(ITEM[m.id]!);
  const crumbs = $derived(trail(cur.id));
  function drag(e: DragEvent, on: boolean) { if (e.dataTransfer?.types.includes("Files") || on === false) { e.preventDefault(); over = on; } }
  const HEADS: { k: Key; label: string }[] = [{ k: "name", label: "Name" }, { k: "modified", label: "Modified" }, { k: "size", label: "Size" }, { k: "kind", label: "Kind" }];
</script>

{#if cur.folder === false}
  <FilePreview m={cur} onback={() => go(metaOf(cur.parent!)!)} />
{:else}
  <div class="pg" class:over data-files-page="folder" role="region" aria-label={`${cur.name}, a folder`} ondragover={(e) => drag(e, true)} ondragleave={(e) => { if (e.currentTarget === e.target) drag(e, false); }} ondrop={(e) => { e.preventDefault(); over = false; }}>
    <header class="top">
      <nav class="crumbs" aria-label="Folder path">
        {#each crumbs as c, i (c.id)}{#if i > 0}<span class="sep" aria-hidden="true"><AppIcon name="chevron" inline /></span>{/if}{#if i < crumbs.length - 1}<button type="button" onclick={() => go(c)}>{c.name}</button>{:else}<span aria-current="page">{c.name}</span>{/if}{/each}
      </nav>
      <div class="act">
        <Selector options={[{ value: "list", label: "List" }, { value: "gallery", label: "Gallery" }]} value={view} label="View" size="sm" fill={false} onchange={(v: string) => (view = v as typeof view)} />
        <Button text="Upload" icon="plus" variant="secondary" />
      </div>
    </header>
    <div class="tt"><h1>{cur.name}</h1><p class="m">{sorted.length ? `${sorted.length} item${sorted.length === 1 ? "" : "s"} · ${bytesText(total)}` : "Empty"}{cur.share ? ` · Shared with ${cur.share.with.length}` : ""}</p></div>
    {#key `${cur.id}/${view}`}<div class="in">
    {#if !sorted.length}
      <div class="empty" data-empty><div class="ef"><FileThumb id={cur.id} /></div><h2>This folder is empty</h2><p>Drop files here, or upload them. Anything you add shows up on your other devices.</p><Button text="Upload files" icon="plus" variant="primary" /></div>
    {:else if view === "list"}
      <div class="lst" role="table" aria-label={`Contents of ${cur.name}`}>
        <div class="hd" role="row">{#each HEADS as h (h.k)}<button type="button" class={`c-${h.k}`} role="columnheader" aria-sort={key === h.k ? (asc ? "ascending" : "descending") : "none"} onclick={() => sortBy(h.k)}>{h.label}{#if key === h.k}<span class="ar" class:up={!asc}><AppIcon name="chevron-down" inline /></span>{/if}</button>{/each}</div>
        {#each sorted as m (m.id)}{@render row(m)}{/each}
      </div>
      {#if recent.length}<h3 class="sec">Recent</h3><div class="lst" role="table" aria-label="Recent files">{#each recent as m (m.id)}{@render row(m)}{/each}</div>{/if}
    {:else}
      <div class="gal">{#each sorted as m (m.id)}
        <button type="button" class="cell" data-file={m.id} onclick={() => open(m)}><span class="face"><FileThumb id={m.id} /></span><span class="cp"><strong>{m.name}</strong><span class="s">{m.folder ? sizeText(m) : `${m.type.label} · ${sizeText(m)}`}</span></span></button>{/each}
      </div>
      {#if recent.length}<h3 class="sec">Recent</h3><div class="gal">{#each recent as m (m.id)}<button type="button" class="cell" data-file={m.id} onclick={() => open(m)}><span class="face"><FileThumb id={m.id} /></span><span class="cp"><strong>{m.name}</strong><span class="s">{m.type.label} · {sizeText(m)}</span></span></button>{/each}</div>{/if}
    {/if}
    </div>{/key}
    {#if over}<div class="drop" aria-hidden="true"><AppIcon name="plus" /><strong>Drop to upload to {cur.name}</strong></div>{/if}
  </div>
{/if}

{#snippet row(m: FileMeta)}
  <button type="button" class="row" role="row" data-file={m.id} onclick={() => open(m)}>
    <span class="c-name"><FileThumb id={m.id} size="icon" /><span class="nm">{m.name}</span>{#if m.share}<span class="bd" title="Shared"><AppIcon name="share" inline /></span>{/if}{#if m.keep}<span class="bd" title="Kept downloaded"><AppIcon name="down" inline /></span>{/if}</span>
    <span class="c-modified">{whenText(m.modified, m.time)}</span><span class="c-size">{m.folder ? sizeText(m) : bytesText(m.bytes)}</span><span class="c-kind">{m.type.label}</span>
  </button>
{/snippet}

<style>
  .pg { position: relative; flex: 1; min-height: 0; overflow-y: auto; container-type: inline-size; padding-bottom: var(--s-8); scrollbar-width: thin; }
  /* a phone: the title first (what you are looking at), then the path and the controls; the Upload label weighs the same as the List selector beside it */
  @media (max-width: 719px) { .pg { display: flex; flex-direction: column; } .pg > .tt { order: -1; } }
  .act :global(.btn), .act :global(button) { font-weight: var(--ui-weight); }
  .top { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: var(--s-5); padding: var(--s-1) 0 var(--s-3); min-height: var(--pill-h, 38px); } .act { flex: none; display: flex; align-items: center; gap: var(--s-4); }
  .crumbs { display: flex; align-items: center; min-width: 0; font-size: var(--text-label); color: var(--ink-3); white-space: nowrap; overflow: hidden; } .crumbs button, .crumbs span[aria-current] { padding: var(--s-1) var(--s-2); border-radius: var(--r-select); } .crumbs span[aria-current] { color: var(--ink-2); overflow: hidden; text-overflow: ellipsis; }
  .crumbs button { transition: background-color var(--dur-fast) var(--ease-out), color var(--dur-fast) var(--ease-out); } @media (hover: hover) and (pointer: fine) { .crumbs button:hover { background: var(--hover); color: var(--ink); } } .sep { display: grid; }
  .tt { display: grid; gap: var(--s-1); padding-bottom: var(--s-5); } h1 { margin: 0; font-size: var(--text-title); line-height: 1.2; font-weight: var(--strong-weight); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } .m { margin: 0; color: var(--ink-3); font-size: var(--text-label); line-height: 1.3; font-variant-numeric: tabular-nums; }
  .in { animation: fin var(--dur-base) var(--ease-out); } @keyframes fin { from { opacity: 0; translate: 0 6px; } }
  .lst { display: grid; } .hd, .row { display: grid; grid-template-columns: minmax(0, 1fr) 8.5rem 6rem 9rem; align-items: center; column-gap: var(--s-5); } .hd { padding: 0 var(--s-5); border-bottom: 1px solid rgb(var(--ink-rgb) / 0.08); }
  .hd button { display: inline-flex; align-items: center; gap: var(--s-2); min-height: 28px; color: var(--ink-3); font-size: var(--text-label); font-weight: var(--ui-weight); text-align: start; } .hd button[aria-sort="ascending"], .hd button[aria-sort="descending"] { color: var(--ink); } .ar { display: grid; transition: rotate var(--dur-fast) var(--ease-out); } .ar.up { rotate: 180deg; }
  .row { width: 100%; min-height: var(--row-h); padding: 0 var(--s-5); border-radius: var(--r-select); font-size: var(--text-ui); line-height: 1.25; text-align: start; color: var(--ink); transition: background-color var(--dur-fast) var(--ease-out), scale var(--dur-fast) var(--ease-out); } .row:active { scale: var(--press-scale); } @media (hover: hover) and (pointer: fine) { .row:hover { background: var(--hover); } }
  .c-name { display: flex; align-items: center; gap: var(--s-4); min-width: 0; } .nm { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } .bd { flex: none; display: grid; color: var(--ink-3); }
  .c-modified, .c-size, .c-kind { color: var(--ink-3); font-size: var(--text-label); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; font-variant-numeric: tabular-nums; } .hd .c-size, .row .c-size { text-align: end; justify-content: flex-end; } .hd .c-size { display: inline-flex; }
  .sec { margin: var(--gap-before-head) 0 var(--gap-head-items); padding: 0 var(--s-5); color: var(--ink-2); font-size: var(--text-heading); line-height: 1.2; font-weight: var(--strong-weight); }
  .gal { display: grid; grid-template-columns: repeat(auto-fill, minmax(9.5rem, 1fr)); gap: var(--s-6) var(--s-5); align-items: start; }
  .cell { display: grid; gap: var(--s-3); width: 100%; align-content: start; text-align: start; color: var(--ink); transition: scale var(--dur-fast) var(--ease-out); } .cell:active { scale: var(--press-scale); }
  .face { display: block; aspect-ratio: 4 / 3; overflow: hidden; border-radius: var(--r-menu); } .cp { display: grid; } .cp strong { font-weight: var(--strong-weight); font-size: var(--text-ui); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } .cp .s { color: var(--ink-3); font-size: var(--text-label); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .empty { display: grid; justify-items: center; gap: var(--s-3); padding: var(--s-9) var(--s-6); text-align: center; } .ef { width: 9rem; height: 7rem; } .ef :global(.fc) { background: none; } .empty h2 { margin: var(--s-3) 0 0; font-size: var(--text-heading); line-height: 1.2; font-weight: var(--strong-weight); } .empty p { margin: 0 0 var(--s-4); max-width: 24rem; color: var(--ink-3); font-size: var(--text-ui); line-height: 1.45; }
  .drop { position: sticky; bottom: var(--s-5); display: flex; align-items: center; justify-content: center; gap: var(--s-4); margin: var(--s-5) 0 0; min-height: 3.5rem; border: 2px dashed var(--accent); border-radius: var(--r-card); background: var(--accent-soft); color: var(--accent); font-size: var(--text-ui); pointer-events: none; animation: dr var(--dur-fast) var(--ease-out); } @keyframes dr { from { opacity: 0; } }
  @container (max-width: 44rem) { .hd, .row { grid-template-columns: minmax(0, 1fr) 8rem 5.5rem; } .c-kind, .hd .c-kind { display: none; } }
  @container (max-width: 30rem) { .hd, .row { grid-template-columns: minmax(0, 1fr) 5.5rem; } .c-size, .hd .c-size { display: none; } .gal { grid-template-columns: repeat(2, 1fr); }   }
</style>

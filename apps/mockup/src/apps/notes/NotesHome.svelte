<script lang="ts">
  // The Notes window when no note is open: pinned notes as a strip of thumbnails, then the recent ones in a view the person picks (gallery of thumbnails, one-line rows, cards), narrowed by folder. It is where the note's four looks (strip, cell, row, card) can be judged side by side.
  // MOTION. Changing view or folder: the old content goes at once and the new rises 6 px while fading in (`--dur-base`, `--ease-out`). Reduced motion: instant. Fixture only.
  import { Selector } from "@calternal/ui";
  import Empty from "../tasks/Empty.svelte";
  import Glyph from "../../shell/AppIcon.svelte";
  import NoteLook from "./NoteLook.svelte";
  import { DOCS, FOLDERS, notes } from "./data";
  import { ITEMS } from "../../data/collections";
  import { wins } from "../../windows.svelte";
  import type { PageProps } from "../registry";
  let { onopen }: PageProps = $props();
  let view = $state<"gallery" | "list" | "cards">("gallery"), folder = $state<string | null>(null);
  const VIEWS = [{ value: "gallery", label: "Gallery", iconOnly: true }, { value: "list", label: "List", iconOnly: true }, { value: "cards", label: "Cards", iconOnly: true }];
  const pinned = ["nt-lisbon", "nt-rewrite", "nt-dal"].map((i) => ITEMS[i]!);
  const shown = $derived(notes.filter((n) => !folder || DOCS[n.id]!.folder === folder));
  const sel = $derived(wins.selected);
</script>

<div class="nh" data-notes-home>
  <header class="hd"><h2>Notes</h2><Selector options={VIEWS} value={view} label="View" size="sm" fill={false} onchange={(v: typeof view) => (view = v)}>{#snippet optionIcon(o)}<Glyph name={o.value === "cards" ? "board" : o.value === "gallery" ? "gallery" : "list"} inline />{/snippet}</Selector></header>
  <div class="scroll">
    {#if !folder}
      <section aria-labelledby="pin-h"><h3 id="pin-h"><Glyph name="pin" inline />Pinned</h3>
        <div class="strip">{#each pinned as n (n.id)}<NoteLook item={n} mode="strip" selected={sel === n.id} onopen={onopen} />{/each}</div></section>
    {/if}
    <section aria-labelledby="rec-h"><h3 id="rec-h">{folder ?? "Recent"}<span>{shown.length}</span></h3>
      <div class="fl" role="group" aria-label="Folder"><button type="button" class="fc" class:on={!folder} aria-pressed={!folder} onclick={() => (folder = null)}>All</button>
        {#each FOLDERS as f (f)}<button type="button" class="fc" class:on={folder === f} aria-pressed={folder === f} onclick={() => (folder = folder === f ? null : f)}>{f}</button>{/each}</div>
      {#key `${view}${folder}`}
        <div class="v" data-view={view}>
          {#if shown.length === 0}<Empty icon="archive" title={`No notes in ${folder}`} text="Move a note here, or start a new one. Archived notes are kept out of Recent and search." action="New note" />
          {:else if view === "gallery"}<div class="gallery">{#each shown as n (n.id)}<NoteLook item={n} mode="cell" selected={sel === n.id} onopen={onopen} />{/each}</div>
          {:else if view === "cards"}<div class="cards">{#each shown as n (n.id)}<NoteLook item={n} mode="card" selected={sel === n.id} onopen={onopen} />{/each}</div>
          {:else}<div class="list">{#each shown as n (n.id)}<NoteLook item={n} selected={sel === n.id} onopen={onopen} />{/each}</div>{/if}
        </div>
      {/key}
    </section>
  </div>
</div>

<style>
  .nh { flex: none; width: 100%; display: flex; flex-direction: column; container-type: inline-size; }
  .hd { flex: none; display: flex; align-items: center; justify-content: space-between; gap: var(--s-4); padding: var(--s-2) 0 var(--s-4); } h2 { margin: 0; font-size: var(--text-title); font-weight: var(--strong-weight); line-height: 1.2; }
  .scroll { flex: none; /* the window scrolls the whole page */ display: grid; grid-template-columns: minmax(0, 1fr); align-content: start; gap: var(--s-8); padding-bottom: var(--s-9); }
  h3 { display: flex; align-items: center; gap: var(--s-3); margin: 0 0 var(--gap-head-items); color: var(--ink-2); font-size: var(--text-label); font-weight: var(--strong-weight); } h3 span { color: var(--ink-3); font-weight: var(--ui-weight); font-variant-numeric: tabular-nums; }
  .strip { --tile-w: 6rem; display: flex; gap: var(--s-6); overflow-x: auto; padding: var(--s-2) var(--s-2) var(--s-4); scrollbar-width: none; }
  .fl { display: flex; gap: var(--s-3); overflow-x: auto; scrollbar-width: none; padding-bottom: var(--s-5); }
  .fc { flex: none; height: var(--s-8); padding: 0 var(--s-5); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.06); color: var(--ink-2); font-size: var(--text-label); transition: background-color var(--dur-fast) var(--ease-out), color var(--dur-fast) var(--ease-out); }
  @media (hover: hover) and (pointer: fine) { .fc:not(.on):hover { background: rgb(var(--ink-rgb) / 0.1); } } .fc.on { background: var(--accent-soft); color: var(--accent); font-weight: var(--strong-weight); }
  .v { animation: vin var(--dur-base) var(--ease-out); } @keyframes vin { from { opacity: 0; translate: 0 6px; } }
  .gallery { display: grid; grid-template-columns: repeat(auto-fill, minmax(9rem, 1fr)); gap: var(--s-7) var(--s-6); align-items: start; padding: var(--s-2); } .cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(18rem, 1fr)); gap: var(--s-5); } .list { display: grid; }
</style>

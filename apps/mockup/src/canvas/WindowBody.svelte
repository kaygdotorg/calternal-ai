<script lang="ts">
  import SettingsPage from "./SettingsPage.svelte";
  // What a window shows: an app's screen (a stand-in, or the open item), a collection big (list, table, board, gallery or calendar, the collection's own default unless switched), a sidebar opened as a collection (its sections as a list of
  // collections, and its file), or one item. Choosing an item in a big view opens it through `onopen`. The views are in `views/`; the item looks in `shell/Item.svelte`.
  import { Selector, IconButton, Menu, type MenuItem } from "@calternal/ui";
  import Item from "../shell/Item.svelte";
  import ItemPage from "./ItemPage.svelte";
  import SidebarFile from "./SidebarFile.svelte";
  import BoardView from "./views/BoardView.svelte";
  import TableView from "./views/TableView.svelte";
  import CalendarView from "./views/CalendarView.svelte";
  import GalleryView from "./views/GalleryView.svelte";
  import Av from "../shell/Av.svelte";
  import { APPS, SETTINGS } from "../data/apps";
  import { ITEMS, type BigView, type Item as ItemT } from "../data/collections";
  import { arr } from "../arrange.svelte";
  import { forced, tweaks } from "../tweaks.svelte";
  import { pages, itemPages } from "../apps/registry";
  import type { Win } from "../windows.svelte";
  let { win, onopen, oncollection }: { win: Win; onopen: (i: ItemT) => void; oncollection: (cid: string) => void } = $props();
  const sb = $derived(win.kind === "collection" && win.ref.startsWith("sb:") ? win.ref.slice(3) : null);
  const c = $derived(win.kind === "collection" && !sb ? arr.coll(win.ref) : undefined);
  const items = $derived(c ? arr.items(c.id) : []);
  const item = $derived(win.kind === "item" ? ITEMS[win.ref] : win.sel ? ITEMS[win.sel] : undefined);
  const app = $derived(win.kind === "app" ? ([...APPS, SETTINGS].find((a) => a.id === win.ref) ?? { id: win.ref, name: arr.custom.find((x) => x.id === win.ref)?.name ?? win.ref, icon: "sidebar" }) : undefined);
  let density = $state<Record<string, "compact" | "comfortable">>({}), optOpen = $state(false), optEl = $state<HTMLElement | null>(null);
  const dens = $derived(density[win.ref] ?? tweaks.t.density);
  const opts = $derived<MenuItem[]>([{ id: "compact", label: "One line per row", checked: dens === "compact" }, { id: "comfortable", label: "Comfortable: two lines", checked: dens === "comfortable" }]);
  let views = $state<Record<string, BigView | "calendar">>({});
  const view = $derived(views[win.ref] ?? (forced.view as BigView | null) ?? c?.big ?? "list");
  const VIEWS = [{ value: "list", label: "List" }, { value: "table", label: "Table" }, { value: "board", label: "Board" }, { value: "gallery", label: "Gallery" }, { value: "calendar", label: "Calendar" }];
</script>

<div class="wb" data-win-body={win.kind}>
  {#if sb}
    <SidebarFile sid={sb} onopen={oncollection} />
  {:else if c}
    <header class="ch"><p class="q">{items.length} {items.length === 1 ? "item" : "items"}</p>
      <span class="vo"><Selector options={VIEWS} value={view} label="View" size="sm" fill={false} onchange={(v: string) => (views[win.ref] = v as BigView)} />{#if view === "list"}<IconButton icon="more" label="View options" onclick={(e: MouseEvent) => { optEl = e.currentTarget as HTMLElement; optOpen = true; }} />{/if}</span></header>
    <Menu open={optOpen} items={opts} label="View options" onpick={(id) => { density[win.ref] = id as "compact" | "comfortable"; optOpen = false; }} onclose={() => (optOpen = false)} trigger={optEl} />
    {#key view}
      <div class="v" data-view={view}>
        {#if view === "list"}<div class="list">{#each items as it (it.id)}<Item item={it} big two={dens === "comfortable"} onopen={onopen} />{/each}</div>
        {:else if view === "table"}<TableView {items} {onopen} />
        {:else if view === "board"}<BoardView {items} {onopen} />
        {:else if view === "calendar"}<CalendarView {items} {onopen} />
        {:else}<GalleryView {items} {onopen} />{/if}
      </div>
    {/key}
  {:else if item}
    {#if itemPages[item.kind]}{@const P = itemPages[item.kind]!}<P {item} {win} />{:else}<ItemPage {item} />{/if}
  {:else if app && pages[app.id]}
    {@const P = pages[app.id]!}<P {win} {onopen} {oncollection} />
  {:else if app && app.id === "settings"}
    <SettingsPage />
  {:else if app && app.id === "chat"}
    <div class="thread"><div class="b o">Can we agree the plan before Friday?<time>9:16</time></div><div class="b">Two options: the old market, or the place by the river.<time>9:19</time></div><div class="b ag"><em>Codex</em> The river place has a free table at 19:00.<time>9:21</time></div></div>
  {:else if app && !APPS.some((a) => a.id === app.id) && app.id !== "settings"}
    <p class="q">A sidebar that belongs to no app. Its sections are collections: open one as a collection, or open an item and it opens in a window beside this one. Open it as a collection from the Collections tab to see its file.</p>
  {:else if app}
    <p class="q">A stand-in for the {app.name} screen.</p>
    <div class="cards">{#each [1, 2, 3, 4, 5, 6] as i (i)}<div class="card"><Av name={`${app.name} ${i}`} /><strong>{app.name} item {i}</strong><span>Fixture data</span></div>{/each}</div>
  {/if}
</div>

<style>
  .wb { display: flex; flex-direction: column; flex: 1; min-height: 0; padding: 0 var(--s-6) var(--s-6); } .h { margin: 0; padding: var(--s-2) 0; font-size: var(--text-title); font-weight: var(--strong-weight); line-height: 1.2; } .q { margin: 0 0 var(--s-4); color: var(--ink-3); font-size: var(--text-ui); line-height: 1.45; }
  /* A list page (a collection view, Notes home, Contacts, Tasks) scrolls as ONE page, its own header included, so nothing is cut along a band under the controls; a page with a pinned foot (a chat) keeps its own scroller. */
  .wb:has(> .v), .wb:has(> :global(.nh)), .wb:has(> :global(.cl)), .wb:has(> :global(.tkp)) { flex: 1 0 auto; min-height: auto; }
  .ch { flex: none; display: flex; align-items: center; justify-content: space-between; gap: var(--s-4); flex-wrap: wrap; min-height: calc(var(--pill-h) + var(--s-5)); padding-bottom: var(--s-4); box-sizing: border-box; } .ch .q { margin: 0; }
  .v { flex: 1 0 auto; /* the window scrolls the whole page, header included */ animation: vin var(--dur-base) var(--ease-out); } @keyframes vin { from { opacity: 0; translate: 0 6px; } }
  .v[data-view="board"], .v[data-view="table"], .v[data-view="calendar"] { overflow: hidden; }
  .vo { display: inline-flex; align-items: center; gap: var(--s-3); } .list { display: grid; grid-template-columns: minmax(0, 1fr); } .gallery { display: grid; grid-template-columns: repeat(auto-fill, minmax(9.5rem, 1fr)); gap: var(--s-6) var(--s-5); align-items: start; padding-bottom: var(--s-6); }
  .thread { display: flex; flex-direction: column; gap: var(--s-3); max-width: 36rem; padding-top: var(--s-4); } .b { align-self: flex-start; padding: var(--s-4) var(--s-5); border-radius: var(--r-card); background: rgb(var(--ink-rgb) / 0.07); font-size: var(--text-ui); line-height: 1.45; } .b.o { align-self: flex-end; background: var(--accent-soft); } .b time { margin-inline-start: var(--s-4); color: var(--ink-3); font-size: var(--text-label); line-height: 1.25; } .ag em { font-style: normal; font-weight: var(--strong-weight); }
  .cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(12rem, 1fr)); gap: var(--s-5); } .card { display: grid; gap: var(--s-3); padding: var(--s-5); border-radius: var(--r-card); background: rgb(var(--ink-rgb) / 0.06); font-size: var(--text-ui); } .card span { color: var(--ink-3); font-size: var(--text-label); }
</style>

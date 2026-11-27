<script lang="ts">
  // A sidebar section IS a collection shown small. Its heading (a chevron folds it) has its own actions on hover and keyboard focus: add an item, sort and filter, and the section menu (open as a collection, small view, rename, hide, remove).
  // A person can drag a section by its grip (or, with a mouse, its heading) to another place in the sidebar, or out of it to remove it; and drag rows within it or into other sections (dnd.svelte.ts): onto a pinned strip a row is pinned.
  // Rows are one line. Fold: the height gathers and the rows fade over `--dur-base` on `--ease-out`; a second click mid-way reverses from where it is; reduced motion is instant. A list longer than six rows shows six and "Show all".
  import { Icon, Menu, tip, type MenuItem } from "@calternal/ui";
  import AppIcon from "../shell/AppIcon.svelte";
  import Item from "../shell/Item.svelte";
  import IconPicker from "../shell/IconPicker.svelte";
  import { type Item as ItemT } from "../data/collections";
  import { arr, type SecRef } from "../arrange.svelte";
  import { dnd } from "../dnd.svelte";
  let { sid, sec, editable = true, selected, onopen, oncollection }: { sid: string | null; sec: SecRef; editable?: boolean; selected: string | null; onopen: (i: ItemT) => void; oncollection: (cid: string) => void } = $props();
  const c = $derived(arr.coll(sec.cid)!);
  const name = $derived(sec.name ?? c.name);
  const view = $derived(sec.view ?? c.small);
  const shut = $derived(sec.shut ?? c.shut ?? false);
  const all = $derived(arr.items(sec.cid, sec));
  const LIMIT = 6;
  const shown = $derived(view === "list" ? all.slice(0, LIMIT) : all);
  let menu = $state<"section" | "sort" | null>(null), trig = $state<HTMLElement | null>(null), strip = $state<HTMLElement>(), fl = $state(false), fr = $state(true);
  let renaming = $state(false), draft = $state(""), rowMenu = $state<{ item: ItemT; el: HTMLElement } | null>(null), picker = $state<ItemT | null>(null);
  const fades = () => { if (!strip) return; fl = strip.scrollLeft > 2; fr = strip.scrollLeft + strip.clientWidth < strip.scrollWidth - 2; };
  $effect(() => { void strip; void view; queueMicrotask(fades); });
  const sectionItems = $derived<MenuItem[]>([{ id: "open", label: "Open as collection", icon: "expand" }, ...(editable ? [{ id: "add", label: "Add an item", icon: "plus" }, { id: "sortm", label: "Sort: my order", checked: (sec.sort ?? "manual") === "manual" }, { id: "sortt", label: "Sort: by title", checked: sec.sort === "title" }, { id: "unread", label: "Only unread or open", checked: !!sec.unread }, { id: "list", label: "Small view: list", icon: "list", checked: view === "list" }, { id: "strip", label: "Small view: strip", icon: "apps", checked: view === "strip" }, { id: "rename", label: "Rename", icon: "edit" }, { id: "hide", label: "Hide section", icon: "archive" }, { id: "remove", label: "Remove from sidebar", icon: "close", danger: true }] as MenuItem[] : [])]);
  const sortItems = $derived<MenuItem[]>([{ id: "manual", label: "Sort: my order", checked: (sec.sort ?? "manual") === "manual" }, { id: "title", label: "Sort: by title", checked: sec.sort === "title" }, { id: "unread", label: "Only unread or open", checked: !!sec.unread }]);
  const pick = (id: string) => {
    const m = menu; menu = null; if (m === "sort") { if (id === "unread") sid && arr.patchSection(sid, c.id, { unread: !sec.unread }); else sid && arr.patchSection(sid, c.id, { sort: id as "manual" | "title" }); return; }
    if (id === "add") add(); else if (id === "sortm" || id === "sortt") sid && arr.patchSection(sid, c.id, { sort: id === "sortt" ? "title" : "manual" }); else if (id === "unread") sid && arr.patchSection(sid, c.id, { unread: !sec.unread });
    else if (id === "open") oncollection(c.id); else if (id === "list" || id === "strip") sid && arr.patchSection(sid, c.id, { view: id });
    else if (id === "rename") { draft = name; renaming = true; } else if (id === "hide") sid && arr.patchSection(sid, c.id, { hidden: true }); else if (id === "remove") sid && arr.removeSection(sid, c.id);
  };
  const toggle = () => { if (dnd.quiet || !sid || renaming) return; arr.patchSection(sid, c.id, { shut: !shut }); };
  const rename = () => { if (!renaming) return; renaming = false; const n = draft.trim(); if (sid) arr.patchSection(sid, c.id, { name: n && n !== c.name ? n : undefined }); };
  const drag = (e: PointerEvent, now: boolean) => { if (editable && sid) dnd.begin(e, "section", c.id, sid, { now, hold: !now }); };
  const rowDown = (e: PointerEvent, it: ItemT) => { if (editable && sid) dnd.begin(e, "row", c.id, sid, { hold: true, row: it.id }); };
  const focusMe = (n: HTMLInputElement) => { n.focus(); n.select(); };
  const rowItems = $derived<MenuItem[]>([{ id: "icon", label: "Change icon and colour", icon: "edit" }, { id: "open", label: "Open", icon: "expand" }]);
  const rowPick = (id: string) => { const r = rowMenu; rowMenu = null; if (!r) return; if (id === "icon") picker = r.item; else onopen(r.item); };
  const add = () => { if (sid) { arr.patchSection(sid, c.id, { shut: false }); arr.addItem(c.id); } };
</script>

<section class="sec" class:flash={dnd.flash === c.id} class:drag={dnd.drag?.cid === c.id && dnd.drag.kind === "section"} class:target={dnd.drag?.kind === "row" && dnd.drag.target === c.id && dnd.drag.over && !!c.pin} data-sec data-cid={c.id}>
  {#if !c.bare}
    <header class="sh">
      {#if renaming}
        <form class="rn" onsubmit={(e) => { e.preventDefault(); rename(); }}><input bind:value={draft} aria-label="Section name" onblur={rename} onkeydown={(e) => e.key === "Escape" && (renaming = false)} use:focusMe /></form>
      {:else}
        <button type="button" class="tog" onclick={() => !dnd.quiet && oncollection(c.id)} onkeydown={(e) => { if (e.key === "m" || e.key === "ContextMenu" || (e.shiftKey && e.key === "F10")) { e.preventDefault(); trig = e.currentTarget as HTMLElement; menu = "section"; } }} onpointerdown={(e) => drag(e, false)} ondblclick={() => editable && ((draft = name), (renaming = true))}><span class="nm" title={name}>{name}</span></button><button type="button" class="chev-b" aria-expanded={!shut} aria-label={shut ? `Unfold ${name}` : `Fold ${name}`} use:tip={{ label: shut ? `Unfold ${name}` : `Fold ${name}` }} onclick={toggle}><span class="chev" class:shut><Icon name="chevron-down" inline /></span></button>
      {/if}
      <span class="act">
        {#if editable}<button type="button" class="ab" aria-label={`Add to ${name}`} use:tip={{ label: `Add to ${name}` }} onclick={add}><Icon name="plus" inline /></button>
        <button type="button" class="ab" class:on={!!sec.unread || sec.sort === "title"} aria-haspopup="menu" aria-label={`${name}: sort and filter`} use:tip={{ label: `${name}: sort and filter` }} bind:this={trig} onclick={(e) => { trig = e.currentTarget as HTMLElement; menu = "sort"; }}><AppIcon name="sort" inline /></button>{/if}
        <button type="button" class="ab mo" aria-haspopup="menu" aria-label={`${name}: section menu`} use:tip={{ label: `${name}: section menu` }} onclick={(e) => { trig = e.currentTarget as HTMLElement; menu = "section"; }}><Icon name="more" inline /></button>
        {#if editable}<span class="grip" role="presentation" aria-hidden="true" onpointerdown={(e) => drag(e, true)}><AppIcon name="grip" inline /></span>{/if}
      </span>
    </header>
  {:else}
    <button type="button" class="ab bare" aria-haspopup="menu" aria-label={`${name}: section menu`} onclick={(e) => { trig = e.currentTarget as HTMLElement; menu = "section"; }} onpointerdown={(e) => drag(e, false)}><Icon name="more" inline /></button>
  {/if}
  <div class="fold" class:shut inert={shut}><div class="in">
    {#if view === "strip"}
      <div class="strip" bind:this={strip} onscroll={fades} class:fl class:fr role="list" aria-label={name}>{#each shown as it (it.id)}<div role="listitem" class="sl" class:fl2={dnd.flash === it.id} data-row={it.id} onpointerdown={(e) => rowDown(e, it)}><Item item={it} mode="strip" selected={selected === it.id} onopen={onopen} /></div>{/each}</div>
    {:else}
      {#each shown as it (it.id)}<div class="rw" class:fl2={dnd.flash === it.id} class:lifted={dnd.drag?.row === it.id} data-row={it.id} onpointerdown={(e) => rowDown(e, it)}><Item item={it} selected={selected === it.id} onopen={onopen} oncontext={it.cal ? (i, el) => (rowMenu = { item: i, el }) : undefined} /></div>{/each}
      {#if !all.length}<p class="em">Empty. Drag a row here.</p>{/if}
      {#if all.length > LIMIT}<button type="button" class="all" onclick={() => oncollection(c.id)}>Show all {all.length}</button>{/if}
    {/if}
  </div></div>
  <Menu open={menu !== null} items={menu === "sort" ? sortItems : sectionItems} label={menu === "sort" ? `${name}: sort and filter` : `${name} section`} onpick={pick} onclose={() => (menu = null)} trigger={trig} />
  <Menu open={rowMenu !== null} items={rowItems} label="Cal options" onpick={rowPick} onclose={() => (rowMenu = null)} trigger={rowMenu?.el ?? null} />
  {#if picker}<IconPicker open={true} id={picker.id} name={picker.title} onclose={() => (picker = null)} />{/if}
</section>

<style>
  .sec { position: relative; border-radius: var(--r-select); } .sec.drag { opacity: 0.4; } .sec.flash { animation: flash var(--dur-pulse) var(--ease-out); } .sec.target { outline: 2px solid var(--accent); outline-offset: -2px; background: var(--accent-soft); }
  @keyframes flash { from { background: var(--accent-soft); } } .fl2 { animation: flash var(--dur-pulse) var(--ease-out); } .lifted { opacity: 0.4; }
  /* heading: larger than the rows (text-heading, strong), its actions at the end on hover and focus */
  .sh { position: relative; display: flex; align-items: center; margin-top: var(--sb-gap-before); margin-bottom: var(--sb-gap-head); }
  .tog { flex: 1; min-width: 0; display: flex; align-items: center; gap: var(--s-3); min-height: var(--sb-head-h); padding: 0 var(--s-5); text-align: start; color: var(--ink-2); font-size: var(--text-heading); line-height: 1.2; font-weight: var(--strong-weight); border-radius: var(--r-select); } .nm { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; flex: 0 1 auto; } .tog { justify-content: flex-start; }
  .chev-b { position: absolute; inset-inline-start: 0; top: 0; bottom: 0; display: none; } .sh { gap: var(--s-1); } .chev-b { position: static; display: grid; place-items: center; flex: none; width: 24px; height: var(--sb-head-h); border-radius: var(--r-select); color: var(--ink-3); } .tog { flex: 0 1 auto; }
  .chev { display: inline-grid; color: var(--ink-3); transition: rotate var(--dur-base) var(--ease-out); } .chev.shut { rotate: -90deg; }
  .act { position: absolute; inset-inline-end: var(--s-2); top: 50%; translate: 0 -50%; display: inline-flex; align-items: center; padding: 0; border-radius: var(--r-pill); background: rgb(var(--tint) / 0.72); backdrop-filter: blur(8px); } /* the heading keeps its whole width: its actions lie over its end, so a narrow sidebar never truncates the name for them */ .ab, .grip { flex: none; display: grid; place-items: center; width: 26px; height: 26px; color: var(--ink-3); border-radius: var(--r-select); } .ab:hover, .ab.on { color: var(--ink); background: var(--hover); } .ab.on { color: var(--accent); } .grip { cursor: grab; touch-action: none; }
  /* on a phone only the section menu shows (add, sort and filter are in it); the section is reordered by holding its heading */
  @media (hover: none) { .ab:not(.mo), .grip { display: none; } }
  .ab.bare { position: absolute; top: 0; right: 0; z-index: 1; }
  .rn { flex: 1; padding: 0 var(--s-5); } .rn input { width: 100%; min-height: var(--row-h); background: none; border: 0; border-bottom: 2px solid var(--accent); outline: 0; color: var(--ink); font: inherit; font-size: var(--text-heading); font-weight: var(--strong-weight); }
  @media (hover: hover) and (pointer: fine) { .act { opacity: 0; transition: opacity var(--dur-fast) var(--ease-out); } .sec:hover .act, .sec:focus-within .act { opacity: 1; } .ab.bare { opacity: 0; } .sec:hover .ab.bare, .sec:focus-within .ab.bare { opacity: 1; } }
  /* every heading control answers the pointer: hover tint, a press scale (--press-scale over --dur-fast), a visible focus ring */
  .tog, .ab, .chev-b { transition: background-color var(--dur-fast) var(--ease-out), scale var(--dur-fast) var(--ease-out), color var(--dur-fast) var(--ease-out); } .tog:active, .ab:active, .chev-b:active { scale: var(--press-scale); }
  @media (hover: hover) and (pointer: fine) { .tog:hover, .chev-b:hover { background: var(--hover); } .ab:hover { background: var(--hover); color: var(--ink); } .chev-b:hover { color: var(--ink-2); } }
  .tog:focus-visible, .ab:focus-visible, .chev-b:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  .fold { display: grid; grid-template-columns: minmax(0, 1fr); grid-template-rows: 1fr; transition: grid-template-rows var(--dur-base) var(--ease-out), opacity var(--dur-base) var(--ease-out); } .fold.shut { grid-template-rows: 0fr; opacity: 0; } .fold > .in { min-height: 0; overflow: clip; }
  .rw { touch-action: pan-y; } .em, .all { display: block; width: 100%; min-height: var(--row-h); padding: var(--s-3) var(--s-5); margin: 0; text-align: start; color: var(--ink-3); font-size: var(--text-label); border-radius: var(--r-select); }
  .strip { display: flex; gap: var(--s-4); overflow-x: auto; padding: var(--s-2) var(--s-3); scrollbar-width: none; overscroll-behavior-x: contain; --fl: 0px; --fr: 0px;
    -webkit-mask-image: linear-gradient(to right, transparent 0, #000 var(--fl), #000 calc(100% - var(--fr)), transparent 100%); mask-image: linear-gradient(to right, transparent 0, #000 var(--fl), #000 calc(100% - var(--fr)), transparent 100%); }
  .strip.fl { --fl: var(--s-9); } .strip.fr { --fr: var(--s-9); } .sl { flex: none; }
</style>

<script lang="ts">
  // Notes sidebar: the current space's notes (the fixture's, or the server's in server mode), newest first, and a way to make a new one.
  import { Button, Icon, IconButton, Menu, Panel, pressRow, pressRows, registerPreview, Row, SearchField, VirtualList, failed, type MenuItem, type PressBag } from "@calternal/ui";
  import { say } from "../../server/say";
  import NotePressPreview from "./NotePressPreview.svelte";
  import { href, navigate, route, segments } from "../../lib/route.svelte";
  import { spaces } from "../../shell/spaces.svelte";
  import { notes } from "./store.svelte";
  import { reach } from "../../lib/reach";
  import { shell } from "../../shell/shell.svelte";
  import { incoming } from "./share/incoming.svelte";
  import { beginSelect, endSelect, selectAll, allPicked, selection, selecting, toggleId, pickRange, provideList, setPicked } from "../../lib/select.svelte";
  import { collectionTitle, createCollection } from "./share/collections";
  import { twoFinger } from "../../lib/two-finger";
  import ShareHost from "./share/ShareHost.svelte";
  import { openShare, shareUi } from "./share/share.svelte";
    // What others have shared with the person, read once (the live stream refreshes it when the sharing lane publishes it).
  $effect(() => { void incoming.pages.refresh().catch(() => {}); });
  // The list follows the space the shell is showing.
  $effect(() => { void notes.useSpace(spaces.current.id); });
  const current = $derived(segments()[0] === "notes" ? segments()[1] : undefined);
  // Each note's menu (the "…" button and a right-click): Rename in place, Move to Trash (with Undo).
  let ctx = $state<{ x: number; y: number; note: (typeof notes.list)[number] } | null>(null);
  let allTags = $state(false);
  // Find a note by the start of its title (the space's title index, one request per pause): the list holds only the newest page, and this reaches every note.
  let q = $state(""), hits = $state<{ id: string; title: string }[] | null>(null);
  $effect(() => {
    const text = q.trim(), space = notes.space?.id;
    if (!text || !space) { hits = null; return; }
    const h = setTimeout(() => void notes.client.lookupNotes(space, text).then((r) => (hits = r), () => (hits = [])), 150);
    return () => clearTimeout(h);
  });

  // ---- Selecting several notes (ported from the existing app's NoteList: lib/select.svelte.ts). The list says what it holds, so ⇧-click ranges and Select all reach rows that are not drawn; leaving drops the selection.
  // ⌘/Ctrl-click or ⇧-click starts; while selecting a click picks instead of opening; Space picks the focused row, Esc lets go. Share makes a collection of the picked notes and opens its sheet.
  $effect(() => provideList("note", { order: () => notes.list.map((n) => n.id) }));
  $effect(() => () => { if (selecting("note")) endSelect(); });
  const picking = $derived(selecting("note"));
  const isMac = /Mac|iPhone|iPad/.test(navigator.platform);
  function pickClick(e: MouseEvent) {
    const id = (e.target as Element | null)?.closest<HTMLElement>("[data-select-id]")?.dataset.selectId;
    if (!id) return;
    const quiet = e.detail === 0;
    if (selecting("note")) { e.preventDefault(); e.stopPropagation(); if (e.shiftKey) pickRange(id, quiet); else toggleId(id, quiet); }
    else if ((isMac ? e.metaKey : e.ctrlKey) || e.shiftKey) { e.preventDefault(); e.stopPropagation(); beginSelect("note", id, quiet); }
  }
  function selectKey(e: KeyboardEvent) {
    if (!selecting("note")) return;
    if (e.key === "Escape") { e.stopPropagation(); endSelect(); return; }
    const row = (document.activeElement as HTMLElement | null)?.closest<HTMLElement>("[data-select-id]");
    if (e.key === " " && row) { e.preventDefault(); toggleId(row.dataset.selectId!, true); }
  }
  /** The picked notes go to the Trash together (one toast, one Undo). */
  async function trashSelection() {
    const picked = [...selection.ids].map((id) => notes.list.find((n) => n.id === id)).filter((n): n is NonNullable<typeof n> => !!n);
    endSelect();
    await notes.removeMany(picked.map((n) => ({ id: n.id, title: n.title })));
  }
  /** One picked note is shared as itself; several become a collection (`collections.create`) that is shared. */
  async function shareSelection(from: HTMLElement) {
    const ids = [...selection.ids];
    const space = notes.space?.id;
    if (!ids.length || !space) return;
    const picked = ids.map((id) => notes.list.find((n) => n.id === id)).filter((n): n is NonNullable<typeof n> => !!n);
    const title = collectionTitle(picked.map((n) => n.title));
    try {
      const item = ids.length === 1 ? { kind: "notes" as const, id: ids[0]! } : { kind: "collections" as const, id: (await createCollection(space, title, ids)).id };
      endSelect();
      openShare(item, title, from);
    } catch (e) { failed(say(e, "share these notes")); }
  }
  let renaming = $state<string | null>(null), draft = $state("");
  const items: MenuItem[] = [{ id: "rename", label: "Rename" }, { id: "select", label: "Select" }, { id: "trash", label: "Move to Trash", danger: true }];
  const menuAt = (e: MouseEvent, note: (typeof notes.list)[number], el?: HTMLElement) => { e.preventDefault(); e.stopPropagation(); const r = el?.getBoundingClientRect(); ctx = { x: r?.right ?? e.clientX, y: r?.bottom ?? e.clientY, note }; };
  registerPreview<string>("note", NotePressPreview);
  /** What a held or right-clicked row says to the shared press (ui/lib/press.ts): the same menu as the three dots, and a preview of the note's top. */
  const bagOf = (n: (typeof notes.list)[number]): PressBag => ({ label: `Note: ${n.title}`, items: () => items, onpick: (id) => { if (id === "rename") { renaming = n.id; draft = n.title; } else if (id === "select") beginSelect("note", n.id); else void notes.remove(n.id, n.title); }, preview: { kind: "note", item: n.id } });
  function pick(id: string) { const c = ctx; ctx = null; if (!c) return; if (id === "rename") { renaming = c.note.id; draft = c.note.title; } else if (id === "select") beginSelect("note", c.note.id); else void notes.remove(c.note.id, c.note.title); }
  const focusNow = (el: HTMLElement) => { el.focus(); (el as HTMLInputElement).select(); };
  const commit = (n: (typeof notes.list)[number]) => { const t = draft; renaming = null; void notes.rename(n, t); };
  async function create() {
    const n = await notes.create("Untitled");
    if (n) { navigate(`/notes/${n.id}`); if (shell.narrow && shell.sidebarOpen) shell.toggleSidebar(); } // a phone: the list steps aside to show the new note
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions: one delegated listener for every row (the rows are the interactive elements) -->
<nav aria-label="Notes" onclickcapture={pickClick} onkeydown={selectKey} use:twoFinger={{ kind: "note" }} use:pressRows>
  {#if notes.space}<p class="section-heading first">{notes.space.name}</p>{/if}
  {#if notes.tags.length}
    <div class="tags" role="group" aria-label="Filter by tag">
      <button type="button" class="tg" aria-pressed={!notes.tag} onclick={() => notes.filter(null)}>All</button>
      {#each (allTags ? notes.tags : notes.tags.slice(0, 12)) as t (t.name)}<button type="button" class="tg" aria-pressed={notes.tag === t.name} onclick={() => notes.filter(notes.tag === t.name ? null : t.name)}>#{t.name} <span class="ct">{t.count}</span></button>{/each}
    {#if notes.tags.length > 12 || notes.tagPages.more}<button type="button" class="tg" onclick={() => { allTags = !allTags; if (allTags) void notes.tagPages.next(); }}>{allTags ? "Fewer tags" : `${notes.tags.length > 12 ? `${notes.tags.length - 12}${notes.tagPages.more ? "+" : ""} ` : ""}more…`}</button>{/if}
    {#if allTags && notes.tagPages.more}<div class="end" aria-hidden="true" use:reach={() => notes.tagPages.next()}></div>{/if}
    </div>
  {/if}
  <SearchField class="find" bind:value={q} label="Find a note by title" placeholder="Find a note" />
  {#if hits}
    <ul>{#each hits as n (n.id)}<li><Row title={n.title} icon="notes" href={href(`/notes/${n.id}`)} selected={current === n.id} /></li>{:else}<li class="none">No note starts with “{q.trim()}”.</li>{/each}</ul>
  {:else}
  {#if notes.list.length}
    <!-- Only the rows near the view exist in the DOM (VirtualList), however many notes there are; the next page is read when the window nears the last loaded row. -->
    <VirtualList inside count={notes.list.length} estimate={40} overscan={600} label="Notes" onrange={(_, end) => { if (end >= notes.list.length - 40) void notes.notePages.next(); }}>
      {#snippet row(i)}{@const n = notes.list[i]}{#if n}<div class="li" role="presentation" data-select-id={n.id} data-picked={selection.ids.has(n.id) || undefined} use:pressRow={() => bagOf(n)}>
    {#if renaming === n.id}
      <input class="ren" aria-label="Note title" bind:value={draft} onkeydown={(e) => { if (e.key === "Enter") commit(n); else if (e.key === "Escape") renaming = null; }} onblur={() => commit(n)} use:focusNow />
    {:else}
      <Row title={n.title} icon="notes" href={href(`/notes/${n.id}`)} selected={current === n.id}>{#snippet trailing()}{#if picking}{#if selection.ids.has(n.id)}<Icon name="check" />{/if}{:else}<IconButton icon="more" label="Actions for {n.title}" onclick={(e) => menuAt(e, n, e.currentTarget as HTMLElement)} />{/if}{/snippet}</Row>
    {/if}
      </div>{/if}{/snippet}
    </VirtualList>
  {:else}<ul><li class="none">{notes.error || "No notes yet."}{#if notes.error} <button type="button" class="retry" onclick={() => notes.retry()}>Try again</button>{/if}</li></ul>{/if}
  {/if}
  {#if !hits}{#if notes.notePages.error}<p class="cap" role="alert">{notes.notePages.error} <button type="button" class="retry" onclick={() => notes.notePages.retry()}>Try again</button></p>{/if}{/if}
  {#if !hits && incoming.items.length}
    <div role="group" aria-label="Shared with me" class="grp">
      <p class="section-heading later">Shared with me</p>
      <ul>
        {#each incoming.items as s (s.item.id)}
          <li><Row title={s.title} icon="notes" href={href(`/notes/${s.item.id}`)} selected={current === s.item.id}>{#snippet trailing()}{#if s.unread}<span class="dot" role="img" aria-label="Changed since you last looked"></span>{/if}{/snippet}</Row></li>
        {/each}
      </ul>
    </div>
  {/if}
  <button type="button" class="new" onclick={create}>New note</button>
  <a class="new trash" href={href("/notes?view=trash")} aria-current={route.query.get("view") === "trash" ? "page" : undefined}>Trash</a>
  {#if notes.waiting.length}
    <div class="wait" role="group" aria-label="Waiting to send">
      <p class="section-heading">Waiting to send</p>
      {#each notes.waiting as w (w.id)}
        <div class="w">
          <span class="wt">{w.operation === "notes.update" ? String(w.input.title ?? "Note edit") : w.operation}{w.state === "confirmation_required" ? " · needs you" : ""}</span>
          <Button variant="quiet" text={w.state === "confirmation_required" ? "Confirm" : "Send now"} onclick={() => notes.sendWaiting(w.id)} />
          <Button variant="quiet" text="Cancel" onclick={() => notes.cancelWaiting(w.id)} />
        </div>
      {/each}
    </div>
  {/if}
  {#if picking}
    <Panel class="bulk" radius="card" dense floating role="toolbar" aria-label="Selected notes">
      <span class="cnt" role="status">{selection.ids.size} selected</span>
      <Button variant="primary" text="Share" disabled={!selection.ids.size} onclick={(e) => void shareSelection(e.currentTarget)} />
      <Button variant="destructive" text="Move to Trash" disabled={!selection.ids.size} onclick={() => void trashSelection()} />
      <Button variant="quiet" text={allPicked() ? "Deselect all" : "Select all"} onclick={() => (allPicked() ? setPicked([]) : selectAll())} />
      <Button variant="quiet" text="Done" onclick={endSelect} />
    </Panel>
  {/if}
</nav>
{#if shareUi.item?.kind === "collections"}<ShareHost item={shareUi.item} />{/if}
<Menu open={!!ctx} {items} label="Note actions" anchor={ctx ? { left: ctx.x, top: ctx.y, right: ctx.x, bottom: ctx.y } : null} placement="bottom" onclose={() => (ctx = null)} onpick={pick} />

<style>
  nav { display: grid; gap: var(--s-2); }
  /* The nav's own gap sits between the heading and what follows, so the heading's margin gives that much back (the shared distances, base.css `.section-heading`). */
  .section-heading { margin-bottom: calc(var(--section-below) - var(--s-2)); }
  ul { display: grid; gap: var(--s-1); }
  .li { padding-bottom: var(--s-1); }
  .retry { text-decoration: underline; color: var(--ink-2); }
  .ren { width: 100%; min-height: var(--row-h); padding: 0 var(--s-5); border: 0; border-radius: var(--r-select); background: rgb(var(--ink-rgb) / 0.07); color: var(--ink); font-size: var(--text-ui); }
  .tags { display: flex; flex-wrap: wrap; gap: var(--s-1); padding: 0 var(--s-5) var(--s-2); }
  .tg { padding: var(--s-1) var(--s-4); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.06); font-size: var(--text-label); color: var(--ink-2); }
  .tg[aria-pressed="true"] { background: var(--accent-soft); color: var(--ink); font-weight: var(--strong-weight); }
  .ct { color: var(--ink-3); }
  /* The group is a grid like the nav, so its heading keeps the shared distances (the nav gap above it is part of the space above). */
  .grp { display: grid; gap: var(--s-2); }
  .later { margin-top: calc(var(--section-above) - var(--s-2)); }
  /* The bar is the last thing in the list and sticks to the bottom of the scroller, above the bottom tier, so it is in reach however far the list is scrolled (a sticky bottom holds an element that would scroll off the bottom edge). */
  nav :global(.bulk) { position: sticky; bottom: var(--s-2); z-index: 2; background: color-mix(in oklab, var(--paper) 90%, transparent); display: flex; flex-wrap: wrap; align-items: center; gap: var(--s-1); padding: var(--s-3) var(--s-4); }
  .cnt { flex: 1 1 100%; font-size: var(--text-label); font-weight: var(--strong-weight); color: var(--ink-2); }
  .li[data-picked] :global(.row) { background: var(--accent-soft); }
  .dot { width: var(--s-3); height: var(--s-3); border-radius: 50%; background: var(--accent); }
  .trash { display: flex; align-items: center; }
  /* The shared search field (its text is the touch body size, 17 pt: a smaller one makes iOS Safari zoom the page when it is focused). */
  nav :global(.find) { margin: 0 var(--s-5) var(--s-2); }
  .end { height: 1px; flex-basis: 100%; }
  .cap { padding: var(--s-2) var(--s-5); font-size: var(--text-label); color: var(--ink-3); }
  .none { padding: var(--s-4) var(--s-5); color: var(--ink-3); font-size: var(--text-label); }
  .wait { display: grid; gap: var(--s-1); margin-top: var(--s-4); }
  .w { display: flex; flex-wrap: wrap; align-items: center; gap: var(--s-1); padding: var(--s-2) var(--s-5); font-size: var(--text-label); }
  .wt { flex: 1 1 100%; color: var(--ink-2); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .new { text-align: start; min-height: var(--row-h); padding: 0 var(--s-5); border-radius: var(--r-select); color: var(--ink-3); font-size: var(--text-label); font-weight: var(--strong-weight); }
</style>

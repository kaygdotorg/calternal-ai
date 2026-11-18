<script lang="ts">
  // Notes sidebar: the current space's notes (the fixture's, or the server's in server mode), newest first, and a way to make a new one.
  import { Button, IconButton, Menu, Row, VirtualList, type MenuItem } from "@calternal/ui";
  import { href, navigate, route, segments } from "../../lib/route.svelte";
  import { spaces } from "../../shell/spaces.svelte";
  import { notes } from "./store.svelte";
  import { reach } from "../../lib/reach";
  import { shell } from "../../shell/shell.svelte";
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
  let renaming = $state<string | null>(null), draft = $state("");
  const items: MenuItem[] = [{ id: "rename", label: "Rename" }, { id: "trash", label: "Move to Trash", danger: true }];
  const menuAt = (e: MouseEvent, note: (typeof notes.list)[number], el?: HTMLElement) => { e.preventDefault(); e.stopPropagation(); const r = el?.getBoundingClientRect(); ctx = { x: r?.right ?? e.clientX, y: r?.bottom ?? e.clientY, note }; };
  function pick(id: string) { const c = ctx; ctx = null; if (!c) return; if (id === "rename") { renaming = c.note.id; draft = c.note.title; } else void notes.remove(c.note.id, c.note.title); }
  const focusNow = (el: HTMLElement) => { el.focus(); (el as HTMLInputElement).select(); };
  const commit = (n: (typeof notes.list)[number]) => { const t = draft; renaming = null; void notes.rename(n, t); };
  async function create() {
    const n = await notes.create("Untitled");
    if (n) { navigate(`/notes/${n.id}`); if (shell.narrow && shell.sidebarOpen) shell.toggleSidebar(); } // a phone: the list steps aside to show the new note
  }
</script>

<nav aria-label="Notes">
  {#if notes.space}<p class="section-heading first">{notes.space.name}</p>{/if}
  {#if notes.tags.length}
    <div class="tags" role="group" aria-label="Filter by tag">
      <button type="button" class="tg" aria-pressed={!notes.tag} onclick={() => notes.filter(null)}>All</button>
      {#each (allTags ? notes.tags : notes.tags.slice(0, 12)) as t (t.name)}<button type="button" class="tg" aria-pressed={notes.tag === t.name} onclick={() => notes.filter(notes.tag === t.name ? null : t.name)}>#{t.name} <span class="ct">{t.count}</span></button>{/each}
    {#if notes.tags.length > 12 || notes.tagPages.more}<button type="button" class="tg" onclick={() => { allTags = !allTags; if (allTags) void notes.tagPages.next(); }}>{allTags ? "Fewer tags" : `${notes.tags.length > 12 ? `${notes.tags.length - 12}${notes.tagPages.more ? "+" : ""} ` : ""}more…`}</button>{/if}
    {#if allTags && notes.tagPages.more}<div class="end" aria-hidden="true" use:reach={() => notes.tagPages.next()}></div>{/if}
    </div>
  {/if}
  <input class="find" type="search" aria-label="Find a note by title" placeholder="Find a note" bind:value={q} autocomplete="off" />
  {#if hits}
    <ul>{#each hits as n (n.id)}<li><Row title={n.title} icon="notes" href={href(`/notes/${n.id}`)} selected={current === n.id} /></li>{:else}<li class="none">No note starts with “{q.trim()}”.</li>{/each}</ul>
  {:else}
  {#if notes.list.length}
    <!-- Only the rows near the view exist in the DOM (VirtualList), however many notes there are; the next page is read when the window nears the last loaded row. -->
    <VirtualList inside count={notes.list.length} estimate={40} overscan={600} label="Notes" onrange={(_, end) => { if (end >= notes.list.length - 40) void notes.notePages.next(); }}>
      {#snippet row(i)}{@const n = notes.list[i]}{#if n}<div class="li" role="presentation" oncontextmenu={(e) => menuAt(e, n)}>
    {#if renaming === n.id}
      <input class="ren" aria-label="Note title" bind:value={draft} onkeydown={(e) => { if (e.key === "Enter") commit(n); else if (e.key === "Escape") renaming = null; }} onblur={() => commit(n)} use:focusNow />
    {:else}
      <Row title={n.title} icon="notes" href={href(`/notes/${n.id}`)} selected={current === n.id}>{#snippet trailing()}<IconButton icon="more" label="Actions for {n.title}" onclick={(e) => menuAt(e, n, e.currentTarget as HTMLElement)} />{/snippet}</Row>
    {/if}
      </div>{/if}{/snippet}
    </VirtualList>
  {:else}<ul><li class="none">{notes.error || "No notes yet."}{#if notes.error} <button type="button" class="retry" onclick={() => notes.retry()}>Try again</button>{/if}</li></ul>{/if}
  {/if}
  {#if !hits}{#if notes.notePages.error}<p class="cap" role="alert">{notes.notePages.error} <button type="button" class="retry" onclick={() => notes.notePages.retry()}>Try again</button></p>{/if}{/if}
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
</nav>
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
  .trash { display: flex; align-items: center; }
  .find { margin: 0 var(--s-5) var(--s-2); min-height: var(--row-h); padding: 0 var(--s-4); border: 0; border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.06); color: var(--ink); font-size: var(--text-label); }
  .end { height: 1px; flex-basis: 100%; }
  .cap { padding: var(--s-2) var(--s-5); font-size: var(--text-label); color: var(--ink-3); }
  .none { padding: var(--s-4) var(--s-5); color: var(--ink-3); font-size: var(--text-label); }
  .wait { display: grid; gap: var(--s-1); margin-top: var(--s-4); }
  .w { display: flex; flex-wrap: wrap; align-items: center; gap: var(--s-1); padding: var(--s-2) var(--s-5); font-size: var(--text-label); }
  .wt { flex: 1 1 100%; color: var(--ink-2); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .new { text-align: start; min-height: var(--row-h); padding: 0 var(--s-5); border-radius: var(--r-select); color: var(--ink-3); font-size: var(--text-label); font-weight: var(--strong-weight); }
</style>

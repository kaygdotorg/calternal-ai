<script lang="ts">
  // Notes sidebar: the current space's notes (the fixture's, or the server's in server mode), newest first, and a way to make a new one.
  import { Button, IconButton, Menu, Row, type MenuItem } from "@calternal/ui";
  import { href, navigate, route, segments } from "../../lib/route.svelte";
  import { spaces } from "../../shell/spaces.svelte";
  import { notes } from "./store.svelte";
  import { shell } from "../../shell/shell.svelte";
  // The list follows the space the shell is showing.
  $effect(() => { void notes.useSpace(spaces.current.id); });
  const current = $derived(segments()[0] === "notes" ? segments()[1] : undefined);
  // Each note's menu (the "…" button and a right-click): Rename in place, Move to Trash (with Undo).
  let ctx = $state<{ x: number; y: number; note: (typeof notes.list)[number] } | null>(null);
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
  {#if notes.space}<p class="sp">{notes.space.name}</p>{/if}
  {#if notes.tags.length}
    <div class="tags" role="group" aria-label="Filter by tag">
      <button type="button" class="tg" aria-pressed={!notes.tag} onclick={() => notes.filter(null)}>All</button>
      {#each notes.tags.slice(0, 12) as t (t.name)}<button type="button" class="tg" aria-pressed={notes.tag === t.name} onclick={() => notes.filter(notes.tag === t.name ? null : t.name)}>#{t.name} <span class="ct">{t.count}</span></button>{/each}
    </div>
  {/if}
  <ul>{#each notes.list as n (n.id)}<li oncontextmenu={(e) => menuAt(e, n)}>
    {#if renaming === n.id}
      <input class="ren" aria-label="Note title" bind:value={draft} onkeydown={(e) => { if (e.key === "Enter") commit(n); else if (e.key === "Escape") renaming = null; }} onblur={() => commit(n)} use:focusNow />
    {:else}
      <Row title={n.title} icon="notes" href={href(`/notes/${n.id}`)} selected={current === n.id}>{#snippet trailing()}<IconButton icon="more" label="Actions for {n.title}" onclick={(e) => menuAt(e, n, e.currentTarget as HTMLElement)} />{/snippet}</Row>
    {/if}
  </li>{:else}<li class="none">{notes.error || "No notes yet."}{#if notes.error} <button type="button" class="retry" onclick={() => notes.retry()}>Try again</button>{/if}</li>{/each}</ul>
  <button type="button" class="new" onclick={create}>New note</button>
  <a class="new trash" href={href("/notes?view=trash")} aria-current={route.query.get("view") === "trash" ? "page" : undefined}>Trash</a>
  {#if notes.waiting.length}
    <div class="wait" role="group" aria-label="Waiting to send">
      <p class="sp">Waiting to send</p>
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
  ul { display: grid; gap: var(--s-1); }
  .sp { padding: 0 var(--s-5); font-size: var(--text-label); font-weight: var(--strong-weight); color: var(--ink-3); }
  .retry { text-decoration: underline; color: var(--ink-2); }
  .ren { width: 100%; min-height: var(--row-h); padding: 0 var(--s-5); border: 0; border-radius: var(--r-select); background: rgb(var(--ink-rgb) / 0.07); color: var(--ink); font-size: var(--text-ui); }
  .tags { display: flex; flex-wrap: wrap; gap: var(--s-1); padding: 0 var(--s-5) var(--s-2); }
  .tg { padding: var(--s-1) var(--s-4); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.06); font-size: var(--text-label); color: var(--ink-2); }
  .tg[aria-pressed="true"] { background: var(--accent-soft); color: var(--ink); font-weight: var(--strong-weight); }
  .ct { color: var(--ink-3); }
  .trash { display: flex; align-items: center; }
  .none { padding: var(--s-4) var(--s-5); color: var(--ink-3); font-size: var(--text-label); }
  .wait { display: grid; gap: var(--s-1); margin-top: var(--s-4); }
  .w { display: flex; flex-wrap: wrap; align-items: center; gap: var(--s-1); padding: var(--s-2) var(--s-5); font-size: var(--text-label); }
  .wt { flex: 1 1 100%; color: var(--ink-2); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .new { text-align: start; min-height: var(--row-h); padding: 0 var(--s-5); border-radius: var(--r-select); color: var(--ink-3); font-size: var(--text-label); font-weight: var(--strong-weight); }
</style>

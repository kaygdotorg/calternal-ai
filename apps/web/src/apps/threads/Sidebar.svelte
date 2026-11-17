<script lang="ts">
  // The Threads app's sidebar body. No automatic groups: the person makes their own sections (create, rename, collapse, delete) and arranges threads
  // freely, between sections and within them, by dragging or with the keyboard (Alt+Up and Alt+Down within a section; Alt+Shift+Up and Down to the
  // next or previous section). Double-click or F2 renames a thread in place; right-click (or the menu key) opens its context menu: rename, move to a
  // section, copy link, archive, stop if it is running. A thread's state shows only when it means something (running, waiting for you, failed). A thread
  // that is in the attention card above is not listed here (its children, if any, stand on their own).
  import { Icon, IconButton, InlineEditor, Menu, Row, undoable, type MenuItem } from "@calternal/ui";
  import { href, navigate, segments } from "../../lib/route.svelte";
  import { layout, type Container } from "./layout.svelte";
  import StateMark from "./StateMark.svelte";
  import type { ThreadSummary } from "./model";
  import { client, store } from "./store.svelte";

  const current = $derived(segments()[0] === "threads" ? segments()[1] : undefined);
  const time = (t: number) => new Date(t).toLocaleTimeString("en-GB", { hour: "2-digit", minute: "2-digit", timeZone: "UTC" });
  const hidden = $derived(new Set(store.attention.slice(0, 3).map((a) => a.thread.id)));
  const shown = $derived(store.threads.filter((t) => !hidden.has(t.id)));
  const byId = $derived(new Map(shown.map((t) => [t.id, t])));
  const top = $derived(shown.filter((t) => !t.parentId || !byId.has(t.parentId)).sort((a, b) => b.updatedAt - a.updatedAt).map((t) => t.id));
  const idsIn = (c: Container) => layout.idsOf(c, top);
  const kids = (id: string) => shown.filter((t) => t.parentId === id).sort((a, b) => b.updatedAt - a.updatedAt);
  const containers = $derived<Container[]>([null, ...layout.sections.map((s) => s.id)]);
  const nameOf = (c: Container) => (c === null ? "no section" : layout.sections.find((s) => s.id === c)?.name ?? "section");

  // ---- renaming in place ----
  let editing = $state<{ kind: "thread" | "section"; id: string } | null>(null);
  let draft = $state("");
  const edit = (kind: "thread" | "section", id: string, text: string) => { editing = { kind, id }; draft = text; };
  function commit() {
    if (!editing) return;
    const { kind, id } = editing;
    editing = null;
    if (kind === "thread") void client.rename(id, draft);
    else layout.rename(id, draft);
  }

  // ---- context menus ----
  let ctx = $state<{ x: number; y: number; thread?: ThreadSummary; section?: string } | null>(null);
  const ctxItems = $derived<MenuItem[]>(
    ctx?.thread
      ? [
          { id: "rename", label: "Rename" },
          ...containers.filter((c) => c !== layout.containerOf(ctx!.thread!.id)).map((c) => ({ id: `move:${c ?? ""}`, label: `Move to ${c === null ? "no section" : nameOf(c)}` })),
          { id: "copy", label: "Copy link" },
          ...(ctx.thread.state === "running" ? [{ id: "stop", label: "Stop" }] : []),
          { id: "archive", label: "Archive", danger: true },
        ]
      : ctx?.section
        ? [{ id: "srename", label: "Rename section" }, { id: "up", label: "Move up" }, { id: "down", label: "Move down" }, { id: "delete", label: "Delete section", danger: true }]
        : [],
  );
  function pick(id: string) {
    const c = ctx;
    ctx = null;
    if (!c) return;
    if (c.thread) {
      const t = c.thread;
      if (id === "rename") edit("thread", t.id, t.title);
      else if (id.startsWith("move:")) { const to = id.slice(5) || null; layout.move(t.id, to, idsIn(to).length, idsIn(to)); }
      else if (id === "copy") void navigator.clipboard?.writeText(location.origin + href(`/threads/${t.id}`));
      else if (id === "stop") void client.stop(t.id);
      else if (id === "archive") { void client.archive(t.id).then(() => undoable("thread.archive", t.id, "Archived", () => client.unarchive(t.id))); if (current === t.id) navigate("/threads"); }
    } else if (c.section) {
      const s = layout.sections.find((x) => x.id === c.section);
      if (!s) return;
      if (id === "srename") edit("section", s.id, s.name);
      else if (id === "up") layout.shift(s.id, -1);
      else if (id === "down") layout.shift(s.id, 1);
      else if (id === "delete") { const undo = layout.remove(s.id); if (undo) undoable("section.delete", s.id, `Deleted \u201c${s.name}\u201d`, undo); }
    }
  }
  const openCtx = (e: MouseEvent, v: { thread?: ThreadSummary; section?: string }) => { e.preventDefault(); ctx = { x: e.clientX, y: e.clientY, ...v }; };

  // ---- dragging ----
  let dragId = $state<string | null>(null);
  let over = $state<{ c: Container; index: number } | null>(null);
  const dropHere = (c: Container, index: number) => { if (dragId) { layout.move(dragId, c, index, idsIn(c)); } dragId = null; over = null; };
  function dragOverRow(e: DragEvent, c: Container, i: number) {
    if (!dragId) return;
    e.preventDefault();
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    over = { c, index: e.clientY < r.top + r.height / 2 ? i : i + 1 };
  }

  // ---- keyboard reordering ----
  let said = $state("");
  function keyRow(e: KeyboardEvent, t: ThreadSummary, c: Container, i: number, n: number) {
    if (e.key === "F2") { e.preventDefault(); return edit("thread", t.id, t.title); }
    if (e.key === "ContextMenu" || (e.shiftKey && e.key === "F10")) { const r = (e.currentTarget as HTMLElement).getBoundingClientRect(); e.preventDefault(); ctx = { x: r.left + 24, y: r.bottom, thread: t }; return; }
    if (!e.altKey || (e.key !== "ArrowUp" && e.key !== "ArrowDown")) return;
    e.preventDefault();
    const up = e.key === "ArrowUp";
    let to = c, at = i + (up ? -1 : 1);
    if (e.shiftKey) {
      const k = containers.indexOf(c) + (up ? -1 : 1);
      if (k < 0 || k >= containers.length) return;
      to = containers[k]!;
      at = up ? idsIn(to).length : 0;
    } else if (at < 0 || at >= n) return;
    layout.move(t.id, to, at, idsIn(to));
    said = `${t.title} moved to ${nameOf(to)}, position ${Math.min(at, idsIn(to).length - 1) + 1} of ${idsIn(to).length}`;
    queueMicrotask(() => document.querySelector<HTMLElement>(`[data-thread="${t.id}"] a`)?.focus());
  }
  function newSection() {
    const s = layout.create("New section");
    edit("section", s.id, s.name);
  }
</script>

{#snippet thread(t: ThreadSummary, c: Container, i: number, n: number)}
  {#if editing?.kind === "thread" && editing.id === t.id}
    <li class="edit"><InlineEditor class="rename" label="Rename thread" bind:value={draft} autoselect onblur={commit} onsubmit={commit} oncancel={() => (editing = null)} /></li>
  {:else}
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <li
      data-thread={t.id}
      class:before={over && over.c === c && over.index === i}
      class:after={over && over.c === c && over.index === i + 1 && i === n - 1}
      class:dragging={dragId === t.id}
      draggable="true"
      ondragstart={(e) => { dragId = t.id; e.dataTransfer?.setData("text/plain", t.id); if (e.dataTransfer) e.dataTransfer.effectAllowed = "move"; }}
      ondragend={() => { dragId = null; over = null; }}
      ondragover={(e) => dragOverRow(e, c, i)}
      ondrop={(e) => { e.preventDefault(); if (over) dropHere(over.c, over.index); }}
      oncontextmenu={(e) => openCtx(e, { thread: t })}
      ondblclick={(e) => { e.preventDefault(); edit("thread", t.id, t.title); }}
      onkeydown={(e) => keyRow(e, t, c, i, n)}
    >
      <Row title={t.title} href={href(`/threads/${t.id}`)} selected={current === t.id}>
        {#snippet trailing()}<span class="tail"><span class="when">{time(t.updatedAt)}</span><StateMark state={t.state} /><Icon inline name={t.provider} /></span>{/snippet}
      </Row>
      {#each kids(t.id) as k (k.id)}
        <Row depth={1} title={k.title} href={href(`/threads/${k.id}`)} selected={current === k.id} onclick={() => {}}>
        {#snippet trailing()}<span class="tail"><span class="when">{time(k.updatedAt)}</span><StateMark state={k.state} /><Icon inline name={k.provider} /></span>{/snippet}
      </Row>
      {/each}
    </li>
  {/if}
{/snippet}

{#snippet list(c: Container, ids: string[])}
  <ul class="threads" ondragover={(e) => { if (dragId) { e.preventDefault(); if (!ids.length) over = { c, index: 0 }; } }} ondrop={(e) => { e.preventDefault(); if (over) dropHere(over.c, over.index); }} role="presentation">
    {#each ids as id, i (id)}{#if byId.get(id)}{@render thread(byId.get(id)!, c, i, ids.length)}{/if}{/each}
    {#if !ids.length && c !== null}<li class="hint" class:over={over?.c === c}>Drag threads here</li>{/if}
  </ul>
{/snippet}

<nav aria-label="Threads">
  {@render list(null, idsIn(null))}

  {#each layout.sections as s (s.id)}
    {@const ids = idsIn(s.id)}
    <section class="sec" aria-label={s.name}>
      <div class="sh" class:over={over?.c === s.id && !ids.length} role="presentation" ondragover={(e) => { if (dragId) { e.preventDefault(); over = { c: s.id, index: ids.length }; } }} ondrop={(e) => { e.preventDefault(); dropHere(s.id, ids.length); }} oncontextmenu={(e) => openCtx(e, { section: s.id })}>
        {#if editing?.kind === "section" && editing.id === s.id}
          <InlineEditor class="rename" label="Rename section" bind:value={draft} autoselect onblur={commit} onsubmit={commit} oncancel={() => (editing = null)} />
        {:else}
          <button type="button" class="tg" aria-expanded={!s.collapsed} onclick={() => layout.toggle(s.id)} ondblclick={() => edit("section", s.id, s.name)}>
            <span class="chev" class:open={!s.collapsed}><Icon inline name="chevron" /></span><span class="nm">{s.name}</span>
          </button>
          <IconButton icon="more" label="Section options: {s.name}" onclick={(e) => { const r = (e.currentTarget as HTMLElement).getBoundingClientRect(); ctx = { x: r.left, y: r.bottom, section: s.id }; }} />
        {/if}
      </div>
      {#if !s.collapsed}{@render list(s.id, ids)}{/if}
    </section>
  {/each}

  <button type="button" class="new" onclick={newSection}><Icon inline name="plus" /><span>New section</span></button>
  <p class="visually-hidden" role="status" aria-live="polite">{said}</p>
</nav>

<Menu open={!!ctx} items={ctxItems} label={ctx?.thread ? `Thread: ${ctx.thread.title}` : "Section"} anchor={ctx ? { left: ctx.x, top: ctx.y, right: ctx.x, bottom: ctx.y } : null} placement="bottom" onclose={() => (ctx = null)} onpick={pick} />

<style>
  .tail { display: inline-flex; align-items: center; gap: var(--s-3); color: var(--ink-3); }
  .when { display: none; font-size: var(--text-label); font-variant-numeric: tabular-nums; }
  :global(.row:hover) .when, :global(.row:focus-visible) .when { display: inline; }
  nav { display: grid; gap: var(--s-1); padding-bottom: var(--s-6); }
  ul { display: grid; gap: var(--s-1); }
  li { position: relative; }
  li.dragging { opacity: 0.5; }
  li.before::before, li.after::after { content: ""; position: absolute; left: var(--s-4); right: var(--s-4); height: 2px; border-radius: var(--r-pill); background: var(--accent); z-index: 1; }
  li.before::before { top: calc(var(--s-1) / -2 - 1px); }
  li.after::after { bottom: calc(var(--s-1) / -2 - 1px); }
  li.edit { padding: var(--s-1) 0; }
  .hint { padding: var(--s-4) var(--s-5); border-radius: var(--r-select); color: var(--ink-3); font-size: var(--text-label); }
  .hint.over { background: var(--accent-soft); color: var(--accent); }
  .sec { display: grid; gap: var(--s-1); margin-top: var(--s-3); }
  .sh { display: flex; align-items: center; gap: var(--s-1); border-radius: var(--r-select); }
  .sh.over { background: var(--accent-soft); }
  .tg { flex: 1; min-width: 0; display: flex; align-items: center; gap: var(--s-2); min-height: var(--row-h); padding: 0 var(--s-4); border-radius: var(--r-select); text-align: start; font-size: var(--text-label); font-weight: var(--strong-weight); color: var(--ink-3); }
  .nm { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .chev { display: grid; transition: rotate var(--dur-base) var(--ease-out); }
  .chev.open { rotate: 90deg; }
  :global(.rename) { width: 100%; min-height: var(--row-h); padding: 0 var(--s-4); border: 0; border-radius: var(--r-select); background: rgb(var(--ink-rgb) / 0.1); color: var(--ink); font-size: var(--text-ui); }
  .new { display: flex; align-items: center; gap: var(--s-3); min-height: var(--row-h); margin-top: var(--s-3); padding: 0 var(--s-4); border-radius: var(--r-select); color: var(--ink-3); font-size: var(--text-label); font-weight: var(--strong-weight); }
  @media (hover: hover) and (pointer: fine) { .tg:hover, .new:hover { background: var(--hover); color: var(--ink); } }
</style>

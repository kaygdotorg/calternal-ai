<script lang="ts">
  // The Threads app's sidebar body. No automatic groups: the person makes their own sections (create, rename, collapse, delete) and arranges threads
  // freely, between sections and within them, by dragging or with the keyboard (Alt+Up and Alt+Down within a section; Alt+Shift+Up and Down to the
  // next or previous section). Double-click or F2 renames a thread in place; right-click (or the menu key) opens its context menu: rename, move to a
  // section, copy link, archive, stop if it is running. A thread's state shows only when it means something (running, waiting for you, failed). A thread
  // that needs the person is also listed in the Inbox tab (the list's other tab, All Threads, lists every thread).
  // A row: the provider's mark at the left, the title, then the state (only when it means something). Pointing at a row (or focusing into it) swaps the state for exactly three actions: copy link,
  // archive and a three-dot button that opens the very menu right-click opens (`ctxItems`, one definition, two triggers). On touch, and on a trackpad, the row swipes (lib/swipe, the existing app's
  // engine): right shows Copy link; left shows Archive (a long swipe runs it) and More (the same menu). The shell's edge swipes (24 px at each screen edge) and dragging between sections are never started by it.
  import { Button, Icon, IconButton, InlineEditor, Menu, Row, toast, undoable, type MenuItem } from "@calternal/ui";
  import { href, navigate, segments } from "../../lib/route.svelte";
  import { layout, type Container } from "./layout.svelte";
  import { archiveThread, containerName, containersOf, copyThreadLink, runThreadItem, snoozeItems, threadItems, topLevelIds } from "./thread-menu";
  import { backLabel, backShort } from "./snooze";
  import StateMark from "./StateMark.svelte";
  import SwipeRow, { type SwipeAction } from "../../lib/swipe/SwipeRow.svelte";
  import { swipeRows } from "../../lib/swipe/rows";
  import type { ThreadSummary } from "./model";
  import { client, startThread, store } from "./store.svelte";
  import { spaces } from "../../shell/spaces.svelte";
  import TopEntries from "./TopEntries.svelte";
  import ReasonMark from "./ReasonMark.svelte";
  import { messageSearch } from "./search.svelte";

  // A fresh account: nothing listed, no section made, and the list has loaded (so it is not just still arriving).
  const empty = $derived(store.ready && store.threads.length === 0 && layout.sections.length === 0);
  const current = $derived(segments()[0] === "threads" ? segments()[1] : undefined);
  const shown = $derived(store.threads.filter((t) => t.spaceId === spaces.current.id)); // only the current space's threads
  const byId = $derived(new Map(shown.map((t) => [t.id, t])));
  const top = $derived(topLevelIds());
  const idsIn = (c: Container) => layout.idsOf(c, top);
  const ids0 = $derived(idsIn(null));
  const kids = (id: string) => shown.filter((t) => t.parentId === id).sort((a, b) => b.updatedAt - a.updatedAt);
  const containers = $derived<Container[]>(containersOf());
  const nameOf = containerName;

  // ---- Inbox and All Threads ----
  // Two tabs under the shortcuts. The Inbox lists only the threads waiting on the person (inbox.ts); it is the page the list opens on when it is not empty. An explicit choice of tab is kept for the visit.
  let chosen = $state<"inbox" | "all" | null>(null);
  const items = $derived(store.inbox.filter((i) => i.thread.spaceId === spaces.current.id));
  // Snoozed: the threads that left the Inbox until a time (the person's snooze, or a subscription limit waiting to reset). Collapsed by default; the section and its count are always there when there is one. The clock moves to the next return, so a thread comes back by itself.
  const snoozedHere = $derived(store.snoozed.filter((s) => s.thread.spaceId === spaces.current.id));
  let snoozedOpen = $state(false);
  $effect(() => {
    const next = Math.min(...store.snoozed.map((s) => s.until));
    if (!Number.isFinite(next)) return;
    const h = setTimeout(() => (store.now = Date.now()), Math.max(50, next - Date.now() + 50));
    return () => clearTimeout(h);
  });
  const returns = (s: { until: number; auto: boolean }) => (s.auto ? `Back when the limit resets, ${new Date(s.until).toLocaleTimeString("en-GB", { hour: "2-digit", minute: "2-digit" })}` : backLabel(s.until, store.now));
  const tab = $derived(chosen ?? (items.length ? "inbox" : "all"));
  const tabKey = (e: KeyboardEvent) => {
    if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
    e.preventDefault();
    chosen = tab === "inbox" ? "all" : "inbox";
    queueMicrotask(() => document.querySelector<HTMLElement>('[role="tab"][aria-selected="true"]')?.focus());
  };

  // ---- renaming in place ----
  let editing = $state<{ kind: "thread" | "section"; id: string } | null>(null);
  const LOOSE = "__loose__";
  let draft = $state("");
  const edit = (kind: "thread" | "section", id: string, text: string) => { editing = { kind, id }; draft = text; };
  function commit() {
    if (!editing) return;
    const { kind, id } = editing;
    editing = null;
    if (kind === "thread") void client.rename(id, draft).catch((e: Error) => toast(`Not renamed: ${e.message}`));
    else if (id === LOOSE) layout.renameLoose(draft);
    else layout.rename(id, draft);
  }

  // ---- context menus ----
  let ctx = $state<{ x: number; y: number; thread?: ThreadSummary; section?: string; snooze?: boolean } | null>(null);
  const ctxItems = $derived<MenuItem[]>(
    ctx?.thread
      ? ctx.snooze ? snoozeItems(ctx.thread) : threadItems(ctx.thread)
      : ctx?.section
        ? ctx.section === LOOSE
        ? [{ id: "srename", label: "Rename section" }]
        : [{ id: "srename", label: "Rename section" }, { id: "up", label: "Move up" }, { id: "down", label: "Move down" }, { id: "delete", label: "Delete section", danger: true }]
      : [],
  );
  const archive = (t: ThreadSummary) => archiveThread(t);
  const copyLink = copyThreadLink;
  /** The three-dot button and the swipe's More open the same menu as right-click, under the control that asked. */
  const menuFrom = (el: HTMLElement | undefined, t: ThreadSummary, snooze = false) => { const r = el?.getBoundingClientRect(); ctx = { x: r?.right ?? 0, y: r?.bottom ?? 0, thread: t, snooze }; };
  /** The swipe buttons of a row: right shows the first list, left the second (slot 0 is the outer edge; a long swipe runs it). */
  const swipeOf = (t: ThreadSummary): { lead: SwipeAction[]; trail: SwipeAction[] } => ({
    lead: [{ id: "copy", label: "Copy link", icon: "link", tone: "plain", run: () => copyLink(t) }],
    trail: [
      { id: "archive", label: "Archive", icon: "archive", tone: "danger", full: true, leaves: true, run: () => archive(t) },
      { id: "more", label: "More", icon: "more", tone: "plain", run: (el) => menuFrom(el, t) },
    ],
  });
  function pick(id: string) {
    const c = ctx;
    ctx = null;
    if (!c) return;
    if (c.thread) {
      const t = c.thread;
      runThreadItem(id, t, () => edit("thread", t.id, t.title));
    } else if (c.section) {
      if (c.section === LOOSE) { if (id === "srename") edit("section", LOOSE, layout.looseName); return; }
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

{#snippet line(t: ThreadSummary, depth: number, selected: boolean)}
  {@const sw = swipeOf(t)}
  <SwipeRow lead={sw.lead} trail={sw.trail} busy={() => dragId !== null}>
    <div class="rw">
      <Row {depth} title={t.title} href={href(`/threads/${t.id}`)} {selected}>
        {#snippet leading()}<span class="mark" class:pulse={t.state === "running"}><Icon inline name={t.provider} /></span>{/snippet}
        {#snippet trailing()}<span class="tail"><StateMark state={t.state} /></span>{/snippet}
      </Row>
      <span class="acts">
        <IconButton small icon="link" label="Copy link" onclick={() => copyLink(t)} />
        <IconButton small icon="archive" label="Archive" onclick={() => void archive(t)} />
        <IconButton small icon="more" label="More actions" aria-haspopup="menu" onclick={(e) => menuFrom(e.currentTarget as HTMLElement, t)} />
      </span>
    </div>
  </SwipeRow>
{/snippet}

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
      {@render line(t, 0, current === t.id)}
      {#each kids(t.id) as k (k.id)}{@render line(k, 1, current === k.id)}{/each}
    </li>
  {/if}
{/snippet}

{#snippet list(c: Container, ids: string[])}
  <ul class="threads" ondragover={(e) => { if (dragId) { e.preventDefault(); if (!ids.length) over = { c, index: 0 }; } }} ondrop={(e) => { e.preventDefault(); if (over) dropHere(over.c, over.index); }} role="presentation">
    {#each ids as id, i (id)}{#if byId.get(id)}{@render thread(byId.get(id)!, c, i, ids.length)}{/if}{/each}
    {#if !ids.length && c !== null}<li class="hint" class:over={over?.c === c}>Drag threads here</li>{/if}
  </ul>
{/snippet}

{#if !empty}<TopEntries />{/if}
<nav aria-label="Threads" use:swipeRows>
  {#if empty}
    <div class="empty">
      <h2>No threads yet</h2>
      <p>Start one and say what you want an agent to do.</p>
      <Button variant="primary" text="Start a thread" onclick={() => void startThread().then((t) => navigate(`/threads/${t.id}`)).catch((e: Error) => toast(`Could not start a thread: ${e.message}`))} />
    </div>
  {:else}
  <div class="tabrow"><div class="tabs" role="tablist" aria-label="Which threads" tabindex="-1" onkeydown={tabKey}>
    <button type="button" role="tab" id="tab-inbox" aria-selected={tab === "inbox"} tabindex={tab === "inbox" ? 0 : -1} onclick={() => (chosen = "inbox")}>Inbox{#if items.length}<span class="n">{items.length}</span>{/if}</button>
    <button type="button" role="tab" id="tab-all" aria-selected={tab === "all"} tabindex={tab === "all" ? 0 : -1} onclick={() => (chosen = "all")}>All Threads</button>
  </div><IconButton icon="search" label="Search messages" onclick={() => (messageSearch.open = true)} /></div>
  {#if tab === "inbox"}
    <div role="tabpanel" aria-labelledby="tab-inbox">
      {#if items.length}
        <ul class="threads" aria-label="Waiting for you">
          {#each items as it (it.thread.id)}
            <li data-thread={it.thread.id}>
              <SwipeRow lead={[]} trail={snoozeItems(it.thread).length ? [{ id: "snooze", label: "Snooze", icon: "clock", tone: "plain", run: (el) => menuFrom(el, it.thread, true) }] : []}>
                <div class="rw">
                  <Row title={it.thread.title} href={href(`/threads/${it.thread.id}`)} selected={current === it.thread.id}>
                    {#snippet leading()}<span class="mark"><Icon inline name={it.thread.provider} /></span>{/snippet}
                    {#snippet trailing()}<span class="tail"><ReasonMark reasons={it.reasons} /></span>{/snippet}
                  </Row>
                  {#if snoozeItems(it.thread).length}<span class="acts"><IconButton small icon="clock" label="Snooze" aria-haspopup="menu" onclick={(e) => menuFrom(e.currentTarget as HTMLElement, it.thread, true)} /></span>{/if}
                </div>
              </SwipeRow>
            </li>
          {/each}
        </ul>
      {:else}
        <p class="none" role="status">Nothing is waiting for you. A thread shows up here when it asks a question, needs an approval or a sign-in, or fails.</p>
      {/if}
      {#if snoozedHere.length}
        <div class="snz" role="group" aria-label="Snoozed">
          <button type="button" class="sg" aria-expanded={snoozedOpen} onclick={() => (snoozedOpen = !snoozedOpen)}><Icon inline name="clock" />Snoozed <span class="n">{snoozedHere.length}</span></button>
          {#if snoozedOpen}
            <ul class="threads" aria-label="Snoozed threads">
              {#each snoozedHere as s (s.thread.id)}
                <li data-thread={s.thread.id} data-snoozed={s.auto ? "auto" : "manual"}>
                  <div class="rw">
                    <Row title={s.thread.title} href={href(`/threads/${s.thread.id}`)} selected={current === s.thread.id}>
                      {#snippet leading()}<span class="mark"><Icon inline name={s.thread.provider} /></span>{/snippet}
                      {#snippet trailing()}<span class="tail" title={returns(s)}>{s.auto ? "Resets " : ""}{backShort(s.until, store.now)}</span>{/snippet}
                    </Row>
                    {#if !s.auto}<span class="acts"><IconButton small icon="close" label="Unsnooze now" onclick={() => runThreadItem("snooze:off", s.thread, () => {})} /></span>{/if}
                  </div>
                </li>
              {/each}
            </ul>
          {/if}
        </div>
      {/if}
    </div>
  {:else}
  <div role="tabpanel" aria-labelledby="tab-all">
  <div class="sec first" role="group" aria-label={layout.looseName}>
    <div class="sh" class:over={over?.c === null && !ids0.length} role="presentation" ondragover={(e) => { if (dragId) { e.preventDefault(); over = { c: null, index: ids0.length }; } }} ondrop={(e) => { e.preventDefault(); dropHere(null, ids0.length); }} oncontextmenu={(e) => openCtx(e, { section: LOOSE })}>
      {#if editing?.kind === "section" && editing.id === LOOSE}
        <InlineEditor class="rename" label="Rename section" bind:value={draft} autoselect onblur={commit} onsubmit={commit} oncancel={() => (editing = null)} />
      {:else}
        <button type="button" class="tg" aria-expanded={!layout.looseCollapsed} onclick={() => layout.toggleLoose()} ondblclick={() => edit("section", LOOSE, layout.looseName)}>
          <span class="chev" class:open={!layout.looseCollapsed}><Icon inline name="chevron" /></span><span class="nm">{layout.looseName}</span>
        </button>
        <IconButton small icon="more" label="Section options: {layout.looseName}" onclick={(e) => { const r = (e.currentTarget as HTMLElement).getBoundingClientRect(); ctx = { x: r.left, y: r.bottom, section: LOOSE }; }} />
      {/if}
    </div>
    {#if !layout.looseCollapsed}{@render list(null, ids0)}{/if}
  </div>

  {#each layout.sections as s (s.id)}
    {@const ids = idsIn(s.id)}
    <div class="sec" role="group" aria-label={s.name}>
      <div class="sh" class:over={over?.c === s.id && !ids.length} role="presentation" ondragover={(e) => { if (dragId) { e.preventDefault(); over = { c: s.id, index: ids.length }; } }} ondrop={(e) => { e.preventDefault(); dropHere(s.id, ids.length); }} oncontextmenu={(e) => openCtx(e, { section: s.id })}>
        {#if editing?.kind === "section" && editing.id === s.id}
          <InlineEditor class="rename" label="Rename section" bind:value={draft} autoselect onblur={commit} onsubmit={commit} oncancel={() => (editing = null)} />
        {:else}
          <button type="button" class="tg" aria-expanded={!s.collapsed} onclick={() => layout.toggle(s.id)} ondblclick={() => edit("section", s.id, s.name)}>
            <span class="chev" class:open={!s.collapsed}><Icon inline name="chevron" /></span><span class="nm">{s.name}</span>
          </button>
          <IconButton small icon="more" label="Section options: {s.name}" onclick={(e) => { const r = (e.currentTarget as HTMLElement).getBoundingClientRect(); ctx = { x: r.left, y: r.bottom, section: s.id }; }} />
        {/if}
      </div>
      {#if !s.collapsed}{@render list(s.id, ids)}{/if}
    </div>
  {/each}

  <button type="button" class="new" onclick={newSection}><Icon inline name="plus" /><span>New section</span></button>
  </div>
  {/if}
  {/if}
  <p class="visually-hidden" role="status" aria-live="polite">{said}</p>
</nav>

<Menu open={!!ctx} items={ctxItems} label={ctx?.thread ? `Thread: ${ctx.thread.title}` : "Section"} anchor={ctx ? { left: ctx.x, top: ctx.y, right: ctx.x, bottom: ctx.y } : null} placement="bottom" onclose={() => (ctx = null)} onpick={pick} />

<style>
  /* A row: the provider's mark leads; the state sits at the right; pointing at the row swaps the state for the three actions. */
  .rw { position: relative; }
  .snz { margin-top: var(--s-3); }
  .sg { display: flex; align-items: center; gap: var(--s-3); min-height: var(--row-h); padding: 0 var(--s-4); border-radius: var(--r-select); font-size: var(--text-label); font-weight: var(--strong-weight); color: var(--ink-3); }
  .sg .n { font-variant-numeric: tabular-nums; }
  @media (hover: hover) and (pointer: fine) { .sg:hover { color: var(--ink); } }
  .mark { flex: none; display: grid; color: var(--ink-2); }
  .tail { display: inline-flex; align-items: center; gap: var(--s-3); color: var(--ink-3); }
  .acts { position: absolute; right: var(--s-2); top: 50%; translate: 0 -50%; display: inline-flex; gap: var(--s-1); opacity: 0; pointer-events: none; transition: opacity var(--dur-fast) var(--ease-out); }
  @media (hover: hover) and (pointer: fine) {
    .rw:hover .acts, .rw:focus-within .acts { opacity: 1; pointer-events: auto; }
    /* The state steps aside and the title leaves room for the three actions (it never runs under them). */
    .rw:hover .tail, .rw:focus-within .tail { display: none; }
    .rw:hover :global(.row), .rw:focus-within :global(.row) { padding-right: calc(var(--s-2) * 2 + 3 * var(--ib-small) + 2 * var(--s-1)); }
  }
  /* The two tabs split the sidebar's width in halves; the selected one's capsule spans its half (no empty space at the right). */
  .tabrow { display: flex; align-items: center; gap: var(--s-1); margin: var(--s-3) 0 var(--s-2); padding-right: var(--s-2); }
  .tabrow .tabs { flex: 1; margin: 0; }
  .tabs { display: grid; grid-template-columns: 1fr 1fr; gap: var(--s-1); margin: var(--s-3) 0 var(--s-2); padding: 0 var(--s-2); }
  .tabs button { display: inline-flex; align-items: center; justify-content: center; gap: var(--s-2); min-height: var(--row-h); padding: 0 var(--s-4); border-radius: var(--r-pill); font-size: var(--text-label); font-weight: var(--strong-weight); color: var(--ink-3); }
  .tabs button[aria-selected="true"] { background: rgb(var(--ink-rgb) / 0.1); color: var(--ink); }
  .tabs .n { font-variant-numeric: tabular-nums; color: var(--accent); }
  @media (hover: hover) and (pointer: fine) { .tabs button:hover { color: var(--ink); } }
  .none { padding: var(--s-4) var(--s-5); color: var(--ink-3); font-size: var(--text-label); }
  .empty { display: grid; gap: var(--s-4); justify-items: start; padding: var(--s-6) var(--s-5); }
  .empty h2 { font-size: var(--text-heading); font-weight: var(--strong-weight); }
  .empty p { color: var(--ink-2); font-size: var(--text-ui); }
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
  /* A section header is an interactive row, taller than the heading line: its margins take the difference out so the distances from its text are the shared ones (base.css `.section-heading`). */
  .sec { display: grid; gap: 0; margin-top: max(0px, calc(var(--section-above) - var(--section-pad))); }
  .sh { display: flex; align-items: center; gap: var(--s-1); border-radius: var(--r-select); margin-bottom: max(0px, calc(var(--section-below) - var(--section-pad))); }
  .sh.over { background: var(--accent-soft); }
  .tg { flex: 1; min-width: 0; display: flex; align-items: center; gap: var(--s-2); min-height: var(--row-h); padding: 0 var(--s-4); border-radius: var(--r-select); text-align: start; font-size: var(--text-label); font-weight: var(--strong-weight); color: var(--ink-3); }
  .nm { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .chev { display: grid; transition: rotate var(--dur-base) var(--ease-out); }
  .chev.open { rotate: 90deg; }
  :global(.rename) { width: 100%; min-height: var(--row-h); padding: 0 var(--s-4); border: 0; border-radius: var(--r-select); background: rgb(var(--ink-rgb) / 0.1); color: var(--ink); font-size: var(--text-ui); }
  .new { display: flex; align-items: center; gap: var(--s-3); min-height: var(--row-h); margin-top: var(--s-3); padding: 0 var(--s-4); border-radius: var(--r-select); color: var(--ink-3); font-size: var(--text-label); font-weight: var(--strong-weight); }
  @media (hover: hover) and (pointer: fine) { .tg:hover, .new:hover { background: var(--hover); color: var(--ink); } }
</style>
